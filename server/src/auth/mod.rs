// SPDX-License-Identifier: AGPL-3.0-or-later
//! First-run setup, login, sessions, login rate limiting, CSRF and client IPs (PLAN.md §10).

use std::time::Instant;

mod limits;
mod password;
mod session;

pub use limits::{ClientIp, RateLimiter, client_ip, csrf};
pub use session::{SqliteStore, session_layer};

use crate::error::JsonBody;
use axum::Json;
use axum::extract::{FromRequestParts, State};
use axum::http::StatusCode;
use axum::http::request::Parts;
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::SqlitePool;
use tower_sessions::Session;

use crate::AppState;
use crate::error::{Error, Result};
use crate::time::now_ms;
use password::{hash_password, verify_password};

pub(super) const USER_KEY: &str = "user_id";
const LOGIN_FAILED: &str = "invalid username or password";

// ---- validation ----

fn validate_username(u: &str) -> Result<()> {
    let ok = (1..=32).contains(&u.len())
        && u.chars()
            .all(|c| c.is_ascii_alphanumeric() || "_.-".contains(c));
    ok.then_some(())
        .ok_or_else(|| Error::bad_request("username must be 1-32 letters, digits, '_', '.' or '-'"))
}

fn validate_password(p: &str) -> Result<()> {
    (8..=1024)
        .contains(&p.chars().count())
        .then_some(())
        .ok_or_else(|| Error::bad_request("password must be at least 8 characters"))
}

pub fn validate_timezone(tz: &str) -> Result<()> {
    jiff::tz::TimeZone::get(tz)
        .map(|_| ())
        .map_err(|_| Error::bad_request(format!("unknown timezone {tz:?}")))
}

// ---- users ----

/// Inserts a user and their profile. With `first_only`, inserts only if there are no users yet,
/// in one statement, so two simultaneous first-run requests can't both succeed.
async fn insert_user(
    db: &SqlitePool,
    username: &str,
    password: &str,
    timezone: &str,
    is_admin: bool,
    first_only: bool,
) -> Result<Option<i64>> {
    validate_username(username)?;
    validate_password(password)?;
    validate_timezone(timezone)?;
    let hash = hash_password(password).await?;
    let now = now_ms();
    let mut tx = db.begin().await?;
    let id = sqlx::query_scalar!(
        "INSERT INTO users (username, password_hash, is_admin, created_at)
         SELECT ?, ?, ?, ? WHERE NOT ? OR NOT EXISTS (SELECT 1 FROM users)
         RETURNING id",
        username,
        hash,
        is_admin,
        now,
        first_only
    )
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| match e {
        sqlx::Error::Database(d) if d.is_unique_violation() => {
            Error::conflict("username already taken")
        }
        e => e.into(),
    })?;
    if let Some(id) = id {
        sqlx::query!(
            "INSERT INTO profiles (user_id, timezone) VALUES (?, ?)",
            id,
            timezone
        )
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(id)
}

pub async fn create_user(
    db: &SqlitePool,
    username: &str,
    password: &str,
    timezone: &str,
    is_admin: bool,
) -> Result<i64> {
    Ok(
        insert_user(db, username, password, timezone, is_admin, false)
            .await?
            .expect("unconditional insert returns an id"),
    )
}

