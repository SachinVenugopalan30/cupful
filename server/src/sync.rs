// SPDX-License-Identifier: AGPL-3.0-or-later
//! `POST /api/sync`: the only write path for logged data (PLAN.md §6).

use crate::error::JsonBody;
use axum::Json;
use axum::extract::State;
use jiff::civil::Date;
use serde::{Deserialize, Deserializer};
use serde_json::{Value, json};
use sqlx::SqlitePool;

use crate::AppState;
use crate::auth::User;
use crate::cleanup::TOMBSTONE_RETENTION_MS;
use crate::error::{Error, Result};
use crate::time::{now_ms, user_today};

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

fn reject(msg: impl Into<String>) -> Outcome {
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

fn date_ok(date: Date, tomorrow: Date) -> std::result::Result<(), String> {
    (date <= tomorrow)
        .then_some(())
        .ok_or_else(|| format!("{date} is in the future"))
}

fn finite_in(v: f64, lo: f64, hi: f64, what: &str) -> std::result::Result<(), String> {
    (v.is_finite() && (lo..=hi).contains(&v))
        .then_some(())
        .ok_or_else(|| format!("{what} must be between {lo} and {hi}"))
}

fn one_of(v: &str, allowed: &[&str], what: &str) -> std::result::Result<(), String> {
    allowed
        .contains(&v)
        .then_some(())
        .ok_or_else(|| format!("{what} must be one of {allowed:?}"))
}

fn uuid(s: &str, what: &str) -> std::result::Result<(), String> {
    uuid::Uuid::parse_str(s)
        .map(|_| ())
        .map_err(|_| format!("{what} must be a UUID"))
}

// ---- meals ----

#[derive(Deserialize)]
struct MealRef {
    id: String,
}

#[derive(Deserialize)]
struct MealIn {
    id: String,
    date: Date,
    meal_type: String,
    description: String,
    scale: f64,
    assumptions: Option<String>,
    confidence: Option<String>,
    source: String,
    provider_id: Option<i64>,
    items: Vec<ItemIn>,
}

#[derive(Deserialize)]
struct ItemIn {
    id: String,
    name: String,
    #[serde(default)]
    portion_text: String,
    grams: Option<f64>,
    kcal: f64,
    protein_g: f64,
    carbs_g: f64,
    fat_g: f64,
    food_id: Option<i64>,
    origin: String,
}

impl MealIn {
    fn validate(&self, tomorrow: Date) -> std::result::Result<(), String> {
        uuid(&self.id, "meal id")?;
        date_ok(self.date, tomorrow)?;
        one_of(
            &self.meal_type,
            &["breakfast", "lunch", "dinner", "snack"],
            "meal_type",
        )?;
        one_of(
            &self.source,
            &["ai", "manual", "repeat", "import"],
            "source",
        )?;
        if let Some(c) = &self.confidence {
            one_of(c, &["low", "medium", "high"], "confidence")?;
        }
        if self.description.chars().count() > 1000 {
            return Err("description is longer than 1000 characters".into());
        }
        if self
            .assumptions
            .as_ref()
            .is_some_and(|a| a.chars().count() > 1000)
        {
            return Err("assumptions are longer than 1000 characters".into());
        }
        finite_in(self.scale, 0.1, 10.0, "scale")?;
        if self.items.len() > 100 {
            return Err("at most 100 items per meal".into());
        }
        for i in &self.items {
            uuid(&i.id, "item id")?;
            if i.name.trim().is_empty() || i.name.chars().count() > 200 {
                return Err("item name must be 1-200 characters".into());
            }
            if i.portion_text.chars().count() > 200 {
                return Err("portion_text is longer than 200 characters".into());
            }
            one_of(&i.origin, &["food_db", "ai_estimate"], "origin")?;
            if let Some(g) = i.grams {
                finite_in(g, 0.0, 10_000.0, "grams")?;
            }
            finite_in(i.kcal, 0.0, 20_000.0, "kcal")?;
            finite_in(i.protein_g, 0.0, 2_000.0, "protein_g")?;
            finite_in(i.carbs_g, 0.0, 2_000.0, "carbs_g")?;
            finite_in(i.fat_g, 0.0, 2_000.0, "fat_g")?;
        }
        Ok(())
    }
}

/// Last write wins on the whole meal by effective time; a tombstone beats any later upsert.
/// Items end up exactly as in the payload.
async fn upsert_meal(
    db: &SqlitePool,
    user_id: i64,
    m: &MealIn,
    effective: i64,
    received: i64,
) -> Result<Outcome> {
    let mut tx = db.begin_with("BEGIN IMMEDIATE").await?;
    let existing = sqlx::query!(
        "SELECT user_id, updated_at, deleted_at FROM meals WHERE id = ?",
        m.id
    )
    .fetch_optional(&mut *tx)
    .await?;
    if let Some(e) = existing {
        if e.user_id != user_id {
            return Ok(reject("meal id belongs to another account"));
        }
        if e.deleted_at.is_some() || e.updated_at >= effective {
            return Ok(Stale);
        }
    }
    for i in &m.items {
        let owner = sqlx::query_scalar!("SELECT meal_id FROM meal_items WHERE id = ?", i.id)
            .fetch_optional(&mut *tx)
            .await?;
        if owner.is_some_and(|o| o != m.id) {
            return Ok(reject("item id belongs to another meal"));
        }
    }
    let date = m.date.to_string();
    sqlx::query!(
        "INSERT INTO meals (id, user_id, date, meal_type, description, scale, assumptions,
             confidence, source, provider_id, updated_at, received_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT (id) DO UPDATE SET date = excluded.date, meal_type = excluded.meal_type,
             description = excluded.description, scale = excluded.scale,
             assumptions = excluded.assumptions, confidence = excluded.confidence,
             source = excluded.source, provider_id = excluded.provider_id,
             updated_at = excluded.updated_at, received_at = excluded.received_at",
        m.id,
        user_id,
        date,
        m.meal_type,
        m.description,
        m.scale,
        m.assumptions,
        m.confidence,
        m.source,
        m.provider_id,
        effective,
        received
    )
    .execute(&mut *tx)
    .await?;
    sqlx::query!("DELETE FROM meal_items WHERE meal_id = ?", m.id)
        .execute(&mut *tx)
        .await?;
    for (pos, i) in m.items.iter().enumerate() {
        let pos = pos as i64;
        sqlx::query!(
            "INSERT INTO meal_items (id, meal_id, position, name, portion_text, grams, kcal,
                 protein_g, carbs_g, fat_g, food_id, origin)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            i.id,
            m.id,
            pos,
            i.name,
            i.portion_text,
            i.grams,
            i.kcal,
            i.protein_g,
            i.carbs_g,
            i.fat_g,
            i.food_id,
            i.origin
        )
        .execute(&mut *tx)
        .await?;
        if let Some(food_id) = i.food_id
            && !crate::products::item_logged(&mut tx, user_id, food_id, i.grams, received).await?
        {
            return Ok(reject(format!("food {food_id} not found")));
        }
    }
    tx.commit().await?;
    Ok(Applied)
}

