// SPDX-License-Identifier: AGPL-3.0-or-later
//! The user's saved products.

use axum::Json;
use axum::extract::{Path, State};
use serde_json::{Value, json};

use super::Food;
use crate::AppState;
use crate::auth::User;
use crate::error::{Error, Result};

pub async fn list(State(s): State<AppState>, user: User) -> Result<Json<Value>> {
    let rows = sqlx::query!(
        "SELECT f.id, f.source, f.name, f.brand, f.barcode, f.kcal, f.protein_g, f.carbs_g,
                f.fat_g, f.serving_g, f.serving_text, f.fetched_at, p.last_servings,
                p.last_used_at
         FROM user_products p JOIN foods f ON f.id = p.food_id
         WHERE p.user_id = ? AND (f.user_id IS NULL OR f.user_id = p.user_id)
         ORDER BY p.last_used_at DESC",
        user.id
    )
    .fetch_all(&s.db)
    .await?;
    let products: Vec<Value> = rows
        .into_iter()
        .map(|r| {
            let mut v = Food {
                id: r.id,
                source: r.source,
                name: r.name,
                brand: r.brand,
                barcode: r.barcode,
                kcal: r.kcal,
                protein_g: r.protein_g,
                carbs_g: r.carbs_g,
                fat_g: r.fat_g,
                serving_g: r.serving_g,
                serving_text: r.serving_text,
                fetched_at: r.fetched_at,
            }
            .json();
            v["last_servings"] = json!(r.last_servings);
            v["last_used_at"] = json!(r.last_used_at);
            v
        })
        .collect();
    Ok(Json(json!({ "products": products })))
}

/// Removes a product from the user's list; the shared food stays.
pub async fn remove(
    State(s): State<AppState>,
    user: User,
    Path(food_id): Path<i64>,
) -> Result<Json<Value>> {
    let n = sqlx::query!(
        "DELETE FROM user_products WHERE user_id = ? AND food_id = ?",
        user.id,
        food_id
    )
    .execute(&s.db)
    .await?
    .rows_affected();
    if n == 0 {
        return Err(Error::not_found());
    }
    Ok(Json(json!({})))
}
