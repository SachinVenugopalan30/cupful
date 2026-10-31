// SPDX-License-Identifier: AGPL-3.0-or-later
//! Google Health exercise sessions: fetch and store, the workouts API, and gym minutes derived
//! from the sessions whose type the user counts as gym. Field and enum names are from
//! <https://developers.google.com/health/reference/rest/v4/users.dataTypes.dataPoints>.

use std::collections::BTreeMap;

use axum::Json;
use axum::extract::State;
use jiff::ToSpan;
use jiff::civil::Date;
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::SqlitePool;

use super::sync::{checked, unreadable};
use super::{creds, read_json};
use crate::AppState;
use crate::auth::User;
use crate::error::{Error, JsonBody, Query, Result};
use crate::reads::{Range, check_span};
use crate::sync::{DayPatch, Source, WINDOW_DAYS, patch_day};
use crate::time::{now_ms, parse_date, rfc3339};

/// Types counted as gym minutes until the user chooses otherwise: strength work plus the
/// indoor machines. Outdoor running, walking, hiking and biking are left out.
const DEFAULT_GYM: &[&str] = &[
    "STRENGTH_TRAINING",
    "FUNCTIONAL_STRENGTH_TRAINING",
    "WEIGHTLIFTING",
    "POWERLIFTING",
    "WEIGHT_MACHINES",
    "FREE_WEIGHTS",
    "WEIGHTS",
    "BODY_WEIGHT",
    "RESISTANCE_BANDS",
    "TRX",
    "CALISTHENICS",
    "HIIT",
    "CIRCUIT_TRAINING",
    "CROSSFIT",
    "TREADMILL",
    "ELLIPTICAL",
    "ROWING_MACHINE",
    "STAIRCLIMBER",
    "SPINNING",
    "STATIONARY_BIKE",
    "ASSAULT_BIKE",
];
/// Also pickable: other gym-floor and cardio types. Every type present in the user's stored
/// workouts is pickable too (see `pickable`). All names are `Exercise.ExerciseType` values.
const OTHER_PICKABLE: &[&str] = &[
    "RUNNING",
    "TRAIL_RUN",
    "INCLINE_RUN",
    "WALKING",
    "INCLINE_WALK",
    "TREADMILL_WALK",
    "NORDIC_WALKING",
    "POWER_WALKING",
    "HIKING",
    "BIKING",
    "MOUNTAIN_BIKE",
    "ROWING",
    "STEP_TRAINING",
    "CORE_TRAINING",
    "BOOTCAMP",
    "CARDIO_WORKOUT",
    "INTERVAL_WORKOUT",
    "TABATA_WORKOUT",
    "CROSS_TRAINING",
    "WALK_WITH_WEIGHTS",
    "SWIMMING",
    "YOGA",
    "PILATES",
];
const MAX_GYM_TYPES: usize = 30;
const PAGE_SIZE: &str = "25";
/// ponytail: 20 pages x 25 = 500 workouts per sync, far above 14 days of training; raise if a
/// window ever needs more (the oldest sessions are the ones cut, newest come first).
const MAX_PAGES: usize = 20;

/// "STRENGTH_TRAINING" -> "Strength training".
fn label(t: &str) -> String {
    if t == "HIIT" {
        return t.into();
    }
    let mut s = t.replace('_', " ").to_lowercase();
    s[..1].make_ascii_uppercase();
    s
}

/// The curated list plus every type in the user's stored workouts, sorted by label.
async fn pickable(db: &SqlitePool, user_id: i64) -> Result<Vec<String>> {
    let stored = sqlx::query_scalar!(
        "SELECT DISTINCT exercise_type FROM workouts WHERE user_id = ?",
        user_id
    )
    .fetch_all(db)
    .await?;
    let mut all: Vec<String> = DEFAULT_GYM
        .iter()
        .chain(OTHER_PICKABLE)
        .map(|t| t.to_string())
        .chain(stored)
        .collect();
    all.sort_by_key(|t| (label(t), t.clone()));
    all.dedup();
    Ok(all)
}

async fn gym_set(db: &SqlitePool, user_id: i64) -> Result<Vec<String>> {
    let raw = sqlx::query_scalar!("SELECT gym_types FROM profiles WHERE user_id = ?", user_id)
        .fetch_one(db)
        .await?;
    Ok(raw
        .and_then(|j| serde_json::from_str(&j).ok())
        .unwrap_or_else(|| DEFAULT_GYM.iter().map(|t| t.to_string()).collect()))
}

