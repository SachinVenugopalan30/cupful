// SPDX-License-Identifier: AGPL-3.0-or-later
//! One-use invite links (PLAN.md §10): an admin creates one, a new person accepts it.
//! Only the SHA-256 hex of the token is stored; the plaintext is returned once.

use std::time::Instant;

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tower_sessions::Session;

use super::{
    Admin, ClientIp, hash_password, insert_user, log_in, validate_password, validate_timezone,
    validate_username,
};
use crate::AppState;
use crate::crypto::random_token;
use crate::error::{Error, JsonBody, Result};
use crate::time::{now_ms, rfc3339};

const EXPIRY_MS: i64 = 7 * 86_400_000;
/// Unknown and unusable tokens share one bucket, so random tokens cannot grow the limiter map.
const MISS_KEY: &str = "invite:miss";

fn hash_token(token: &str) -> String {
    Sha256::digest(token.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn gone() -> Error {
    Error::Status(
        StatusCode::GONE,
        "invite is unknown, used or expired".into(),
    )
}

fn identity_mode() -> Error {
    Error::conflict("invites are disabled while Tailscale identity mode is on")
}

fn too_many() -> Error {
    Error::Status(
        StatusCode::TOO_MANY_REQUESTS,
        "too many invite attempts; wait an hour".into(),
    )
}

// ---- admin ----

pub async fn list(State(s): State<AppState>, _admin: Admin) -> Result<Json<Value>> {
    let rows = sqlx::query!(
        "SELECT id, created_at, expires_at FROM invites
         WHERE used_at IS NULL AND expires_at > ? ORDER BY id",
        now_ms()
    )
    .fetch_all(&s.db)
    .await?;
    let invites = rows
        .into_iter()
        .map(|r| {
            Ok(json!({
                "id": r.id,
                "created_at": rfc3339(r.created_at)?,
                "expires_at": rfc3339(r.expires_at)?,
            }))
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(Json(json!({ "invites": invites })))
}

pub async fn create(State(s): State<AppState>, admin: Admin) -> Result<Json<Value>> {
    if s.config.trust_tailscale {
        return Err(identity_mode());
    }
    let token = random_token();
    let now = now_ms();
    let expires = now + EXPIRY_MS;
    let id = sqlx::query_scalar!(
        "INSERT INTO invites (token_hash, created_by, created_at, expires_at)
         VALUES (?, ?, ?, ?) RETURNING id",
        hash_token(&token),
        admin.0.id,
        now,
        expires
    )
    .fetch_one(&s.db)
    .await?;
    Ok(Json(json!({
        "id": id, "token": token, "expires_at": rfc3339(expires)?,
    })))
}

pub async fn revoke(
    State(s): State<AppState>,
    _admin: Admin,
    Path(id): Path<i64>,
) -> Result<Json<Value>> {
    let done = sqlx::query!("DELETE FROM invites WHERE id = ? AND used_at IS NULL", id)
        .execute(&s.db)
        .await?;
    if done.rows_affected() == 0 {
        return Err(Error::not_found());
    }
    Ok(Json(json!({})))
}

// ---- acceptance ----

#[derive(Deserialize)]
pub struct AcceptBody {
    token: String,
    username: String,
    password: String,
    timezone: String,
}

pub async fn accept(
    State(s): State<AppState>,
    session: Session,
    ip: ClientIp,
    JsonBody(b): JsonBody<AcceptBody>,
) -> Result<Json<Value>> {
    if s.config.trust_tailscale {
        return Err(identity_mode());
    }
    let now = now_ms();
    let token_hash = hash_token(&b.token);
    let row = sqlx::query!(
        "SELECT id, expires_at, used_at FROM invites WHERE token_hash = ?",
        token_hash
    )
    .fetch_optional(&s.db)
    .await?;
    let usable = row
        .as_ref()
        .is_some_and(|r| r.used_at.is_none() && r.expires_at > now);
    // Before the Argon2 hash: cheap check, and unknown tokens only touch the shared miss bucket.
    let instant = Instant::now();
    let key = match &row {
        Some(r) if usable => format!("invite:{}", r.id),
        _ => MISS_KEY.to_string(),
    };
    let by_invite = s.invite_limiter.allow(&key, instant);
    let by_ip =
        ip.0.is_none_or(|ip| s.invite_limiter.allow(&format!("ip:{ip}"), instant));
    if !(by_invite && by_ip) {
        return Err(too_many());
    }
    if !usable {
        return Err(gone());
    }
    validate_username(&b.username)?;
    validate_password(&b.password)?;
    validate_timezone(&b.timezone)?;
    let password_hash = hash_password(&b.password).await?;

    let mut tx = s.db.begin_with("BEGIN IMMEDIATE").await?;
    let invite = sqlx::query!(
        "SELECT id FROM invites
         WHERE token_hash = ? AND used_at IS NULL AND expires_at > ?",
        token_hash,
        now
    )
    .fetch_optional(&mut *tx)
    .await?;
    let invite = invite.ok_or_else(gone)?;
    let id = insert_user(
        &mut tx,
        &b.username,
        &password_hash,
        &b.timezone,
        false,
        false,
        now,
    )
    .await?
    .expect("unconditional insert returns an id");
    let claimed = sqlx::query!(
        "UPDATE invites SET used_by = ?, used_at = ? WHERE id = ? AND used_at IS NULL",
        id,
        now,
        invite.id
    )
    .execute(&mut *tx)
    .await?;
    if claimed.rows_affected() != 1 {
        return Err(gone());
    }
    tx.commit().await?;
    log_in(&session, id).await?;
    Ok(Json(json!({ "id": id })))
}
