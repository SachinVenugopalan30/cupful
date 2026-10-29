// SPDX-License-Identifier: AGPL-3.0-or-later
//! Google Health API v4 steps sync (PLAN.md §15). OAuth code flow with PKCE; tokens encrypted at
//! rest. Never logs tokens, codes or response bodies. Every route answers 404 unless
//! `CUPFUL_GOOGLE_CLIENT_ID` and `CUPFUL_GOOGLE_CLIENT_SECRET` are set.

use std::time::{Duration, Instant};

use axum::Json;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::Redirect;
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD as B64URL;
use chacha20poly1305::Key;
use chacha20poly1305::aead::common::Generate;
use jiff::ToSpan;
use jiff::civil::Date;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tower_sessions::Session;

use crate::AppState;
use crate::auth::User;
use crate::error::{Error, Query, Result};
use crate::ingest::rfc3339;
use crate::sync::{DayPatch, Source, patch_day};
use crate::time::{now_ms, user_today};

pub const SCOPE: &str =
    "https://www.googleapis.com/auth/googlehealth.activity_and_fitness.readonly";
const SESSION_KEY: &str = "google_oauth";
/// Days synced each time, today included; a phone that syncs late still lands in range.
const WINDOW_DAYS: i8 = 14;

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

fn random() -> String {
    B64URL.encode(Key::generate().as_slice())
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
    Ok(Json(match row {
        None => json!({ "configured": true, "connected": false }),
        Some(r) => json!({
            "configured": true,
            "connected": true,
            "needs_reconnect": r.needs_reconnect != 0,
            "last_sync_at": r.last_sync_at.map(rfc3339).transpose()?,
            "last_error": r.last_error,
        }),
    }))
}

// ---- OAuth ----

#[derive(Serialize, Deserialize)]
struct Pending {
    state: String,
    verifier: String,
}