/// Adds the gym-type preference and its choices to the Google status.
pub(super) async fn extend_status(db: &SqlitePool, user_id: i64, status: &mut Value) -> Result<()> {
    status["gym_types"] = json!(gym_set(db, user_id).await?);
    status["exercise_types"] = pickable(db, user_id)
        .await?
        .iter()
        .map(|t| json!({ "value": t, "label": label(t) }))
        .collect();
    Ok(())
}

// ---- fetch ----

#[derive(Deserialize)]
struct Page {
    #[serde(default, rename = "dataPoints")]
    points: Vec<Point>,
    #[serde(default, rename = "nextPageToken")]
    next: Option<String>,
}
#[derive(Deserialize)]
struct Point {
    #[serde(default)]
    name: String,
    exercise: Option<Exercise>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Exercise {
    interval: Interval,
    exercise_type: String,
    #[serde(default)]
    display_name: String,
    #[serde(default)]
    metrics_summary: Metrics,
    /// Duration string such as "1860s", excluding pauses.
    active_duration: Option<String>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Interval {
    start_time: String,
    end_time: String,
}
#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Metrics {
    calories_kcal: Option<f64>,
    distance_millimeters: Option<f64>,
    /// int64, so a string in JSON.
    average_heart_rate_beats_per_minute: Option<String>,
}

fn ms(t: &str) -> Option<i64> {
    t.parse::<jiff::Timestamp>()
        .ok()
        .map(|t| t.as_millisecond())
}

/// Fetches the last 14 days of sessions (newest first, 25 per page) and upserts them, then
/// refreshes gym minutes. Returns how many sessions were stored.
pub(super) async fn sync(s: &AppState, user_id: i64, token: &str, today: Date) -> Result<usize> {
    let tz = sqlx::query_scalar!("SELECT timezone FROM profiles WHERE user_id = ?", user_id)
        .fetch_one(&s.db)
        .await?;
    let tz = jiff::tz::TimeZone::get(&tz).map_err(|e| Error::Internal(e.to_string()))?;
    let filter = format!(
        r#"exercise.interval.civil_start_time >= "{}" AND exercise.interval.civil_start_time < "{}""#,
        today.saturating_sub((WINDOW_DAYS - 1).days()),
        today.saturating_add(1.day())
    );
    let mut url = reqwest::Url::parse(&format!(
        "{}/users/me/dataTypes/exercise/dataPoints",
        s.config.google_health_url
    ))
    .map_err(|e| Error::Internal(e.to_string()))?;
    let mut stored = 0;
    let mut page_token = String::new();
    for _ in 0..MAX_PAGES {
        {
            let mut q = url.query_pairs_mut();
            q.clear()
                .append_pair("pageSize", PAGE_SIZE)
                .append_pair("filter", &filter);
            if !page_token.is_empty() {
                q.append_pair("pageToken", &page_token);
            }
        }
        let res = s
            .google_http
            .get(url.clone())
            .bearer_auth(token)
            .send()
            .await;
        let res = checked(s, user_id, res).await?;
        let Some(page) = read_json::<Page>(res).await else {
            return Err(unreadable(s, user_id).await?);
        };
        for p in page.points {
            let Some(e) = p.exercise else { continue };
            let (Some(start), Some(end)) = (ms(&e.interval.start_time), ms(&e.interval.end_time))
            else {
                continue;
            };
            // Google's id when the point has one; else start and type, which a re-sync repeats.
            let external_id = if p.name.is_empty() {
                format!("{start}:{}", e.exercise_type)
            } else {
                p.name
            };
            let date = jiff::Timestamp::from_millisecond(start)
                .map_err(|e| Error::Internal(e.to_string()))?
                .to_zoned(tz.clone())
                .date()
                .to_string();
            let active = e
                .active_duration
                .as_deref()
                .and_then(|d| d.strip_suffix('s')?.parse::<f64>().ok())
                .map_or((end - start) / 1000, |secs| secs.round() as i64);
            let name = if e.display_name.is_empty() {
                label(&e.exercise_type)
            } else {
                e.display_name
            };
            let distance_m = e.metrics_summary.distance_millimeters.map(|mm| mm / 1000.0);
            let avg_hr = e
                .metrics_summary
                .average_heart_rate_beats_per_minute
                .and_then(|h| h.parse::<i64>().ok());
            sqlx::query!(
                "INSERT INTO workouts (user_id, external_id, start_ms, end_ms, date, exercise_type,
                     name, active_seconds, kcal, distance_m, avg_hr)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                 ON CONFLICT (user_id, external_id) DO UPDATE SET
                     start_ms = excluded.start_ms, end_ms = excluded.end_ms, date = excluded.date,
                     exercise_type = excluded.exercise_type, name = excluded.name,
                     active_seconds = excluded.active_seconds, kcal = excluded.kcal,
                     distance_m = excluded.distance_m, avg_hr = excluded.avg_hr",
                user_id,
                external_id,
                start,
                end,
                date,
                e.exercise_type,
                name,
                active,
                e.metrics_summary.calories_kcal,
                distance_m,
                avg_hr
            )
            .execute(&s.db)
            .await?;
            stored += 1;
        }
        match page.next {
            Some(t) if !t.is_empty() => page_token = t,
            _ => break,
        }
    }
    recompute_gym(&s.db, user_id).await?;
    Ok(stored)
}

