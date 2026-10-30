// SPDX-License-Identifier: AGPL-3.0-or-later
//! `POST /api/restore`: replaces the user's logs and private foods with a backup, in one
//! transaction. Shared food rows (`user_id IS NULL`) are read-only: a missing one becomes a
//! private copy owned by the target instead of being written back into the shared database.

use std::collections::HashMap;

use axum::Json;
use axum::extract::{Request, State};
use axum::http::StatusCode;
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::SqliteConnection;

use super::doc::*;
use super::validate::{bad, check, check_doc, meal_in};
use crate::AppState;
use crate::auth::User;
use crate::error::{Error, Result};
use crate::nutrition::foods::normalize;
use crate::sync::{Outcome, WriteMode};
use crate::time::now_ms;

#[derive(Deserialize)]
struct Envelope {
    confirm: bool,
    backup: Doc,
}

/// A backup is a whole account; anything bigger than this is not one.
const MAX_BODY: usize = 10 << 20;

async fn insert_private_food(conn: &mut SqliteConnection, user_id: i64, f: &Food) -> Result<i64> {
    let c = &f.core;
    let name = c.name.trim().to_string();
    let norm = normalize(&name);
    let existing = sqlx::query_scalar!(
        "SELECT id FROM foods WHERE user_id = ? AND name_norm = ? AND barcode IS ?
           AND kcal = ? AND protein_g = ? AND carbs_g = ? AND fat_g = ? LIMIT 1",
        user_id,
        &norm,
        &c.barcode,
        c.kcal,
        c.protein_g,
        c.carbs_g,
        c.fat_g
    )
    .fetch_optional(&mut *conn)
    .await?;
    let id = match existing {
        Some(id) => id,
        None => {
            sqlx::query_scalar!(
                "INSERT INTO foods (source, source_ref, user_id, name, name_norm, brand, barcode,
                     is_prepared, kcal, protein_g, carbs_g, fat_g, default_portion_g, serving_g,
                     serving_text, fetched_at)
                 VALUES ('user', ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                 RETURNING id",
                user_id.to_string(),
                user_id,
                &name,
                &norm,
                &c.brand,
                &c.barcode,
                i64::from(c.is_prepared),
                c.kcal,
                c.protein_g,
                c.carbs_g,
                c.fat_g,
                c.default_portion_g,
                c.serving_g,
                &c.serving_text,
                c.fetched_at
            )
            .fetch_one(&mut *conn)
            .await?
        }
    };
    for p in &f.portions {
        sqlx::query!(
            "INSERT INTO food_portions (food_id, unit, grams) VALUES (?, ?, ?)
             ON CONFLICT (food_id, unit) DO UPDATE SET grams = excluded.grams",
            id,
            &p.unit,
            p.grams
        )
        .execute(&mut *conn)
        .await?;
    }
    Ok(id)
}

/// Resolve one backup food to a live `foods.id`, reading shared rows and writing private ones.
/// User-entered foods (including label entries, which are private) are always restored privately.
async fn resolve_food(conn: &mut SqliteConnection, user_id: i64, f: &Food) -> Result<i64> {
    let c = &f.core;
    let shared = match (&c.source_ref, c.source.as_str()) {
        (Some(sr), "staple" | "usda" | "off") => {
            sqlx::query_scalar!(
                "SELECT id FROM foods
                 WHERE source = ? AND source_ref = ? AND user_id IS NULL LIMIT 1",
                &c.source,
                sr
            )
            .fetch_optional(&mut *conn)
            .await?
        }
        _ => None,
    };
    match shared {
        Some(id) => Ok(id),
        None => insert_private_food(conn, user_id, f).await,
    }
}

