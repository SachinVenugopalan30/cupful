// SPDX-License-Identifier: AGPL-3.0-or-later
//! Writes grouped drafts and per-day activity in one transaction. Import is never newer than the
//! app: a tombstoned or edited meal is left alone.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use jiff::civil::Date;
use uuid::Uuid;

use super::super::parse::{DayAccum, Draft, line_err};
use crate::AppState;
use crate::error::{Error, Result};
use crate::sync::{
    DayPatch, ItemIn, MealIn, Outcome, Source, WriteMode, patch_day_on, write_meal_on,
};

#[derive(sqlx::FromRow)]
struct StoredMeal {
    date: String,
    meal_type: String,
    description: String,
    scale: f64,
    assumptions: Option<String>,
    confidence: Option<String>,
    source: String,
    provider_id: Option<i64>,
    deleted_at: Option<i64>,
    edited: i64,
}

#[derive(sqlx::FromRow)]
struct StoredItem {
    id: String,
    name: String,
    portion_text: String,
    grams: Option<f64>,
    kcal: f64,
    protein_g: f64,
    carbs_g: f64,
    fat_g: f64,
    food_id: Option<i64>,
    origin: String,
}

#[derive(sqlx::FromRow)]
struct StoredDay {
    steps: Option<i64>,
    gym_minutes: Option<i64>,
    trained: Option<String>,
    weight_kg: Option<f64>,
}

#[derive(Default)]
pub(super) struct Counts {
    pub(super) created: i64,
    pub(super) updated: i64,
    pub(super) unchanged: i64,
    pub(super) skipped_deleted: i64,
    pub(super) skipped_edited: i64,
    pub(super) items: i64,
    pub(super) days: i64,
}

fn import_ns() -> Uuid {
    static NS: OnceLock<Uuid> = OnceLock::new();
    *NS.get_or_init(|| Uuid::new_v5(&Uuid::NAMESPACE_OID, b"cupful fuel-ledger import v1"))
}