async fn delete_meal(db: &SqlitePool, user_id: i64, id: &str, effective: i64) -> Result<Outcome> {
    let mut tx = db.begin_with("BEGIN IMMEDIATE").await?;
    let existing = sqlx::query!(
        "SELECT user_id, updated_at, deleted_at FROM meals WHERE id = ?",
        id
    )
    .fetch_optional(&mut *tx)
    .await?;
    let Some(e) = existing else {
        return Ok(Applied); // never synced; nothing to delete
    };
    if e.user_id != user_id {
        return Ok(reject("meal id belongs to another account"));
    }
    if e.deleted_at.is_some() {
        return Ok(Applied);
    }
    if e.updated_at >= effective {
        return Ok(Stale);
    }
    sqlx::query!(
        "UPDATE meals SET deleted_at = ?, updated_at = ? WHERE id = ?",
        effective,
        effective,
        id
    )
    .execute(&mut *tx)
    .await?;
    sqlx::query!("DELETE FROM meal_items WHERE meal_id = ?", id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(Applied)
}

// ---- days ----

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Source {
    Manual,
    Shortcut,
    Import,
}

impl Source {
    fn as_str(self) -> &'static str {
        match self {
            Source::Manual => "manual",
            Source::Shortcut => "shortcut",
            Source::Import => "import",
        }
    }
}

