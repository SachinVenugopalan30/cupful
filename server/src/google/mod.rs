// SPDX-License-Identifier: AGPL-3.0-or-later
//! Google Health API v4 steps sync (PLAN.md §15). OAuth code flow with PKCE; tokens encrypted at
//! rest. Never logs tokens, codes or response bodies. Every route answers 404 unless
//! `CUPFUL_GOOGLE_CLIENT_ID` and `CUPFUL_GOOGLE_CLIENT_SECRET` are set.

mod oauth;
mod sync;
mod workouts;

pub use oauth::{callback, connect};
pub use sync::{sync_now, sync_user};
pub use workouts::{list as list_workouts, set_gym_types};

use std::time::Duration;

use axum::Json;
use axum::extract::State;
use axum::http::HeaderMap;
use serde_json::{Value, json};

use crate::AppState;
use crate::auth::User;
use crate::error::{Error, Result};
use crate::time::rfc3339;

pub const SCOPE: &str =
    "https://www.googleapis.com/auth/googlehealth.activity_and_fitness.readonly";
const SESSION_KEY: &str = "google_oauth";

fn creds(s: &AppState) -> Result<(&str, &str)> {
    match (&s.config.google_client_id, &s.config.google_client_secret) {
        (Some(id), Some(secret)) => Ok((id, secret.expose())),
        _ => Err(Error::not_found()),
    }
}

fn redirect_uri(s: &AppState, headers: &HeaderMap) -> Result<String> {
    let base = match (
        &s.config.public_url,
        headers.get("host").and_then(|h| h.to_str().ok()),
    ) {
        (Some(u), _) => u.clone(),
        (None, Some(h)) => format!(
            "{}://{h}",
            if s.config.secure_cookies {
                "https"
            } else {
                "http"
            }
        ),
        (None, None) => {
            return Err(Error::bad_request(
                "set CUPFUL_PUBLIC_URL; the host is unknown",
            ));
        }
    };
    Ok(format!("{base}/api/google/callback"))
}

/// `application/x-www-form-urlencoded` (also the authorization URL's query string).
fn form(pairs: &[(&str, &str)]) -> String {
    let mut u = reqwest::Url::parse("http://x/").expect("static URL");
    u.query_pairs_mut().extend_pairs(pairs);
    u.query().unwrap_or_default().to_string()
}

async fn read_json<T: serde::de::DeserializeOwned>(res: reqwest::Response) -> Option<T> {
    serde_json::from_slice(&res.bytes().await.ok()?).ok()
}

// ---- status ----

pub async fn status(State(s): State<AppState>, user: User) -> Result<Json<Value>> {
    if creds(&s).is_err() {
        return Ok(Json(json!({ "configured": false })));
    }
    let row = sqlx::query!(
        "SELECT last_sync_at, last_error, needs_reconnect FROM google_connections WHERE user_id = ?",
        user.id
    )
    .fetch_optional(&s.db)
    .await?;
    let mut out = match row {
        None => json!({ "configured": true, "connected": false }),
        Some(r) => json!({
            "configured": true,
            "connected": true,
            "needs_reconnect": r.needs_reconnect != 0,
            "last_sync_at": r.last_sync_at.map(rfc3339).transpose()?,
            "last_error": r.last_error,
        }),
    };
    workouts::extend_status(&s.db, user.id, &mut out).await?;
    Ok(Json(out))
}

/// Forgets the connection and asks Google to revoke the token (best effort: the row goes either way).
pub async fn disconnect(State(s): State<AppState>, user: User) -> Result<Json<Value>> {
    creds(&s)?;
    let row = sqlx::query_scalar!(
        "DELETE FROM google_connections WHERE user_id = ? RETURNING refresh_token",
        user.id
    )
    .fetch_optional(&s.db)
    .await?
    .ok_or_else(Error::not_found)?;
    if let Ok(token) = s.secrets.decrypt(&row) {
        let sent = s
            .google_http
            .post(&s.config.google_revoke_url)
            .header("content-type", "application/x-www-form-urlencoded")
            .body(form(&[("token", &token)]))
            .send()
            .await;
        if sent.is_err() {
            tracing::warn!(
                "google token revoke failed; remove access at myaccount.google.com/permissions"
            );
        }
    }
    Ok(Json(json!({})))
}

/// Daily sync for everyone still connected, starting a minute after boot.
pub fn spawn(s: AppState) {
    if creds(&s).is_err() {
        return;
    }
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(60)).await;
        let mut every = tokio::time::interval(Duration::from_secs(24 * 3600));
        loop {
            every.tick().await;
            let ids = sqlx::query_scalar!(
                "SELECT user_id FROM google_connections WHERE needs_reconnect = 0"
            )
            .fetch_all(&s.db)
            .await
            .unwrap_or_default();
            for id in ids {
                if sync_user(&s, id).await.is_err() {
                    tracing::warn!("google sync failed for user {id}");
                }
            }
        }
    });
}