/// Writes gym minutes (Google source, so manual values win) for every day that has a stored
/// workout of a gym type, and clears Google-written gym minutes on days that no longer have
/// one. Other days are left alone.
// ponytail: walks every stored workout day each time; fine for years of data, narrow to a
// date range if history grows past a few thousand sessions
async fn recompute_gym(db: &SqlitePool, user_id: i64) -> Result<()> {
    let gym = gym_set(db, user_id).await?;
    let rows = sqlx::query!(
        "SELECT date, exercise_type, active_seconds FROM workouts WHERE user_id = ?",
        user_id
    )
    .fetch_all(db)
    .await?;
    let mut secs: BTreeMap<String, i64> = BTreeMap::new();
    for r in rows.into_iter().filter(|r| gym.contains(&r.exercise_type)) {
        *secs.entry(r.date).or_default() += r.active_seconds;
    }
    let stale = sqlx::query_scalar!(
        "SELECT date FROM days WHERE user_id = ? AND gym_source = 'google'",
        user_id
    )
    .fetch_all(db)
    .await?;
    let now = now_ms();
    for date in stale.into_iter().filter(|d| !secs.contains_key(d)) {
        let clear = DayPatch {
            gym_minutes: Some(None),
            ..DayPatch::default()
        };
        patch_day(db, user_id, parse_date(&date)?, &clear, Source::Google, now).await?;
    }
    for (date, s) in secs {
        let patch = DayPatch {
            gym_minutes: Some(Some(((s as f64 / 60.0).round() as i64).clamp(1, 600))),
            ..DayPatch::default()
        };
        patch_day(db, user_id, parse_date(&date)?, &patch, Source::Google, now).await?;
    }
    Ok(())
}

// ---- API ----

/// Stored workouts; works whether or not Google is configured.
pub async fn list(
    State(s): State<AppState>,
    user: User,
    Query(r): Query<Range>,
) -> Result<Json<Value>> {
    let (from, to) = (parse_date(&r.from)?, parse_date(&r.to)?);
    check_span(from, to)?;
    let (from, to) = (from.to_string(), to.to_string());
    let gym = gym_set(&s.db, user.id).await?;
    let rows = sqlx::query!(
        "SELECT id, date, start_ms, end_ms, exercise_type, name, active_seconds, kcal, distance_m,
                avg_hr
         FROM workouts WHERE user_id = ? AND date BETWEEN ? AND ? ORDER BY start_ms, id",
        user.id,
        from,
        to
    )
    .fetch_all(&s.db)
    .await?;
    let workouts = rows
        .into_iter()
        .map(|w| {
            Ok(json!({
                "id": w.id,
                "date": w.date,
                "start": rfc3339(w.start_ms)?,
                "end": rfc3339(w.end_ms)?,
                "type": w.exercise_type,
                "type_label": label(&w.exercise_type),
                "name": w.name,
                "active_minutes": w.active_seconds as f64 / 60.0,
                "kcal": w.kcal,
                "distance_m": w.distance_m,
                "avg_hr": w.avg_hr,
                "counts_as_gym": gym.contains(&w.exercise_type),
            }))
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(Json(json!({ "workouts": workouts })))
}

#[derive(Deserialize)]
pub struct GymTypes {
    types: Vec<String>,
}

pub async fn set_gym_types(
    State(s): State<AppState>,
    user: User,
    JsonBody(b): JsonBody<GymTypes>,
) -> Result<Json<Value>> {
    creds(&s)?;
    let ok = pickable(&s.db, user.id).await?;
    if b.types.len() > MAX_GYM_TYPES || !b.types.iter().all(|t| ok.contains(t)) {
        return Err(Error::bad_request("unknown or too many exercise types"));
    }
    let mut types = b.types;
    types.sort();
    types.dedup();
    sqlx::query!(
        "UPDATE profiles SET gym_types = ? WHERE user_id = ?",
        json!(types).to_string(),
        user.id
    )
    .execute(&s.db)
    .await?;
    recompute_gym(&s.db, user.id).await?;
    Ok(Json(json!({})))
}