/// For the `reset-password` CLI. Also ends the user's sessions.
pub async fn reset_password(
    db: &SqlitePool,
    username: &str,
    password: &str,
) -> std::result::Result<(), String> {
    validate_password(password).map_err(Error::message)?;
    let hash = hash_password(password).await.map_err(Error::message)?;
    let id = sqlx::query_scalar!(
        "UPDATE users SET password_hash = ? WHERE username = ? RETURNING id",
        hash,
        username
    )
    .fetch_optional(db)
    .await
    .map_err(|e| e.to_string())?
    .ok_or_else(|| format!("no user named {username:?}"))?;
    sqlx::query!("DELETE FROM sessions WHERE user_id = ?", id)
        .execute(db)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// The logged-in user. Extracting it from an anonymous request answers 401.
#[derive(Clone, Copy, Debug)]
pub struct User {
    pub id: i64,
    pub is_admin: bool,
}

impl FromRequestParts<AppState> for User {
    type Rejection = Error;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self> {
        let session = Session::from_request_parts(parts, state)
            .await
            .map_err(|(_, m)| Error::Internal(m.into()))?;
        let id: i64 = session
            .get(USER_KEY)
            .await?
            .ok_or_else(Error::unauthorized)?;
        let user = sqlx::query!("SELECT is_admin FROM users WHERE id = ?", id)
            .fetch_optional(&state.db)
            .await?
            .ok_or_else(Error::unauthorized)?;
        Ok(User {
            id,
            is_admin: user.is_admin != 0,
        })
    }
}

/// The logged-in admin: 401 when anonymous, 403 for other users.
pub struct Admin(pub User);

impl FromRequestParts<AppState> for Admin {
    type Rejection = Error;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self> {
        let user = User::from_request_parts(parts, state).await?;
        if !user.is_admin {
            return Err(Error::Status(StatusCode::FORBIDDEN, "admins only".into()));
        }
        Ok(Admin(user))
    }
}

// ---- handlers ----

pub async fn auth_state(State(s): State<AppState>, session: Session) -> Result<Json<Value>> {
    let needs_setup =
        sqlx::query_scalar!("SELECT NOT EXISTS (SELECT 1 FROM users) AS \"v!: bool\"")
            .fetch_one(&s.db)
            .await?;
    let user = match session.get::<i64>(USER_KEY).await? {
        Some(id) => sqlx::query!(
            "SELECT id, username, is_admin AS \"is_admin: bool\" FROM users WHERE id = ?",
            id
        )
        .fetch_optional(&s.db)
        .await?
        .map(|u| json!({"id": u.id, "username": u.username, "is_admin": u.is_admin})),
        None => None,
    };
    Ok(Json(json!({ "needs_setup": needs_setup, "user": user })))
}

#[derive(Deserialize)]
pub struct SetupBody {
    username: String,
    password: String,
    timezone: String,
}

pub async fn setup(
    State(s): State<AppState>,
    session: Session,
    JsonBody(b): JsonBody<SetupBody>,
) -> Result<Json<Value>> {
    let id = insert_user(&s.db, &b.username, &b.password, &b.timezone, true, true)
        .await?
        .ok_or_else(|| Error::conflict("setup is already done"))?;
    log_in(&session, id).await?;
    Ok(Json(json!({ "id": id })))
}

#[derive(Deserialize)]
pub struct LoginBody {
    username: String,
    password: String,
}

pub async fn login(
    State(s): State<AppState>,
    session: Session,
    ip: ClientIp,
    JsonBody(b): JsonBody<LoginBody>,
) -> Result<Json<Value>> {
    let now = Instant::now();
    let by_name = s
        .login_limiter
        .allow(&format!("user:{}", b.username.to_lowercase()), now);
    let by_ip =
        ip.0.is_none_or(|ip| s.login_limiter.allow(&format!("ip:{ip}"), now));
    if !(by_name && by_ip) {
        return Err(Error::Status(
            StatusCode::TOO_MANY_REQUESTS,
            "too many login attempts; wait a minute".into(),
        ));
    }
    let user = sqlx::query!(
        "SELECT id, password_hash FROM users WHERE username = ?",
        b.username
    )
    .fetch_optional(&s.db)
    .await?;
    let (id, hash) = user.map_or((None, None), |u| (Some(u.id), u.password_hash));
    if !verify_password(&b.password, hash).await {
        return Err(Error::Status(StatusCode::UNAUTHORIZED, LOGIN_FAILED.into()));
    }
    let id = id.expect("verified password implies a user");
    log_in(&session, id).await?;
    Ok(Json(json!({ "id": id })))
}

/// New session id at every login, against fixation.
async fn log_in(session: &Session, user_id: i64) -> Result<()> {
    session.cycle_id().await?;
    session.insert(USER_KEY, user_id).await?;
    Ok(())
}

pub async fn logout(session: Session) -> Result<Json<Value>> {
    session.flush().await?;
    Ok(Json(json!({})))
}
