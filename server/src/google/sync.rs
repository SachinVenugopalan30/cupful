// SPDX-License-Identifier: AGPL-3.0-or-later
//! Google Health v4 steps sync: access tokens, daily rollups, `sync_now`.

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use jiff::ToSpan;
use jiff::civil::Date;
use serde::Deserialize;
use serde_json::{Value, json};

use super::oauth::{TokenError, save_tokens, token_request};
use super::{creds, read_json};
use crate::AppState;
use crate::auth::User;
use crate::error::{Error, Result};
use crate::sync::{DayPatch, Source, WINDOW_DAYS, patch_day};
use crate::time::{now_ms, user_today};

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

/// Turns a Google reply into the response to read, or records why the sync failed.
pub(super) async fn checked(
    s: &AppState,
    user_id: i64,
    res: reqwest::Result<reqwest::Response>,
) -> Result<reqwest::Response> {
    let Ok(res) = res else {
        return Err(fail(s, user_id, "could not reach Google Health", false).await?);
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
    Ok(res)
}

pub(super) async fn unreadable(s: &AppState, user_id: i64) -> Result<Error> {
    fail(s, user_id, "unreadable reply from Google Health", false).await
}

pub struct Synced {
    pub days: usize,
    pub changed: usize,
    pub workouts: usize,
    /// Set when only the exercise part failed; steps still synced.
    pub workouts_error: Option<String>,
}

/// Pulls the last two weeks of steps and workouts. The parts fail independently: a steps error
/// is returned (workouts are still stored), an exercise error is reported in `Synced`, except
/// a reconnect need, which always wins.
pub async fn sync_user(s: &AppState, user_id: i64) -> Result<Synced> {
    let token = access_token(s, user_id).await?;
    let today = user_today(&s.db, user_id).await?;
    let steps = sync_steps(s, user_id, &token, today).await;
    let workouts = super::workouts::sync(s, user_id, &token, today).await;
    let (days, changed) = steps?;
    let mut out = Synced {
        days,
        changed,
        workouts: 0,
        workouts_error: None,
    };
    match workouts {
        Ok(n) => out.workouts = n,
        Err(Error::Coded(StatusCode::CONFLICT, "reconnect", m)) => {
            return Err(Error::Coded(StatusCode::CONFLICT, "reconnect", m));
        }
        Err(e) => out.workouts_error = Some(e.message()),
    }
    // fail() already stored an exercise error; only a clean run clears it.
    sqlx::query!(
        "UPDATE google_connections SET last_sync_at = ?,
             last_error = CASE WHEN ? THEN last_error END WHERE user_id = ?",
        now_ms(),
        out.workouts_error.is_some(),
        user_id
    )
    .execute(&s.db)
    .await?;
    Ok(out)
}

/// Daily steps into the user's days. Returns (days seen, days changed).
async fn sync_steps(
    s: &AppState,
    user_id: i64,
    token: &str,
    today: Date,
) -> Result<(usize, usize)> {
    let body = json!({
        "range": {
            "start": civil(today.saturating_sub((WINDOW_DAYS - 1).days())),
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
        .bearer_auth(token)
        .header("content-type", "application/json")
        .body(body.to_string())
        .send()
        .await;
    let res = checked(s, user_id, res).await?;
    let Some(rollups) = read_json::<Rollups>(res).await else {
        return Err(unreadable(s, user_id).await?);
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
    Ok((seen, changed))
}

pub async fn sync_now(State(s): State<AppState>, user: User) -> Result<Json<Value>> {
    creds(&s)?;
    // ponytail: shares the ingest limiter's 60/hour; a dedicated limit if this ever matters
    s.rate_limit(
        &format!("google:{}", user.id),
        "too many syncs; try again later",
    )?;
    let r = sync_user(&s, user.id).await?;
    let mut out = json!({ "days": r.days, "changed": r.changed, "workouts": r.workouts });
    if let Some(e) = r.workouts_error {
        out["workouts_error"] = e.into();
    }
    Ok(Json(out))
}