pub async fn restore(State(s): State<AppState>, user: User, req: Request) -> Result<Json<Value>> {
    // Parsed straight into the typed document: no intermediate `Value` copy.
    let bytes = axum::body::to_bytes(req.into_body(), MAX_BODY)
        .await
        .map_err(|_| Error::Status(StatusCode::PAYLOAD_TOO_LARGE, "backup is over 10 MB".into()))?;
    let env: Envelope =
        serde_json::from_slice(&bytes).map_err(|e| bad(format!("invalid backup request: {e}")))?;
    drop(bytes);
    check(env.confirm, "confirm must be true")?;
    let doc = env.backup;
    check_doc(&doc, user.id)?;

    let now = now_ms();
    let mut tx = s.db.begin_with("BEGIN IMMEDIATE").await?;

    // Ids owned by another account are a conflict, never an overwrite. Checked before any delete.
    for m in &doc.meals {
        let owner = sqlx::query_scalar!("SELECT user_id FROM meals WHERE id = ?", m.core.id)
            .fetch_optional(&mut *tx)
            .await?;
        if owner.is_some_and(|o| o != user.id) {
            return Err(Error::conflict(
                "a meal id in the backup belongs to another user",
            ));
        }
        for i in &m.items {
            let owner = sqlx::query_scalar!(
                "SELECT m.user_id FROM meal_items i JOIN meals m ON m.id = i.meal_id
                 WHERE i.id = ?",
                i.core.id
            )
            .fetch_optional(&mut *tx)
            .await?;
            if owner.is_some_and(|o| o != user.id) {
                return Err(Error::conflict(
                    "an item id in the backup belongs to another user",
                ));
            }
        }
    }

    // Full replacement of this user's logged data and private foods only.
    // Tombstones and `sync_ops` stay: a device replaying its offline queue after a restore must
    // not resurrect deleted meals, and old op ids stay duplicates. A backed-up meal whose id is
    // tombstoned here wins (restore is "go back to the snapshot") and is revived below.
    sqlx::query!(
        "DELETE FROM meals WHERE user_id = ? AND deleted_at IS NULL",
        user.id
    )
    .execute(&mut *tx)
    .await?;
    sqlx::query!("DELETE FROM days WHERE user_id = ?", user.id)
        .execute(&mut *tx)
        .await?;
    sqlx::query!("DELETE FROM food_aliases WHERE user_id = ?", user.id)
        .execute(&mut *tx)
        .await?;
    sqlx::query!("DELETE FROM user_portions WHERE user_id = ?", user.id)
        .execute(&mut *tx)
        .await?;
    sqlx::query!("DELETE FROM user_products WHERE user_id = ?", user.id)
        .execute(&mut *tx)
        .await?;
    sqlx::query!("DELETE FROM profile_versions WHERE user_id = ?", user.id)
        .execute(&mut *tx)
        .await?;
    sqlx::query!("DELETE FROM foods WHERE user_id = ?", user.id)
        .execute(&mut *tx)
        .await?;

    sqlx::query!(
        "UPDATE profiles SET timezone = ?, weight_unit = ?, height_unit = ? WHERE user_id = ?",
        &doc.profile.timezone,
        &doc.profile.weight_unit,
        &doc.profile.height_unit,
        user.id
    )
    .execute(&mut *tx)
    .await?;

    for v in &doc.profile_versions {
        sqlx::query!(
            "INSERT INTO profile_versions (user_id, effective_from, sex, birth_year, height_cm,
                 start_weight_kg, goal_kg, pace_kg_per_week, typical_steps)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            user.id,
            &v.effective_from,
            &v.sex,
            v.birth_year,
            v.height_cm,
            v.start_weight_kg,
            v.goal_kg,
            v.pace_kg_per_week,
            v.typical_steps
        )
        .execute(&mut *tx)
        .await?;
    }

    // Shared rows are read only. A missing one becomes a private copy for this user.
    let mut food_map: HashMap<i64, i64> = HashMap::with_capacity(doc.foods.len());
    for f in &doc.foods {
        let id = resolve_food(&mut tx, user.id, f).await?;
        food_map.insert(f.core.id, id);
    }

    for d in &doc.days {
        sqlx::query!(
            "INSERT INTO days (user_id, date, steps, steps_source, steps_updated_at, gym_minutes,
                 gym_source, gym_updated_at, trained, trained_updated_at, weight_kg,
                 weight_updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            user.id,
            &d.date,
            d.steps,
            &d.steps_source,
            d.steps_updated_at,
            d.gym_minutes,
            &d.gym_source,
            d.gym_updated_at,
            &d.trained,
            d.trained_updated_at,
            d.weight_kg,
            d.weight_updated_at
        )
        .execute(&mut *tx)
        .await?;
    }

    for m in &doc.meals {
        let meal = meal_in(m, |fid| {
            food_map
                .get(&fid)
                .copied()
                .ok_or_else(|| bad(format!("meal item references unresolved food {fid}")))
        })?;
        // Preserve the effective time, but never trust a client clock into the future.
        let updated = m.core.updated_at.0.min(now);
        let outcome =
            crate::sync::write_meal_on(&mut tx, user.id, &meal, updated, now, WriteMode::Restore)
                .await?;
        if outcome != Outcome::Applied {
            return Err(bad("a meal in the backup could not be restored"));
        }
        sqlx::query!("UPDATE meals SET deleted_at = NULL WHERE id = ?", m.core.id)
            .execute(&mut *tx)
            .await?;
    }

    // `write_meal_on` records barcoded items as products; the backup is the source of truth.
    sqlx::query!("DELETE FROM user_products WHERE user_id = ?", user.id)
        .execute(&mut *tx)
        .await?;

    for a in &doc.aliases {
        let alias = a.alias.trim();
        sqlx::query!(
            "INSERT INTO food_aliases (food_id, alias, alias_norm, user_id) VALUES (?, ?, ?, ?)",
            food_map[&a.food_id],
            alias,
            normalize(alias),
            user.id
        )
        .execute(&mut *tx)
        .await?;
    }

    for p in &doc.portions {
        sqlx::query!(
            "INSERT INTO user_portions (user_id, phrase_norm, food_id, grams) VALUES (?, ?, ?, ?)",
            user.id,
            p.phrase_norm.trim(),
            food_map[&p.food_id],
            p.grams
        )
        .execute(&mut *tx)
        .await?;
    }

    for p in &doc.products {
        sqlx::query!(
            "INSERT INTO user_products (user_id, food_id, last_servings, last_used_at)
             VALUES (?, ?, ?, ?)",
            user.id,
            food_map[&p.food_id],
            p.last_servings,
            p.last_used_at
        )
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(Json(json!({
        "profile_versions": doc.profile_versions.len(),
        "days": doc.days.len(),
        "meals": doc.meals.len(),
        "items": doc.meals.iter().map(|m| m.items.len()).sum::<usize>(),
        "foods": doc.foods.len(),
        "aliases": doc.aliases.len(),
        "portions": doc.portions.len(),
        "products": doc.products.len(),
    })))
}
