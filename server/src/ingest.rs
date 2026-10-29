// SPDX-License-Identifier: AGPL-3.0-or-later
//! Ingest tokens and the Shortcuts activity endpoint (PLAN.md §11).

use std::time::Instant;

use axum::Json;
use axum::extract::{FromRequestParts, Path, State};
use axum::http::StatusCode;
use axum::http::request::Parts;
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD as B64URL;
use chacha20poly1305::Key;
use chacha20poly1305::aead::common::Generate;
use jiff::ToSpan;
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::AppState;
use crate::auth::User;
use crate::error::{Error, JsonBody, Result};
use crate::sync::{DayPatch, Source, patch_day};
use crate::time::{now_ms, parse_date, user_today};

pub const ACTIVITY_PATH: &str = "/api/ingest/activity";
const PREFIX: &str = "cupful_";
const MAX_LABEL_CHARS: usize = 100;

fn hash(token: &str) -> String {
    B64URL.encode(Sha256::digest(token.as_bytes()))
}

fn rfc3339(ms: i64) -> Result<String> {
    jiff::Timestamp::from_millisecond(ms)
        .map(|t| t.to_string())
        .map_err(|e| Error::Internal(e.to_string()))
}

// ---- token management (session auth) ----

pub async fn list(State(s): State<AppState>, user: User) -> Result<Json<Value>> {
    let rows = sqlx::query!(
        "SELECT id, label, created_at, last_used_at FROM ingest_tokens
         WHERE user_id = ? ORDER BY id",
        user.id
    )
    .fetch_all(&s.db)
    .await?;
    let tokens = rows
        .into_iter()
        .map(|r| {
            Ok(json!({
                "id": r.id,
                "label": r.label,
                "created_at": rfc3339(r.created_at)?,
                "last_used_at": r.last_used_at.map(rfc3339).transpose()?,
            }))
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(Json(json!({ "tokens": tokens })))
}

#[derive(Deserialize)]
pub struct CreateIn {
    label: String,
}

pub async fn create(
    State(s): State<AppState>,
    user: User,
    JsonBody(b): JsonBody<CreateIn>,
) -> Result<Json<Value>> {
    if !(1..=MAX_LABEL_CHARS).contains(&b.label.chars().count()) {
        return Err(Error::bad_request(format!(
            "label must be 1-{MAX_LABEL_CHARS} characters"
        )));
    }
    let token = format!("{PREFIX}{}", B64URL.encode(Key::generate().as_slice()));
    let (hash, now) = (hash(&token), now_ms());
    let id = sqlx::query_scalar!(
        "INSERT INTO ingest_tokens (user_id, label, token_hash, created_at)
         VALUES (?, ?, ?, ?) RETURNING id",
        user.id,
        b.label,
        hash,
        now
    )
    .fetch_one(&s.db)
    .await?;
    Ok(Json(json!({
        "id": id, "label": b.label, "token": token, "created_at": rfc3339(now)?,
    })))
}

pub async fn revoke(
    State(s): State<AppState>,
    user: User,
    Path(id): Path<i64>,
) -> Result<Json<Value>> {
    let done = sqlx::query!(
        "DELETE FROM ingest_tokens WHERE id = ? AND user_id = ?",
        id,
        user.id
    )
    .execute(&s.db)
    .await?;
    if done.rows_affected() == 0 {
        return Err(Error::not_found());
    }
    Ok(Json(json!({})))
}

// ---- ingest (token auth) ----

/// The user a valid, unrevoked bearer token belongs to; the only way the ingest route learns who
/// is calling. Counts against that token's hourly limit.
pub struct TokenUser(pub i64);

impl FromRequestParts<AppState> for TokenUser {
    type Rejection = Error;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self> {
        let bad = || Error::Status(StatusCode::UNAUTHORIZED, "missing or invalid token".into());
        let token = parts
            .headers
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .ok_or_else(bad)?;
        let hash = hash(token.trim());
        let row = sqlx::query!(
            "SELECT id, user_id FROM ingest_tokens WHERE token_hash = ?",
            hash
        )
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(bad)?;
        if !state
            .ingest_limiter
            .allow(&format!("token:{}", row.id), Instant::now())
        {
            return Err(Error::Status(
                StatusCode::TOO_MANY_REQUESTS,
                "at most 60 requests per hour per token".into(),
            ));
        }
        let now = now_ms();
        sqlx::query!(
            "UPDATE ingest_tokens SET last_used_at = ? WHERE id = ?",
            now,
            row.id
        )
        .execute(&state.db)
        .await?;
        Ok(TokenUser(row.user_id))
    }
}

/// Numbers may arrive as floats (Shortcuts sums often do); they are rounded.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActivityIn {
    date: String,
    steps: Option<f64>,
    gym_minutes: Option<f64>,
}

pub async fn activity(
    State(s): State<AppState>,
    TokenUser(user_id): TokenUser,
    JsonBody(b): JsonBody<ActivityIn>,
) -> Result<Json<Value>> {
    let date = parse_date(&b.date)?;
    let patch = DayPatch {
        steps: b.steps.map(|v| Some(v.round() as i64)),
        gym_minutes: b.gym_minutes.map(|v| Some(v.round() as i64)),
        ..DayPatch::default()
    };
    if patch.steps.is_none() && patch.gym_minutes.is_none() {
        return Err(Error::bad_request("send steps or gym_minutes"));
    }
    patch.validate().map_err(Error::bad_request)?;
    let today = user_today(&s.db, user_id).await?;
    if !(today.saturating_sub(14.days())..=today.saturating_add(1.day())).contains(&date) {
        return Err(Error::bad_request(
            "date must be within the last 14 days, or tomorrow",
        ));
    }
    let changed = patch_day(&s.db, user_id, date, &patch, Source::Shortcut, now_ms()).await?;
    Ok(Json(json!({ "changed": changed })))
}