pub(super) async fn apply(
    s: &AppState,
    user_id: i64,
    drafts: &[Draft],
    days: &BTreeMap<String, DayAccum>,
    now: i64,
    tomorrow: Date,
) -> Result<Counts> {
    let mut tx = s.db.begin_with("BEGIN IMMEDIATE").await?;
    let mut c = Counts::default();

    for d in drafts {
        let uuid = if d.legacy {
            Uuid::new_v5(
                &import_ns(),
                format!("legacy:{user_id}:{}:{}", d.legacy_hash, d.legacy_occurrence).as_bytes(),
            )
        } else {
            match Uuid::parse_str(&d.raw_id) {
                Ok(u) => {
                    let owner = sqlx::query_scalar!(
                        "SELECT user_id FROM meals WHERE id = ?",
                        u.to_string()
                    )
                    .fetch_optional(&mut *tx)
                    .await?;
                    if owner == Some(user_id) {
                        u
                    } else {
                        Uuid::new_v5(&import_ns(), format!("{user_id}:{}", d.raw_id).as_bytes())
                    }
                }
                Err(_) => Uuid::new_v5(&import_ns(), format!("{user_id}:{}", d.raw_id).as_bytes()),
            }
        };
        let meal_id = uuid.to_string();

        let existing: Option<StoredMeal> = sqlx::query_as!(
            StoredMeal,
            "SELECT date, meal_type, description, scale, assumptions, confidence, source,
                    provider_id, deleted_at, edited
             FROM meals WHERE id = ? AND user_id = ?",
            &meal_id,
            user_id
        )
        .fetch_optional(&mut *tx)
        .await?;

        // PLAN §6: an upsert of a deleted id is stale, and import is never newer than the app.
        if existing.as_ref().is_some_and(|e| e.deleted_at.is_some()) {
            c.skipped_deleted += 1;
            continue;
        }

        let existing_items: Vec<StoredItem> = if existing.is_some() {
            sqlx::query_as!(
                StoredItem,
                "SELECT id, name, portion_text, grams, kcal, protein_g, carbs_g, fat_g,
                        food_id, origin
                 FROM meal_items WHERE meal_id = ? ORDER BY position",
                &meal_id
            )
            .fetch_all(&mut *tx)
            .await?
        } else {
            Vec::new()
        };

        let live = existing.as_ref().is_some_and(|e| e.deleted_at.is_none());
        let mut items = Vec::with_capacity(d.items.len());
        let (source, scale, assumptions, confidence, provider_id) = if live {
            let e = existing.as_ref().unwrap();
            (
                e.source.clone(),
                e.scale,
                e.assumptions.clone(),
                e.confidence.clone(),
                e.provider_id,
            )
        } else {
            ("import".to_string(), 1.0, None, None, None)
        };

        // Match by position, not the raw column: a 1-based Fuel Ledger file still maps onto the
        // item created at that ordinal by an earlier import.
        for (ordinal, it) in d.items.iter().enumerate() {
            let matched = if live {
                existing_items.get(ordinal)
            } else {
                None
            };
            match matched {
                Some(e) => items.push(ItemIn {
                    id: e.id.clone(),
                    name: it.name.clone(),
                    portion_text: it.portion.clone(),
                    grams: e.grams,
                    kcal: it.kcal,
                    protein_g: it.protein_g,
                    carbs_g: it.carbs_g,
                    fat_g: it.fat_g,
                    food_id: e.food_id,
                    origin: e.origin.clone(),
                }),
                None => items.push(ItemIn {
                    id: Uuid::new_v5(&uuid, it.index.to_string().as_bytes()).to_string(),
                    name: it.name.clone(),
                    portion_text: it.portion.clone(),
                    grams: None,
                    kcal: it.kcal,
                    protein_g: it.protein_g,
                    carbs_g: it.carbs_g,
                    fat_g: it.fat_g,
                    food_id: None,
                    origin: "manual".to_string(),
                }),
            }
        }

        let meal = MealIn {
            id: meal_id.clone(),
            date: d.date,
            meal_type: d.meal_type.clone(),
            description: d.description.clone(),
            scale,
            assumptions,
            confidence,
            source,
            provider_id,
            items,
        };
        meal.validate(tomorrow).map_err(|e| line_err(d.line, e))?;

        if live {
            let e = existing.as_ref().unwrap();
            let same_meal = e.date == d.date.to_string()
                && e.meal_type == meal.meal_type
                && e.description == meal.description
                && e.scale == meal.scale
                && e.assumptions == meal.assumptions
                && e.confidence == meal.confidence
                && e.source == meal.source
                && e.provider_id == meal.provider_id;
            let same_items = existing_items.len() == meal.items.len()
                && existing_items.iter().zip(&meal.items).all(|(a, b)| {
                    a.id == b.id
                        && a.name == b.name
                        && a.portion_text == b.portion_text
                        && a.grams == b.grams
                        && a.kcal == b.kcal
                        && a.protein_g == b.protein_g
                        && a.carbs_g == b.carbs_g
                        && a.fat_g == b.fat_g
                        && a.food_id == b.food_id
                        && a.origin == b.origin
                });
            if same_meal && same_items {
                c.unchanged += 1;
                c.items += meal.items.len() as i64;
                continue;
            }
            // Edited in Cupful since it was logged or imported: the app's version wins.
            if existing.as_ref().is_some_and(|e| e.edited != 0) {
                c.skipped_edited += 1;
                continue;
            }
        }

        match write_meal_on(&mut tx, user_id, &meal, now, now, WriteMode::Import).await? {
            Outcome::Applied => {
                if live {
                    c.updated += 1;
                } else {
                    c.created += 1;
                }
                c.items += meal.items.len() as i64;
            }
            Outcome::Rejected(e) => {
                return Err(Error::conflict(format!("line {}: {e}", d.line)));
            }
            Outcome::Duplicate | Outcome::Stale => {
                c.unchanged += 1;
            }
        }
    }

    for (date, acc) in days {
        let date = date.parse::<Date>().expect("parsed above");
        let stored: Option<StoredDay> = sqlx::query_as!(
            StoredDay,
            "SELECT steps, gym_minutes, trained, weight_kg FROM days
                 WHERE user_id = ? AND date = ?",
            user_id,
            date.to_string()
        )
        .fetch_optional(&mut *tx)
        .await?;
        let (s_steps, s_gym, s_trained, s_weight) = stored
            .map(|d| (d.steps, d.gym_minutes, d.trained, d.weight_kg))
            .unwrap_or((None, None, None, None));
        let mut patch = DayPatch::default();

        if let Some((v, _)) = acc.steps
            && s_steps != Some(v)
        {
            patch.steps = Some(Some(v));
        }
        if let Some((v, _)) = acc.gym_minutes
            && s_gym != Some(v)
        {
            patch.gym_minutes = Some(Some(v));
        }
        if let Some((v, _)) = &acc.weight_kg
            && s_weight.is_none()
        {
            patch.weight_kg = Some(Some(*v));
        }
        if let Some((v, _)) = &acc.trained {
            let stored_parts: Vec<String> = s_trained
                .as_deref()
                .and_then(|t| serde_json::from_str(t).ok())
                .unwrap_or_default();
            if stored_parts != *v && (s_trained.is_none() || stored_parts.is_empty()) {
                patch.trained = Some(Some(v.clone()));
            }
        }

        let any = patch.steps.is_some()
            || patch.gym_minutes.is_some()
            || patch.trained.is_some()
            || patch.weight_kg.is_some();
        if any && patch_day_on(&mut tx, user_id, date, &patch, Source::Import, now).await? {
            c.days += 1;
        }
    }

    tx.commit().await?;
    Ok(c)
}
