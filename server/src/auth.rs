// SPDX-License-Identifier: AGPL-3.0-or-later
//! First-run setup, login, sessions, login rate limiting, CSRF and client IPs (PLAN.md §10).

use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use argon2::Argon2;
use argon2::password_hash::phc::PasswordHash;
use argon2::password_hash::{PasswordHasher, PasswordVerifier};
use async_trait::async_trait;
use axum::Json;
use axum::extract::{ConnectInfo, FromRequestParts, Request, State};
use axum::http::request::Parts;
use axum::http::{Method, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::SqlitePool;
use tower_sessions::session::{Id, Record};
use tower_sessions::session_store::{self, SessionStore};
use tower_sessions::{Expiry, Session, SessionManagerLayer, cookie};

use crate::AppState;
use crate::error::{Error, Result};

const USER_KEY: &str = "user_id";
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

// ---- passwords ----

async fn hash_password(password: &str) -> Result<String> {
    let password = password.to_owned();
    tokio::task::spawn_blocking(move || {
        Argon2::default()
            .hash_password(password.as_bytes())
            .map(|h| h.to_string())
            .map_err(|e| Error::Internal(e.to_string()))
    })
    .await
    .map_err(|e| Error::Internal(e.to_string()))?
}

/// Verifies against `hash`, or against a dummy hash when the user doesn't exist, so both cases take as long.
async fn verify_password(password: &str, hash: Option<String>) -> bool {
    static DUMMY: OnceLock<String> = OnceLock::new();
    let found = hash.is_some();
    let hash = hash.unwrap_or_else(|| {
        DUMMY
            .get_or_init(|| {
                Argon2::default()
                    .hash_password(b"dummy password")
                    .unwrap()
                    .to_string()
            })
            .clone()
    });
    let password = password.to_owned();
    let ok = tokio::task::spawn_blocking(move || {
        PasswordHash::new(&hash).is_ok_and(|h| {
            Argon2::default()
                .verify_password(password.as_bytes(), &h)
                .is_ok()
        })
    })
    .await
    .unwrap_or(false);
    ok && found
}

// ---- users ----

fn now_ms() -> i64 {
    jiff::Timestamp::now().as_millisecond()
}

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
    let msg = |e: Error| match e {
        Error::Status(_, m) | Error::Internal(m) => m,
    };
    validate_password(password).map_err(msg)?;
    let hash = hash_password(password).await.map_err(msg)?;
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
    Json(b): Json<SetupBody>,
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
    Json(b): Json<LoginBody>,
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

// ---- CSRF ----

/// Mutating API requests must carry `X-Cupful: 1`, on top of SameSite=Lax cookies.
pub async fn csrf(req: Request, next: Next) -> Response {
    let safe = matches!(*req.method(), Method::GET | Method::HEAD | Method::OPTIONS);
    if !safe
        && req.uri().path().starts_with("/api/")
        && req.headers().get("x-cupful").is_none_or(|v| v != "1")
    {
        return Error::Status(StatusCode::FORBIDDEN, "missing X-Cupful header".into())
            .into_response();
    }
    next.run(req).await
}

// ---- client IP and rate limiting ----

/// The client's IP, when it is known. `X-Forwarded-For` is trusted only from the configured proxy,
/// and then only its rightmost entry (the one that proxy appended; the rest are client-controlled).
/// Loopback counts as unknown: behind the Tailscale sidecar every request comes from 127.0.0.1.
pub fn client_ip(
    peer: Option<IpAddr>,
    xff: Option<&str>,
    trusted: Option<IpAddr>,
) -> Option<IpAddr> {
    let peer = peer?;
    let ip = if Some(peer) == trusted {
        xff?.rsplit(',').next()?.trim().parse().ok()?
    } else {
        peer
    };
    (!ip.is_loopback()).then_some(ip)
}

pub struct ClientIp(pub Option<IpAddr>);

impl FromRequestParts<AppState> for ClientIp {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> std::result::Result<Self, Self::Rejection> {
        let peer = parts
            .extensions
            .get::<ConnectInfo<SocketAddr>>()
            .map(|c| c.0.ip());
        let xff = parts
            .headers
            .get("x-forwarded-for")
            .and_then(|v| v.to_str().ok());
        Ok(ClientIp(client_ip(peer, xff, state.config.trusted_proxy)))
    }
}

/// Sliding-window limiter: at most `limit` hits per key per `window`.
pub struct RateLimiter {
    hits: Mutex<HashMap<String, Vec<Instant>>>,
    limit: usize,
    window: Duration,
}

impl RateLimiter {
    pub fn new(limit: usize, window: Duration) -> Self {
        Self {
            hits: Mutex::default(),
            limit,
            window,
        }
    }

    /// Records a hit and says whether it's within the limit.
    pub fn allow(&self, key: &str, now: Instant) -> bool {
        let mut hits = self.hits.lock().unwrap();
        // ponytail: in-memory and per-process; fine for one binary, resets on restart.
        if hits.len() > 10_000 {
            hits.retain(|_, v| v.last().is_some_and(|t| now - *t < self.window));
        }
        let v = hits.entry(key.to_owned()).or_default();
        v.retain(|t| now - *t < self.window);
        if v.len() >= self.limit {
            return false;
        }
        v.push(now);
        true
    }
}

// ---- sessions ----

pub fn session_layer(db: SqlitePool, secure: bool) -> SessionManagerLayer<SqliteStore> {
    SessionManagerLayer::new(SqliteStore(db))
        .with_name("cupful_session")
        .with_http_only(true)
        .with_secure(secure)
        .with_same_site(cookie::SameSite::Lax)
        .with_expiry(Expiry::OnInactivity(cookie::time::Duration::days(30)))
        .with_always_save(true)
}

/// tower-sessions store on our pool. (tower-sessions-sqlx-store still targets sqlx 0.8.)
#[derive(Clone, Debug)]
pub struct SqliteStore(SqlitePool);

fn backend(e: impl std::fmt::Display) -> session_store::Error {
    session_store::Error::Backend(e.to_string())
}

impl SqliteStore {
    async fn upsert(&self, r: &Record, replace: bool) -> session_store::Result<bool> {
        let id = r.id.to_string();
        let user_id = r.data.get(USER_KEY).and_then(Value::as_i64);
        let data = serde_json::to_string(&r.data).map_err(backend)?;
        let expiry = r.expiry_date.unix_timestamp();
        let res = sqlx::query!(
            "INSERT INTO sessions (id, user_id, data, expiry_date) VALUES (?, ?, ?, ?)
             ON CONFLICT (id) DO UPDATE SET user_id = excluded.user_id, data = excluded.data,
                 expiry_date = excluded.expiry_date
             WHERE ?",
            id,
            user_id,
            data,
            expiry,
            replace
        )
        .execute(&self.0)
        .await
        .map_err(backend)?;
        Ok(res.rows_affected() == 1)
    }
}

#[async_trait]
impl SessionStore for SqliteStore {
    async fn create(&self, record: &mut Record) -> session_store::Result<()> {
        while !self.upsert(record, false).await? {
            record.id = Id::default();
        }
        Ok(())
    }

    async fn save(&self, record: &Record) -> session_store::Result<()> {
        self.upsert(record, true).await.map(|_| ())
    }

    async fn load(&self, id: &Id) -> session_store::Result<Option<Record>> {
        let id_str = id.to_string();
        let now = jiff::Timestamp::now().as_second();
        let Some(row) = sqlx::query!(
            "SELECT data, expiry_date FROM sessions WHERE id = ? AND expiry_date > ?",
            id_str,
            now
        )
        .fetch_optional(&self.0)
        .await
        .map_err(backend)?
        else {
            return Ok(None);
        };
        Ok(Some(Record {
            id: *id,
            data: serde_json::from_str(&row.data).map_err(backend)?,
            expiry_date: cookie::time::OffsetDateTime::from_unix_timestamp(row.expiry_date)
                .map_err(backend)?,
        }))
    }

    async fn delete(&self, id: &Id) -> session_store::Result<()> {
        let id = id.to_string();
        sqlx::query!("DELETE FROM sessions WHERE id = ?", id)
            .execute(&self.0)
            .await
            .map_err(backend)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PROXY: &str = "10.0.0.1";

    fn ip(s: &str) -> Option<IpAddr> {
        Some(s.parse().unwrap())
    }

    #[test]
    fn no_trusted_proxy_uses_the_peer_and_ignores_xff() {
        assert_eq!(
            client_ip(ip("100.64.0.7"), Some("1.2.3.4"), None),
            ip("100.64.0.7")
        );
    }

    #[test]
    fn trusted_proxy_uses_the_rightmost_xff_entry() {
        assert_eq!(
            client_ip(ip(PROXY), Some("6.6.6.6, 203.0.113.9"), ip(PROXY)),
            ip("203.0.113.9")
        );
    }

    #[test]
    fn spoofed_leading_entry_is_ignored() {
        assert_ne!(
            client_ip(ip(PROXY), Some("6.6.6.6, 203.0.113.9"), ip(PROXY)),
            ip("6.6.6.6")
        );
    }

    #[test]
    fn untrusted_source_cannot_set_xff() {
        assert_eq!(
            client_ip(ip("198.51.100.4"), Some("203.0.113.9"), ip(PROXY)),
            ip("198.51.100.4")
        );
    }

    #[test]
    fn loopback_and_missing_peers_are_unknown() {
        assert_eq!(client_ip(ip("127.0.0.1"), None, None), None);
        assert_eq!(client_ip(None, Some("203.0.113.9"), ip(PROXY)), None);
        assert_eq!(client_ip(ip(PROXY), None, ip(PROXY)), None);
    }

    #[test]
    fn limiter_allows_five_per_window_then_recovers() {
        let l = RateLimiter::new(5, Duration::from_secs(60));
        let t0 = Instant::now();
        assert!((0..5).all(|_| l.allow("k", t0)));
        assert!(!l.allow("k", t0 + Duration::from_secs(30)));
        assert!(l.allow("other", t0));
        assert!(l.allow("k", t0 + Duration::from_secs(61)));
    }
}
