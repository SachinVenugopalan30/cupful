// SPDX-License-Identifier: AGPL-3.0-or-later
//! `GET /api/backup`.

use std::collections::HashMap;

use axum::Json;
use axum::extract::State;
use axum::http::HeaderValue;
use axum::http::header::{CACHE_CONTROL, CONTENT_DISPOSITION, CONTENT_TYPE};
use axum::response::{IntoResponse, Response};

use super::doc::*;
use crate::AppState;
use crate::auth::User;
use crate::error::{Error, Result};
use crate::time::{now_ms, rfc3339, user_today};

/// Files grouped by their owner, for the nested `items` / `portions` arrays.
fn group<K: std::hash::Hash + Eq, V>(rows: Vec<V>, key: impl Fn(&V) -> K) -> HashMap<K, Vec<V>> {
    let mut m: HashMap<K, Vec<V>> = HashMap::new();
    for r in rows {
        m.entry(key(&r)).or_default().push(r);
    }
    m
}

pub async fn export(State(s): State<AppState>, user: User) -> Result<Response> {
    let db = &s.db;
    let id = user.id;
    let today = user_today(db, id).await?;

    let profile = sqlx::query_as!(
        Profile,
        "SELECT timezone, weight_unit, height_unit FROM profiles WHERE user_id = ?",
        id
    )
    .fetch_one(db)
    .await?;
    let profile_versions = sqlx::query_as!(
        Version,
        "SELECT effective_from, sex, birth_year, height_cm, start_weight_kg, goal_kg,
                pace_kg_per_week, typical_steps
         FROM profile_versions WHERE user_id = ? ORDER BY effective_from, id",
        id
    )
    .fetch_all(db)
    .await?;
    let days = sqlx::query_as!(
        Day,
        "SELECT date, steps, steps_source, steps_updated_at AS \"steps_updated_at: Ts\",
                gym_minutes, gym_source, gym_updated_at AS \"gym_updated_at: Ts\", trained,
                trained_updated_at AS \"trained_updated_at: Ts\", weight_kg,
                weight_updated_at AS \"weight_updated_at: Ts\"
         FROM days WHERE user_id = ? ORDER BY date",
        id
    )
    .fetch_all(db)
    .await?;
    let meal_cores = sqlx::query_as!(
        MealCore,
        "SELECT id, date, meal_type, description, scale, assumptions, confidence, source,
                provider_id, updated_at AS \"updated_at: Ts\", received_at AS \"received_at: Ts\"
         FROM meals WHERE user_id = ? AND deleted_at IS NULL ORDER BY date, id",
        id
    )
    .fetch_all(db)
    .await?;
    let item_cores = sqlx::query_as!(
        ItemCore,
        "SELECT i.meal_id, i.id, i.position, i.name, i.portion_text, i.grams, i.kcal,
                i.protein_g, i.carbs_g, i.fat_g, i.food_id, i.origin
         FROM meal_items i JOIN meals m ON m.id = i.meal_id
         WHERE m.user_id = ? AND m.deleted_at IS NULL
         ORDER BY i.meal_id, i.position, i.id",
        id
    )
    .fetch_all(db)
    .await?;
    // Food ids visible to the user that this backup must carry: private foods, their own label
    // entries, and everything referenced by a live meal item, alias, portion or product.
    // Another user's private food can never match.
    let food_cores = sqlx::query_as!(
        FoodCore,
        "SELECT f.id, f.source, f.source_ref, f.user_id, f.name, f.name_norm, f.brand, f.barcode,
               f.is_prepared AS \"is_prepared: bool\", f.kcal, f.protein_g, f.carbs_g, f.fat_g,
               f.default_portion_g, f.serving_g, f.serving_text,
               f.fetched_at AS \"fetched_at: Ts\"
        FROM foods f
        WHERE (f.user_id IS NULL OR f.user_id = ?)
          AND (
            f.user_id = ?
            OR f.id IN (
                SELECT i.food_id FROM meal_items i JOIN meals m ON m.id = i.meal_id
                WHERE m.user_id = ? AND m.deleted_at IS NULL AND i.food_id IS NOT NULL
                UNION SELECT food_id FROM food_aliases WHERE user_id = ?
                UNION SELECT food_id FROM user_portions WHERE user_id = ?
                UNION SELECT food_id FROM user_products WHERE user_id = ?
            )
          )
        ORDER BY f.id",
        id,
        id,
        id,
        id,
        id,
        id
    )
    .fetch_all(db)
    .await?;
    let food_ids = serde_json::to_string(&food_cores.iter().map(|f| f.id).collect::<Vec<_>>())
        .map_err(|e| Error::Internal(e.to_string()))?;
    let portions = sqlx::query_as!(
        FoodPortion,
        "SELECT food_id, unit, grams FROM food_portions
         WHERE food_id IN (SELECT value FROM json_each(?)) ORDER BY food_id, unit",
        food_ids
    )
    .fetch_all(db)
    .await?;
    let aliases = sqlx::query_as!(
        Alias,
        "SELECT id, food_id, alias, alias_norm FROM food_aliases
         WHERE user_id = ? ORDER BY id",
        id
    )
    .fetch_all(db)
    .await?;
    let user_portions = sqlx::query_as!(
        UserPortion,
        "SELECT phrase_norm, food_id, grams FROM user_portions
         WHERE user_id = ? ORDER BY phrase_norm",
        id
    )
    .fetch_all(db)
    .await?;
    let products = sqlx::query_as!(
        Product,
        "SELECT food_id, last_servings, last_used_at AS \"last_used_at: Ts\" FROM user_products
         WHERE user_id = ? ORDER BY food_id",
        id
    )
    .fetch_all(db)
    .await?;
    let username = sqlx::query_scalar!("SELECT username FROM users WHERE id = ?", id)
        .fetch_one(db)
        .await?;

    let food_ref: HashMap<i64, (String, Option<String>)> = food_cores
        .iter()
        .map(|f| (f.id, (f.source.clone(), f.source_ref.clone())))
        .collect();
    let mut items = group(item_cores, |i| i.meal_id.clone());
    let mut portions = group(portions, |p| p.food_id);

    let meals = meal_cores
        .into_iter()
        .map(|core| Meal {
            items: items
                .remove(&core.id)
                .unwrap_or_default()
                .into_iter()
                .map(|i| Item {
                    food: i
                        .food_id
                        .and_then(|f| food_ref.get(&f))
                        .map(|(s, r)| FoodRef {
                            source: s.clone(),
                            source_ref: r.clone(),
                        }),
                    core: i,
                })
                .collect(),
            core,
        })
        .collect();
    let foods = food_cores
        .into_iter()
        .map(|core| Food {
            portions: portions.remove(&core.id).unwrap_or_default(),
            core,
        })
        .collect();

    let doc = Doc {
        format: FORMAT.into(),
        version: VERSION,
        exported_at: rfc3339(now_ms())?,
        username: Some(username),
        profile,
        profile_versions,
        days,
        meals,
        foods,
        aliases,
        portions: user_portions,
        products,
    };

    let mut res = Json(doc).into_response();
    let headers = res.headers_mut();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    headers.insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    headers.insert(
        CONTENT_DISPOSITION,
        HeaderValue::from_str(&format!(
            "attachment; filename=\"cupful-backup-{today}.json\""
        ))
        .map_err(|e| Error::Internal(e.to_string()))?,
    );
    Ok(res)
}