/// Fields to change. Outer `None` = leave alone; `Some(None)` = clear.
#[derive(Debug, Default, Deserialize)]
pub struct DayPatch {
    #[serde(default, deserialize_with = "present")]
    pub steps: Option<Option<i64>>,
    #[serde(default, deserialize_with = "present")]
    pub gym_minutes: Option<Option<i64>>,
    #[serde(default, deserialize_with = "present")]
    pub trained: Option<Option<Vec<String>>>,
    #[serde(default, deserialize_with = "present")]
    pub weight_kg: Option<Option<f64>>,
}

/// Distinguishes `"field": null` (clear) from an absent field.
fn present<'de, T: Deserialize<'de>, D: Deserializer<'de>>(
    d: D,
) -> std::result::Result<Option<T>, D::Error> {
    T::deserialize(d).map(Some)
}

#[derive(Deserialize)]
struct DayPatchIn {
    date: Date,
    #[serde(flatten)]
    patch: DayPatch,
}

impl DayPatch {
    pub fn validate(&self) -> std::result::Result<(), String> {
        if let Some(Some(s)) = self.steps {
            finite_in(s as f64, 0.0, 150_000.0, "steps")?;
        }
        if let Some(Some(m)) = self.gym_minutes {
            finite_in(m as f64, 0.0, 600.0, "gym_minutes")?;
        }
        if let Some(Some(w)) = self.weight_kg {
            finite_in(w, 20.0, 400.0, "weight_kg")?;
        }
        if let Some(Some(t)) = &self.trained {
            for part in t {
                one_of(part, &BODY_PARTS, "trained")?;
            }
        }
        Ok(())
    }
}

/// Applies each present field that wins: newer than that field's last write, and never an
/// automated source over a manual value. Returns whether any field changed. Callers validate first.
pub async fn patch_day(
    db: &SqlitePool,
    user_id: i64,
    date: Date,
    p: &DayPatch,
    source: Source,
    effective: i64,
) -> Result<bool> {
    let date = date.to_string();
    let src = source.as_str();
    let mut tx = db.begin_with("BEGIN IMMEDIATE").await?;
    sqlx::query!(
        "INSERT OR IGNORE INTO days (user_id, date) VALUES (?, ?)",
        user_id,
        date
    )
    .execute(&mut *tx)
    .await?;
    let mut changed = 0;
    if let Some(v) = p.steps {
        // A cleared value has no source, so automation may fill it again.
        let s = v.map(|_| src);
        changed += sqlx::query!(
            "UPDATE days SET steps = ?, steps_source = ?, steps_updated_at = ?
             WHERE user_id = ? AND date = ? AND coalesce(steps_updated_at, 0) < ?
               AND NOT (? != 'manual' AND steps_source IS 'manual')",
            v,
            s,
            effective,
            user_id,
            date,
            effective,
            src
        )
        .execute(&mut *tx)
        .await?
        .rows_affected();
    }
    if let Some(v) = p.gym_minutes {
        let s = v.map(|_| src);
        changed += sqlx::query!(
            "UPDATE days SET gym_minutes = ?, gym_source = ?, gym_updated_at = ?
             WHERE user_id = ? AND date = ? AND coalesce(gym_updated_at, 0) < ?
               AND NOT (? != 'manual' AND gym_source IS 'manual')",
            v,
            s,
            effective,
            user_id,
            date,
            effective,
            src
        )
        .execute(&mut *tx)
        .await?
        .rows_affected();
    }
    if let Some(v) = &p.trained {
        let json = v
            .as_ref()
            .map(|t| serde_json::to_string(t).expect("strings serialize"));
        changed += sqlx::query!(
            "UPDATE days SET trained = ?, trained_updated_at = ?
             WHERE user_id = ? AND date = ? AND coalesce(trained_updated_at, 0) < ?",
            json,
            effective,
            user_id,
            date,
            effective
        )
        .execute(&mut *tx)
        .await?
        .rows_affected();
    }
    if let Some(v) = p.weight_kg {
        changed += sqlx::query!(
            "UPDATE days SET weight_kg = ?, weight_updated_at = ?
             WHERE user_id = ? AND date = ? AND coalesce(weight_updated_at, 0) < ?",
            v,
            effective,
            user_id,
            date,
            effective
        )
        .execute(&mut *tx)
        .await?
        .rows_affected();
    }
    tx.commit().await?;
    Ok(changed > 0)
}
