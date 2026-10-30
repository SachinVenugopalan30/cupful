// SPDX-License-Identifier: AGPL-3.0-or-later
//! Tailscale identity mode (PLAN.md §10, D4 to D7).
//!
//! A Tailscale sidecar adds `Tailscale-User-Login` to requests. When
//! `CUPFUL_TRUST_TAILSCALE` is on and a loopback bind is enforced, an anonymous
//! API request is mapped to a non-admin user. Headers never override a session.

use axum::extract::{Request, State};
use axum::http::HeaderName;
use axum::middleware::Next;
use axum::response::Response;
use sqlx::{Sqlite, SqlitePool, Transaction};
use tower_sessions::Session;

use super::USER_KEY;
use crate::AppState;
use crate::time::now_ms;

/// Lowercase a `Tailscale-User-Login` value. `None` means ignore the header.
pub fn normalize_login(raw: &str) -> Option<String> {
    let login = raw.trim().to_lowercase();
    let valid = !login.is_empty() && login.len() <= 320 && !login.chars().any(char::is_control);
    valid.then_some(login)
}

pub async fn identify(State(state): State<AppState>, mut req: Request, next: Next) -> Response {
    let login = req
        .headers()
        .get("tailscale-user-login")
        .and_then(|v| v.to_str().ok())
        .and_then(normalize_login);
    strip_headers(&mut req);

    if !state.config.trust_tailscale {
        return next.run(req).await;
    }
    let path = req.uri().path();
    if !path.starts_with("/api/") || path == "/api/health" {
        return next.run(req).await;
    }
    let Some(login) = login else {
        return next.run(req).await;
    };
    let Some(session) = req.extensions().get::<Session>().cloned() else {
        return next.run(req).await;
    };
    match session.get::<i64>(USER_KEY).await {
        Ok(Some(_)) => return next.run(req).await,
        Ok(None) => {}
        Err(e) => {
            tracing::warn!("tailscale identity: cannot read the session: {e}");
            return next.run(req).await;
        }
    }

    match find_or_create(&state.db, &login).await {
        Ok(Some(id)) => {
            if let Err(e) = super::log_in(&session, id).await {
                tracing::error!("tailscale identity: cannot log in user {id}: {e:?}");
            }
        }
        Ok(None) => {}
        Err(e) => tracing::error!("tailscale identity: {e}"),
    }
    next.run(req).await
}

/// Removes every `Tailscale-User-*` header in both modes, so handlers never see one.
fn strip_headers(req: &mut Request) {
    let names: Vec<HeaderName> = req
        .headers()
        .keys()
        .filter(|n| n.as_str().starts_with("tailscale-user-"))
        .cloned()
        .collect();
    for name in names {
        req.headers_mut().remove(&name);
    }
}

/// Finds the user bound to `login`, or creates one. `None` means no admin exists yet.
async fn find_or_create(db: &SqlitePool, login: &str) -> Result<Option<i64>, sqlx::Error> {
    let mut tx = db.begin_with("BEGIN IMMEDIATE").await?;
    let has_admin = sqlx::query_scalar!(
        "SELECT EXISTS (SELECT 1 FROM users WHERE is_admin = 1) AS \"v!: bool\""
    )
    .fetch_one(&mut *tx)
    .await?;
    if !has_admin {
        tx.rollback().await?;
        return Ok(None);
    }

    if let Some(id) = sqlx::query_scalar!("SELECT id FROM users WHERE tailscale_login = ?", login)
        .fetch_optional(&mut *tx)
        .await?
    {
        tx.commit().await?;
        return Ok(Some(id));
    }

    let username = free_username(&mut tx, login).await?;
    let id = sqlx::query_scalar!(
        "INSERT INTO users (username, password_hash, tailscale_login, is_admin, created_at)
         VALUES (?, NULL, ?, 0, ?) RETURNING id",
        &username,
        login,
        now_ms()
    )
    .fetch_one(&mut *tx)
    .await?;
    sqlx::query!(
        "INSERT INTO profiles (user_id, timezone) VALUES (?, 'UTC')",
        id
    )
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    tracing::info!(login, user = %username, "created a user from Tailscale identity");
    Ok(Some(id))
}

async fn free_username(
    tx: &mut Transaction<'_, Sqlite>,
    login: &str,
) -> Result<String, sqlx::Error> {
    let base = derive_username(login);
    for candidate in candidates(&base) {
        if is_free(tx, &candidate).await? {
            return Ok(candidate);
        }
    }
    loop {
        let random = uuid::Uuid::now_v7().simple().to_string();
        let suffix = format!("-{}", &random[..8]);
        let keep = 32 - suffix.len();
        let candidate = format!("{}{suffix}", &base[..base.len().min(keep)]);
        if is_free(tx, &candidate).await? {
            return Ok(candidate);
        }
    }
}

/// The email local part, limited to username characters, at most 32 bytes.
fn derive_username(login: &str) -> String {
    let local = login.split('@').next().unwrap_or(login);
    let name: String = local
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || "_.-".contains(*c))
        .take(32)
        .collect();
    if name.is_empty() {
        "tailscale".to_string()
    } else {
        name
    }
}

/// `base`, then `base-2` … `base-9`, each cut to the 32-byte username limit.
fn candidates(base: &str) -> impl Iterator<Item = String> + '_ {
    std::iter::once(base.to_string()).chain((2..=9).map(move |n| {
        let suffix = format!("-{n}");
        let keep = 32 - suffix.len();
        format!("{}{suffix}", &base[..base.len().min(keep)])
    }))
}

async fn is_free(tx: &mut Transaction<'_, Sqlite>, username: &str) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar!(
        "SELECT NOT EXISTS (SELECT 1 FROM users WHERE username = ?) AS \"v!: bool\"",
        username
    )
    .fetch_one(&mut **tx)
    .await
}

/// `bind-tailscale` CLI: binds an existing user to a tailnet login, lowercased.
pub async fn bind_tailscale(
    db: &SqlitePool,
    username: &str,
    raw_login: &str,
) -> Result<(), String> {
    let login = normalize_login(raw_login).ok_or_else(|| {
        format!("invalid Tailscale login {raw_login:?}: expected a non-empty address or hostname")
    })?;
    let user = sqlx::query!(
        "SELECT id, tailscale_login FROM users WHERE username = ?",
        username
    )
    .fetch_optional(db)
    .await
    .map_err(|e| e.to_string())?
    .ok_or_else(|| format!("no user named {username:?}"))?;

    if let Some(existing) = user.tailscale_login {
        if existing == login {
            return Ok(());
        }
        return Err(format!(
            "{username:?} is already bound to {existing:?}; one tailnet login per user"
        ));
    }
    if let Some(other) = sqlx::query_scalar!(
        "SELECT username FROM users WHERE tailscale_login = ?",
        login
    )
    .fetch_optional(db)
    .await
    .map_err(|e| e.to_string())?
    {
        return Err(format!(
            "tailnet login {login:?} is already bound to user {other:?}"
        ));
    }

    sqlx::query!(
        "UPDATE users SET tailscale_login = ? WHERE id = ?",
        login,
        user.id
    )
    .execute(db)
    .await
    .map_err(|e| match e {
        sqlx::Error::Database(d) if d.is_unique_violation() => {
            format!("tailnet login {login:?} is already bound")
        }
        e => e.to_string(),
    })?;
    Ok(())
}
