// SPDX-License-Identifier: AGPL-3.0-or-later
//! Google OAuth: connect redirect, callback, token storage.

use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::Redirect;
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD as B64URL;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tower_sessions::Session;

use super::{SCOPE, SESSION_KEY, creds, form, read_json, redirect_uri};
use crate::AppState;
use crate::auth::User;
use crate::crypto::random_token;
use crate::error::{Error, Query, Result};
use crate::time::now_ms;

#[derive(Serialize, Deserialize)]
struct Pending {
    state: String,
    verifier: String,
    /// Fixed at connect: Google requires the same value in the token exchange.
    redirect_uri: String,
}

pub async fn connect(
    State(s): State<AppState>,
    _user: User,
    session: Session,
    headers: HeaderMap,
) -> Result<Redirect> {
    let (client_id, _) = creds(&s)?;
    let pending = Pending {
        state: random_token(),
        verifier: random_token(),
        redirect_uri: redirect_uri(&s, &headers)?,
    };
    let challenge = B64URL.encode(Sha256::digest(pending.verifier.as_bytes()));
    let url = format!(
        "{}?{}",
        s.config.google_auth_url,
        form(&[
            ("client_id", client_id),
            ("redirect_uri", &pending.redirect_uri),
            ("response_type", "code"),
            ("scope", SCOPE),
            ("state", &pending.state),
            ("code_challenge", &challenge),
            ("code_challenge_method", "S256"),
            ("access_type", "offline"),
            ("prompt", "consent"),
        ])
    );
    session.insert(SESSION_KEY, pending).await?;
    Ok(Redirect::to(&url))
}

#[derive(Deserialize)]
pub struct CallbackQuery {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct TokenReply {
    pub(super) access_token: Option<String>,
    pub(super) refresh_token: Option<String>,
    pub(super) expires_in: Option<i64>,
    pub(super) error: Option<String>,
}

pub(super) enum TokenError {
    /// The refresh token or code is dead; the user must connect again.
    InvalidGrant,
    Other(String),
}

pub(super) async fn token_request(
    s: &AppState,
    pairs: &[(&str, &str)],
) -> std::result::Result<TokenReply, TokenError> {
    let (id, secret) = creds(s).map_err(|_| TokenError::Other("not configured".into()))?;
    let mut all = vec![("client_id", id), ("client_secret", secret)];
    all.extend_from_slice(pairs);
    let res = s
        .google_http
        .post(&s.config.google_token_url)
        .header("content-type", "application/x-www-form-urlencoded")
        .body(form(&all))
        .send()
        .await
        .map_err(|e| TokenError::Other(format!("could not reach Google: {}", e.without_url())))?;
    let ok = res.status().is_success();
    let reply: TokenReply = read_json(res)
        .await
        .ok_or_else(|| TokenError::Other("unreadable reply from Google".into()))?;
    match (ok, reply.error.as_deref()) {
        (true, None) if reply.access_token.is_some() => Ok(reply),
        (_, Some("invalid_grant")) => Err(TokenError::InvalidGrant),
        (_, e) => Err(TokenError::Other(format!(
            "Google refused the request ({})",
            e.unwrap_or("no token")
        ))),
    }
}

pub(super) async fn save_tokens(
    s: &AppState,
    user_id: i64,
    t: &TokenReply,
    refresh: Option<&str>,
) -> Result<()> {
    let access = s
        .secrets
        .seal(&s.db, t.access_token.as_deref().unwrap_or_default())
        .await?;
    let expires = now_ms() + t.expires_in.unwrap_or(3600) * 1000;
    match refresh {
        Some(r) => {
            let refresh = s.secrets.seal(&s.db, r).await?;
            let now = now_ms();
            sqlx::query!(
                "INSERT INTO google_connections
                   (user_id, refresh_token, access_token, access_expires_at, connected_at)
                 VALUES (?, ?, ?, ?, ?)
                 ON CONFLICT (user_id) DO UPDATE SET refresh_token = excluded.refresh_token,
                   access_token = excluded.access_token, access_expires_at = excluded.access_expires_at,
                   connected_at = excluded.connected_at, needs_reconnect = 0, last_error = NULL",
                user_id, refresh, access, expires, now
            )
            .execute(&s.db)
            .await?;
        }
        None => {
            sqlx::query!(
                "UPDATE google_connections SET access_token = ?, access_expires_at = ? WHERE user_id = ?",
                access, expires, user_id
            )
            .execute(&s.db)
            .await?;
        }
    }
    Ok(())
}

/// Google sends the browser here. The one-time `state` (kept in the session) must match, and the
/// tokens go to the user logged in to this session.
pub async fn callback(
    State(s): State<AppState>,
    user: User,
    session: Session,
    Query(q): Query<CallbackQuery>,
) -> Result<Redirect> {
    creds(&s)?;
    let pending: Option<Pending> = session.remove(SESSION_KEY).await?;
    let pending = pending
        .filter(|p| q.state.as_deref() == Some(p.state.as_str()))
        .ok_or_else(|| {
            Error::bad_request("unknown or expired sign-in; start again from Settings")
        })?;
    if q.error.is_some() {
        return Ok(Redirect::to("/settings?google=denied"));
    }
    let Some(code) = q.code else {
        return Err(Error::bad_request("missing code"));
    };
    let tokens = token_request(
        &s,
        &[
            ("grant_type", "authorization_code"),
            ("code", &code),
            ("code_verifier", &pending.verifier),
            ("redirect_uri", &pending.redirect_uri),
        ],
    )
    .await;
    match tokens {
        Ok(t) if t.refresh_token.is_some() => {
            save_tokens(&s, user.id, &t, t.refresh_token.as_deref()).await?;
            Ok(Redirect::to("/settings?google=connected"))
        }
        Ok(_) => Ok(Redirect::to("/settings?google=error")),
        Err(_) => {
            tracing::warn!("google code exchange failed");
            Ok(Redirect::to("/settings?google=error"))
        }
    }
}
