// SPDX-License-Identifier: AGPL-3.0-or-later
//! Meal writes: validation, last-write-wins upsert, tombstoned delete.

use jiff::civil::Date;
use serde::Deserialize;
use sqlx::{SqliteConnection, SqlitePool};

use super::Outcome::{self, *};
use super::{date_ok, finite_in, one_of, reject, uuid};
use crate::error::Result;

#[derive(Deserialize)]
pub(super) struct MealRef {
    pub(super) id: String,
}

#[derive(Deserialize)]
pub(crate) struct MealIn {
    pub(crate) id: String,
    pub(crate) date: Date,
    pub(crate) meal_type: String,
    pub(crate) description: String,
    pub(crate) scale: f64,
    pub(crate) assumptions: Option<String>,
    pub(crate) confidence: Option<String>,
    pub(crate) source: String,
    pub(crate) provider_id: Option<i64>,
    pub(crate) items: Vec<ItemIn>,
}

#[derive(Deserialize)]
pub(crate) struct ItemIn {
    pub(crate) id: String,
    pub(crate) name: String,
    #[serde(default)]
    pub(crate) portion_text: String,
    pub(crate) grams: Option<f64>,
    pub(crate) kcal: f64,
    pub(crate) protein_g: f64,
    pub(crate) carbs_g: f64,
    pub(crate) fat_g: f64,
    pub(crate) food_id: Option<i64>,
    pub(crate) origin: String,
}

/// How a caller writes a meal: through sync (full conflict rules), a CSV import, or a restore.
/// `Import` and `Restore` are used by the transfer handlers.
#[allow(dead_code)]
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum WriteMode {
    Sync,
    Import,
    Restore,
}

impl MealIn {
    pub(crate) fn validate(&self, tomorrow: Date) -> std::result::Result<(), String> {
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
            one_of(&i.origin, &["food_db", "ai_estimate", "manual"], "origin")?;
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
pub(super) async fn upsert_meal(
    db: &SqlitePool,
    user_id: i64,
    m: &MealIn,
    effective: i64,
    received: i64,
) -> Result<Outcome> {
    let mut tx = db.begin_with("BEGIN IMMEDIATE").await?;
    let outcome = write_meal_on(&mut tx, user_id, m, effective, received, WriteMode::Sync).await?;
    if outcome == Applied {
        tx.commit().await?;
    }
    Ok(outcome)
}

/// Inserts or updates `m` and replaces its items on an open connection. The caller owns the
/// transaction. `Sync` also applies the stale/tombstone rules; `Import` and `Restore` skip only
/// those, still refusing ids owned by another account.
pub(crate) async fn write_meal_on(
    conn: &mut SqliteConnection,
    user_id: i64,
    m: &MealIn,
    effective: i64,
    received: i64,
    mode: WriteMode,
) -> Result<Outcome> {
    let existing = sqlx::query!(
        "SELECT user_id, updated_at, deleted_at FROM meals WHERE id = ?",
        m.id
    )
    .fetch_optional(&mut *conn)
    .await?;
    // A rewrite through sync, or anything restored, counts as edited: CSV import leaves it alone.
    let edited = mode == WriteMode::Restore || (mode == WriteMode::Sync && existing.is_some());
    if let Some(e) = existing {
        if e.user_id != user_id {
            return Ok(reject("meal id belongs to another account"));
        }
        if mode == WriteMode::Sync && (e.deleted_at.is_some() || e.updated_at >= effective) {
            return Ok(Stale);
        }
    }
    for i in &m.items {
        let owner = sqlx::query_scalar!("SELECT meal_id FROM meal_items WHERE id = ?", i.id)
            .fetch_optional(&mut *conn)
            .await?;
        if owner.is_some_and(|o| o != m.id) {
            return Ok(reject("item id belongs to another meal"));
        }
    }
    let date = m.date.to_string();
    // The client supplies this; an id that matches no provider is stored as NULL.
    let provider_id = match m.provider_id {
        Some(p) => {
            sqlx::query_scalar!("SELECT id FROM ai_providers WHERE id = ?", p)
                .fetch_optional(&mut *conn)
                .await?
        }
        None => None,
    };
    sqlx::query!(
        "INSERT INTO meals (id, user_id, date, meal_type, description, scale, assumptions,
             confidence, source, provider_id, updated_at, received_at, edited)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT (id) DO UPDATE SET date = excluded.date, meal_type = excluded.meal_type,
             description = excluded.description, scale = excluded.scale,
             assumptions = excluded.assumptions, confidence = excluded.confidence,
             source = excluded.source, provider_id = excluded.provider_id,
             updated_at = excluded.updated_at, received_at = excluded.received_at,
             edited = meals.edited OR excluded.edited",
        m.id,
        user_id,
        date,
        m.meal_type,
        m.description,
        m.scale,
        m.assumptions,
        m.confidence,
        m.source,
        provider_id,
        effective,
        received,
        edited
    )
    .execute(&mut *conn)
    .await?;
    sqlx::query!("DELETE FROM meal_items WHERE meal_id = ?", m.id)
        .execute(&mut *conn)
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
        .execute(&mut *conn)
        .await?;
        if let Some(food_id) = i.food_id
            && !crate::products::item_logged(&mut *conn, user_id, food_id, i.grams, received)
                .await?
        {
            return Ok(reject(format!("food {food_id} not found")));
        }
    }
    Ok(Applied)
}

pub(super) async fn delete_meal(
    db: &SqlitePool,
    user_id: i64,
    id: &str,
    effective: i64,
) -> Result<Outcome> {
    let mut tx = db.begin_with("BEGIN IMMEDIATE").await?;
    let existing = sqlx::query!(
        "SELECT user_id, updated_at, deleted_at FROM meals WHERE id = ?",
        id
    )
    .fetch_optional(&mut *tx)
    .await?;
    let Some(e) = existing else {
        // Never seen here: leave a tombstone so an older upsert of this id (a delayed offline
        // write from another device) loses. Placeholder values are never read: reads skip
        // tombstones, and cleanup purges them after TOMBSTONE_RETENTION_MS.
        sqlx::query!(
            "INSERT INTO meals (id, user_id, date, meal_type, description, source,
                 updated_at, received_at, deleted_at)
             VALUES (?, ?, '1970-01-01', 'snack', '', 'manual', ?, ?, ?)",
            id,
            user_id,
            effective,
            effective,
            effective
        )
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        return Ok(Applied);
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
