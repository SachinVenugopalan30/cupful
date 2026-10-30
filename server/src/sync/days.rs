// SPDX-License-Identifier: AGPL-3.0-or-later
//! Day patches: per-field last-write-wins with source precedence.

use jiff::civil::Date;
use serde::{Deserialize, Deserializer};
use sqlx::{SqliteConnection, SqlitePool};

use super::{BODY_PARTS, finite_in, one_of};
use crate::error::Result;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Source {
    Manual,
    Shortcut,
    Import,
    Google,
}

impl Source {
    fn as_str(self) -> &'static str {
        match self {
            Source::Manual => "manual",
            Source::Shortcut => "shortcut",
            Source::Import => "import",
            Source::Google => "google",
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
pub(super) struct DayPatchIn {
    pub(super) date: Date,
    #[serde(flatten)]
    pub(super) patch: DayPatch,
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

/// Days of Shortcut and Google activity accepted or synced, counting back from today.
pub const WINDOW_DAYS: i64 = 14;

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
    let mut tx = db.begin_with("BEGIN IMMEDIATE").await?;
    let changed = patch_day_on(&mut tx, user_id, date, p, source, effective).await?;
    tx.commit().await?;
    Ok(changed)
}

/// Applies a day patch on an open connection. The caller owns the transaction.
pub(crate) async fn patch_day_on(
    conn: &mut SqliteConnection,
    user_id: i64,
    date: Date,
    p: &DayPatch,
    source: Source,
    effective: i64,
) -> Result<bool> {
    let date = date.to_string();
    let src = source.as_str();
    sqlx::query!(
        "INSERT OR IGNORE INTO days (user_id, date) VALUES (?, ?)",
        user_id,
        date
    )
    .execute(&mut *conn)
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
        .execute(&mut *conn)
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
        .execute(&mut *conn)
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
        .execute(&mut *conn)
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
        .execute(&mut *conn)
        .await?
        .rows_affected();
    }
    Ok(changed > 0)
}
