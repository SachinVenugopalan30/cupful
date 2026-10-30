// SPDX-License-Identifier: AGPL-3.0-or-later
//! "Enter from the label": a user food private to its owner.

use axum::Json;
use axum::extract::State;
use serde::Deserialize;
use serde_json::Value;

use super::{Food, check_per_100g, gtin_ok, save, set_portions, text};
use crate::AppState;
use crate::auth::User;
use crate::error::{Error, JsonBody, Result};
use crate::time::now_ms;

#[derive(Deserialize)]
pub struct LabelIn {
    name: String,
    brand: Option<String>,
    barcode: String,
    /// `100g` or `serving` (then `serving_g` is required).
    basis: String,
    kcal: f64,
    protein_g: f64,
    carbs_g: f64,
    fat_g: f64,
    serving_g: Option<f64>,
    serving_text: Option<String>,
}

fn label_food(b: LabelIn) -> std::result::Result<Food, String> {
    let name = b.name.trim().to_string();
    if name.is_empty() || name.chars().count() > 200 {
        return Err("name must be 1-200 characters".into());
    }
    let brand = text(b.brand.as_deref());
    if brand.as_ref().is_some_and(|b| b.chars().count() > 200) {
        return Err("brand is longer than 200 characters".into());
    }
    let serving_text = text(b.serving_text.as_deref());
    if serving_text
        .as_ref()
        .is_some_and(|s| s.chars().count() > 100)
    {
        return Err("serving_text is longer than 100 characters".into());
    }
    if !gtin_ok(&b.barcode) {
        return Err("barcode must be 8-14 digits with a valid check digit".into());
    }
    if b.serving_g
        .is_some_and(|g| !(g.is_finite() && g > 0.0 && g <= 2000.0))
    {
        return Err("serving_g must be more than 0 and at most 2000".into());
    }
    let factor = match (b.basis.as_str(), b.serving_g) {
        ("100g", _) => 1.0,
        ("serving", Some(g)) => 100.0 / g,
        ("serving", None) => return Err("per-serving values need serving_g".into()),
        _ => return Err("basis must be \"100g\" or \"serving\"".into()),
    };
    let (kcal, protein_g, carbs_g, fat_g) = (
        b.kcal * factor,
        b.protein_g * factor,
        b.carbs_g * factor,
        b.fat_g * factor,
    );
    check_per_100g(kcal, protein_g, carbs_g, fat_g)?;
    Ok(Food {
        id: 0,
        source: "user".into(),
        name,
        brand,
        barcode: Some(b.barcode),
        kcal,
        protein_g,
        carbs_g,
        fat_g,
        serving_g: b.serving_g,
        serving_text,
        fetched_at: None,
    })
}

/// "Enter from the label": a `user` food private to the caller for the barcode, saved to the user's products.
/// Entering the same barcode again updates the user's earlier entry.
pub async fn create(
    State(s): State<AppState>,
    user: User,
    JsonBody(body): JsonBody<LabelIn>,
) -> Result<Json<Value>> {
    let f = label_food(body).map_err(Error::bad_request)?;
    let (norm, now) = (crate::nutrition::foods::normalize(&f.name), now_ms());
    let mut tx = s.db.begin_with("BEGIN IMMEDIATE").await?;
    let existing = sqlx::query_scalar!(
        "SELECT id FROM foods
         WHERE barcode = ? AND source = 'user' AND user_id = ?",
        f.barcode,
        user.id
    )
    .fetch_optional(&mut *tx)
    .await?;
    let id = match existing {
        Some(id) => {
            sqlx::query!(
                "UPDATE foods SET name = ?, name_norm = ?, brand = ?, kcal = ?, protein_g = ?,
                     carbs_g = ?, fat_g = ?, serving_g = ?, serving_text = ?
                 WHERE id = ?",
                f.name,
                norm,
                f.brand,
                f.kcal,
                f.protein_g,
                f.carbs_g,
                f.fat_g,
                f.serving_g,
                f.serving_text,
                id
            )
            .execute(&mut *tx)
            .await?;
            id
        }
        None => {
            sqlx::query_scalar!(
                "INSERT INTO foods (source, source_ref, user_id, name, name_norm, brand, barcode,
                     kcal, protein_g, carbs_g, fat_g, serving_g, serving_text)
                 VALUES ('user', ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                 RETURNING id",
                user.id.to_string(),
                user.id,
                f.name,
                norm,
                f.brand,
                f.barcode,
                f.kcal,
                f.protein_g,
                f.carbs_g,
                f.fat_g,
                f.serving_g,
                f.serving_text
            )
            .fetch_one(&mut *tx)
            .await?
        }
    };
    set_portions(&mut tx, id, f.serving_g, f.serving_text.as_deref()).await?;
    save(&mut *tx, user.id, id, None, now).await?;
    tx.commit().await?;
    Ok(Json(Food { id, ..f }.json()))
}
