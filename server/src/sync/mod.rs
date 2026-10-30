// SPDX-License-Identifier: AGPL-3.0-or-later
//! `POST /api/sync`: the only write path for logged data (PLAN.md §6).

use crate::error::JsonBody;
use axum::Json;
use axum::extract::State;
use jiff::civil::Date;
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::SqlitePool;

mod days;
mod meals;

pub(crate) use days::patch_day_on;
pub use days::{DayPatch, Source, WINDOW_DAYS, patch_day};
pub(crate) use meals::{ItemIn, MealIn, WriteMode, write_meal_on};

use crate::AppState;
use crate::auth::User;
use crate::cleanup::TOMBSTONE_RETENTION_MS;
use crate::error::{Error, Result};
use crate::time::{now_ms, user_today};
use days::DayPatchIn;
use meals::{MealRef, delete_meal, upsert_meal};

const MAX_OPS: usize = 100;
const MAX_CLOCK_AHEAD_MS: i64 = 5 * 60 * 1000;
pub const BODY_PARTS: [&str; 8] = [
    "Chest",
    "Back",
    "Legs",
    "Shoulders",
    "Arms",
    "Core",
    "Full body",
    "Cardio",
];

#[derive(Debug, PartialEq)]
pub enum Outcome {
    Applied,
    Duplicate,
    Stale,
    Rejected(String),
}
use Outcome::*;

pub(super) fn reject(msg: impl Into<String>) -> Outcome {
    Rejected(msg.into())
}

#[derive(Deserialize)]
pub struct SyncBody {
    ops: Vec<Value>,
}

#[derive(Deserialize)]
struct Op {
    op_id: String,
    kind: String,
    at: String,
    payload: Value,
}

pub async fn sync(
    State(s): State<AppState>,
    user: User,
    JsonBody(body): JsonBody<SyncBody>,
) -> Result<Json<Value>> {
    if body.ops.len() > MAX_OPS {
        return Err(Error::bad_request(format!(
            "at most {MAX_OPS} ops per request"
        )));
    }
    let today = user_today(&s.db, user.id).await?;
    let mut results = Vec::with_capacity(body.ops.len());
    for raw in body.ops {
        let op_id = raw.get("op_id").cloned().unwrap_or(Value::Null);
        let outcome = match serde_json::from_value::<Op>(raw) {
            Ok(op) => apply(&s.db, user.id, today, op, now_ms()).await?,
            Err(e) => reject(format!("malformed op: {e}")),
        };
        results.push(match outcome {
            Applied => json!({"op_id": op_id, "status": "applied"}),
            Duplicate => json!({"op_id": op_id, "status": "duplicate"}),
            Stale => json!({"op_id": op_id, "status": "stale"}),
            Rejected(e) => json!({"op_id": op_id, "status": "rejected", "error": e}),
        });
    }
    Ok(Json(json!({ "results": results })))
}

async fn apply(
    db: &SqlitePool,
    user_id: i64,
    today: Date,
    op: Op,
    received: i64,
) -> Result<Outcome> {
    if op.op_id.is_empty() || op.op_id.len() > 64 {
        return Ok(reject("op_id must be 1-64 characters"));
    }
    let seen = sqlx::query_scalar!(
        "SELECT 1 AS \"x: i64\" FROM sync_ops WHERE user_id = ? AND op_id = ?",
        user_id,
        op.op_id
    )
    .fetch_optional(db)
    .await?;
    if seen.is_some() {
        return Ok(Duplicate);
    }
    let Ok(at) = op.at.parse::<jiff::Timestamp>() else {
        return Ok(reject("`at` must be an RFC 3339 timestamp"));
    };
    let at = at.as_millisecond();
    if at > received + MAX_CLOCK_AHEAD_MS {
        return Ok(reject(
            "`at` is more than 5 minutes ahead of the server clock",
        ));
    }
    // Older than tombstone retention: it could resurrect a deleted meal, so it only loses.
    if at < received - TOMBSTONE_RETENTION_MS {
        return Ok(Stale);
    }
    let effective = at.min(received);
    let tomorrow = today
        .tomorrow()
        .map_err(|e| Error::Internal(e.to_string()))?;
    let outcome = match op.kind.as_str() {
        "meal.upsert" => match serde_json::from_value::<MealIn>(op.payload) {
            Err(e) => reject(format!("invalid meal: {e}")),
            Ok(m) => match m.validate(tomorrow) {
                Err(e) => reject(e),
                Ok(()) => upsert_meal(db, user_id, &m, effective, received).await?,
            },
        },
        "meal.delete" => match serde_json::from_value::<MealRef>(op.payload) {
            Err(e) => reject(format!("invalid meal.delete: {e}")),
            Ok(m) => delete_meal(db, user_id, &m.id, effective).await?,
        },
        "day.patch" => match serde_json::from_value::<DayPatchIn>(op.payload) {
            Err(e) => reject(format!("invalid day.patch: {e}")),
            Ok(p) => match p.patch.validate().and(date_ok(p.date, tomorrow)) {
                Err(e) => reject(e),
                Ok(()) => {
                    if patch_day(db, user_id, p.date, &p.patch, Source::Manual, effective).await? {
                        Applied
                    } else {
                        Stale
                    }
                }
            },
        },
        k => reject(format!("unknown op kind {k:?}")),
    };
    if outcome == Applied {
        sqlx::query!(
            "INSERT OR IGNORE INTO sync_ops (user_id, op_id, received_at) VALUES (?, ?, ?)",
            user_id,
            op.op_id,
            received
        )
        .execute(db)
        .await?;
    }
    Ok(outcome)
}

pub(crate) fn date_ok(date: Date, tomorrow: Date) -> std::result::Result<(), String> {
    (date <= tomorrow)
        .then_some(())
        .ok_or_else(|| format!("{date} is in the future"))
}

pub(crate) fn finite_in(v: f64, lo: f64, hi: f64, what: &str) -> std::result::Result<(), String> {
    (v.is_finite() && (lo..=hi).contains(&v))
        .then_some(())
        .ok_or_else(|| format!("{what} must be between {lo} and {hi}"))
}

pub(crate) fn one_of(v: &str, allowed: &[&str], what: &str) -> std::result::Result<(), String> {
    allowed
        .contains(&v)
        .then_some(())
        .ok_or_else(|| format!("{what} must be one of {allowed:?}"))
}

pub(super) fn uuid(s: &str, what: &str) -> std::result::Result<(), String> {
    uuid::Uuid::parse_str(s)
        .map(|_| ())
        .map_err(|_| format!("{what} must be a UUID"))
}