pub async fn connect(
    State(s): State<AppState>,
    _user: User,
    session: Session,
    headers: HeaderMap,
) -> Result<Redirect> {
    let (client_id, _) = creds(&s)?;
    let pending = Pending {
        state: random(),
        verifier: random(),
    };
    let challenge = B64URL.encode(Sha256::digest(pending.verifier.as_bytes()));
    let url = format!(
        "{}?{}",
        s.config.google_auth_url,
        form(&[
            ("client_id", client_id),
            ("redirect_uri", &redirect_uri(&s, &headers)?),
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
struct TokenReply {
    access_token: Option<String>,
    refresh_token: Option<String>,
    expires_in: Option<i64>,
    error: Option<String>,
}

enum TokenError {
    /// The refresh token or code is dead; the user must connect again.
    InvalidGrant,
    Other(String),
}

async fn token_request(
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

async fn save_tokens(
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
    headers: HeaderMap,
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
    let uri = redirect_uri(&s, &headers)?;
    let tokens = token_request(
        &s,
        &[
            ("grant_type", "authorization_code"),
            ("code", &code),
            ("code_verifier", &pending.verifier),
            ("redirect_uri", &uri),
        ],
    )
    .await;
    match tokens {
        Ok(t) if t.refresh_token.is_some() => {
            save_tokens(&s, user.id, &t, t.refresh_token.as_deref()).await?;
            Ok(Redirect::to("/settings?google=connected"))
        }
        Ok(_) => Ok(Redirect::to("/settings?google=error")),
        Err(TokenError::InvalidGrant) | Err(TokenError::Other(_)) => {
            tracing::warn!("google code exchange failed");
            Ok(Redirect::to("/settings?google=error"))
        }
    }
}

// ---- sync ----

#[derive(Deserialize)]
struct Rollups {
    #[serde(default, rename = "rollupDataPoints")]
    points: Vec<Point>,
}
#[derive(Deserialize)]
struct Point {
    #[serde(rename = "civilStartTime")]
    start: Civil,
    steps: Option<StepsValue>,
}
#[derive(Deserialize)]
struct Civil {
    date: GDate,
}
#[derive(Deserialize)]
struct GDate {
    year: i16,
    month: i8,
    day: i8,
}
#[derive(Deserialize)]
struct StepsValue {
    /// int64, so a string in JSON.
    #[serde(rename = "countSum")]
    count_sum: String,
}

fn civil(d: Date) -> Value {
    json!({ "date": { "year": d.year(), "month": d.month(), "day": d.day() } })
}

/// Records why a sync failed; `reconnect` stops the daily retries until the user connects again.
async fn fail(s: &AppState, user_id: i64, msg: &str, reconnect: bool) -> Result<Error> {
    sqlx::query!(
        "UPDATE google_connections SET last_error = ?, needs_reconnect = needs_reconnect OR ?
         WHERE user_id = ?",
        msg,
        reconnect,
        user_id
    )
    .execute(&s.db)
    .await?;
    Ok(if reconnect {
        Error::Coded(StatusCode::CONFLICT, "reconnect", msg.into())
    } else {
        Error::Status(StatusCode::BAD_GATEWAY, msg.into())
    })
}

const RECONNECT_MSG: &str = "Google access ended; reconnect Google Health";

async fn access_token(s: &AppState, user_id: i64) -> Result<String> {
    let row = sqlx::query!(
        "SELECT refresh_token, access_token, access_expires_at, needs_reconnect
         FROM google_connections WHERE user_id = ?",
        user_id
    )
    .fetch_optional(&s.db)
    .await?
    .ok_or_else(|| Error::conflict("Google Health is not connected"))?;
    if row.needs_reconnect != 0 {
        return Err(Error::Coded(
            StatusCode::CONFLICT,
            "reconnect",
            RECONNECT_MSG.into(),
        ));
    }
    let open = |v: &str| s.secrets.decrypt(v).map_err(Error::Internal);
    if let (Some(a), Some(exp)) = (&row.access_token, row.access_expires_at)
        && exp - 60_000 > now_ms()
    {
        return open(a);
    }
    let refresh = open(&row.refresh_token)?;
    match token_request(
        s,
        &[("grant_type", "refresh_token"), ("refresh_token", &refresh)],
    )
    .await
    {
        Ok(t) => {
            save_tokens(s, user_id, &t, None).await?;
            Ok(t.access_token.unwrap_or_default())
        }
        Err(TokenError::InvalidGrant) => Err(fail(s, user_id, RECONNECT_MSG, true).await?),
        Err(TokenError::Other(m)) => Err(fail(s, user_id, &m, false).await?),
    }
}

/// Pulls the last two weeks of daily steps into the user's days. Returns (days seen, days changed).
pub async fn sync_user(s: &AppState, user_id: i64) -> Result<(usize, usize)> {
    let token = access_token(s, user_id).await?;
    let today = user_today(&s.db, user_id).await?;
    let body = json!({
        "range": {
            "start": civil(today.saturating_sub(i64::from(WINDOW_DAYS - 1).days())),
            "end": civil(today.saturating_add(1.day())),
        },
        "windowSizeDays": 1,
        "pageSize": 90,
    });
    // ponytail: one page; 14 windows never reach the 90-point page size, so nextPageToken is ignored
    let res = s
        .google_http
        .post(format!(
            "{}/users/me/dataTypes/steps/dataPoints:dailyRollUp",
            s.config.google_health_url
        ))
        .bearer_auth(&token)
        .header("content-type", "application/json")
        .body(body.to_string())
        .send()
        .await;
    let res = match res {
        Ok(r) => r,
        Err(_) => return Err(fail(s, user_id, "could not reach Google Health", false).await?),
    };
    let status = res.status();
    if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
        return Err(fail(
            s,
            user_id,
            "Google denied access (missing permission); reconnect Google Health",
            true,
        )
        .await?);
    }
    if !status.is_success() {
        return Err(fail(
            s,
            user_id,
            &format!("Google Health answered {status}"),
            false,
        )
        .await?);
    }
    let rollups: Rollups = match read_json(res).await {
        Some(r) => r,
        None => return Err(fail(s, user_id, "unreadable reply from Google Health", false).await?),
    };
    let (mut seen, mut changed) = (0, 0);
    let now = now_ms();
    for p in rollups.points {
        let (Some(steps), Ok(date)) = (
            p.steps.and_then(|v| v.count_sum.parse::<i64>().ok()),
            Date::new(p.start.date.year, p.start.date.month, p.start.date.day),
        ) else {
            continue;
        };
        let patch = DayPatch {
            steps: Some(Some(steps)),
            ..DayPatch::default()
        };
        if patch.validate().is_err() {
            continue;
        }
        seen += 1;
        changed += usize::from(patch_day(&s.db, user_id, date, &patch, Source::Google, now).await?);
    }
    sqlx::query!(
        "UPDATE google_connections SET last_sync_at = ?, last_error = NULL WHERE user_id = ?",
        now,
        user_id
    )
    .execute(&s.db)
    .await?;
    Ok((seen, changed))
}

pub async fn sync_now(State(s): State<AppState>, user: User) -> Result<Json<Value>> {
    creds(&s)?;
    // ponytail: shares the ingest limiter's 60/hour; a dedicated limit if this ever matters
    if !s
        .ingest_limiter
        .allow(&format!("google:{}", user.id), Instant::now())
    {
        return Err(Error::Status(
            StatusCode::TOO_MANY_REQUESTS,
            "too many syncs; try again later".into(),
        ));
    }
    let (days, changed) = sync_user(&s, user.id).await?;
    Ok(Json(json!({ "days": days, "changed": changed })))
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
