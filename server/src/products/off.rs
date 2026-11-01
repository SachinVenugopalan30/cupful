// SPDX-License-Identifier: AGPL-3.0-or-later
//! Open Food Facts lookup, the stored copy and its refresh.

use std::time::Instant;

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use serde_json::Value;
use sqlx::SqlitePool;

use super::{Food, check_per_100g, gtin_ok, save, set_portions, text};
use crate::AppState;
use crate::auth::User;
use crate::error::{Error, Result};
use crate::time::now_ms;

const FIELDS: &str =
    "product_name,brands,nutriments,serving_size,serving_quantity,serving_quantity_unit";
const REFETCH_AFTER_MS: i64 = 90 * 24 * 60 * 60 * 1000;

pub async fn lookup(
    State(s): State<AppState>,
    user: User,
    Path(barcode): Path<String>,
) -> Result<Json<Value>> {
    if !gtin_ok(&barcode) {
        return Err(Error::bad_request(
            "barcode must be 8-14 digits with a valid check digit",
        ));
    }
    let now = now_ms();
    let food = match stored(&s.db, user.id, &barcode).await? {
        Some(f)
            if f.source != "off"
                || (f.fetched_at.unwrap_or(0) > now - REFETCH_AFTER_MS
                    && !f.needs_liquid_refetch()) =>
        {
            f
        }
        // Missing or due for a refetch; a failed refetch keeps the stored copy.
        stored => match (fetch(&s, user.id, &barcode).await, stored) {
            (Ok(Some(f)), _) => store_off(&s.db, f, now).await?,
            (_, Some(old)) => old,
            (Ok(None), None) => {
                return Err(Error::Coded(
                    StatusCode::NOT_FOUND,
                    "product_not_found",
                    "product not found; enter it from the label".into(),
                ));
            }
            (Err(e), None) => return Err(e),
        },
    };
    save(&s.db, user.id, food.id, None, now).await?;
    Ok(Json(food.json()))
}

/// The best stored entry for `barcode`: the user's own label entry, then the Open Food Facts copy.
/// Other users' label entries are never visible.
async fn stored(db: &SqlitePool, user_id: i64, barcode: &str) -> Result<Option<Food>> {
    Ok(sqlx::query_as!(
        Food,
        "SELECT id, source, name, brand, barcode, kcal, protein_g, carbs_g, fat_g, serving_g,
                serving_text, fetched_at
         FROM foods
         WHERE barcode = ? AND ((source = 'off' AND user_id IS NULL)
                                OR (source = 'user' AND user_id = ?))
         ORDER BY source = 'off', id DESC
         LIMIT 1",
        barcode,
        user_id
    )
    .fetch_optional(db)
    .await?)
}

/// Asks Open Food Facts. `Ok(None)`: not found, unusable, or lookups are switched off.
async fn fetch(s: &AppState, user_id: i64, barcode: &str) -> Result<Option<Food>> {
    if !s.config.product_lookup {
        return Ok(None);
    }
    if !s
        .products
        .limiter
        .allow(&user_id.to_string(), Instant::now())
    {
        return Err(Error::Coded(
            StatusCode::TOO_MANY_REQUESTS,
            "rate_limited",
            "too many product lookups this hour; enter it from the label".into(),
        ));
    }
    let failed = |e: String| {
        tracing::warn!("Open Food Facts lookup failed: {e}");
        Error::Coded(
            StatusCode::BAD_GATEWAY,
            "lookup_failed",
            "product lookup failed; enter it from the label".into(),
        )
    };
    let url = format!(
        "{}/api/v2/product/{barcode}?fields={FIELDS}",
        s.config.off_base_url.trim_end_matches('/')
    );
    let get = || s.products.http.get(&url).send();
    let mut res = get().await;
    if res.as_ref().is_ok_and(|r| r.status().is_server_error()) {
        res = get().await; // one retry, 5xx only
    }
    let res = res.map_err(|e| failed(e.to_string()))?;
    if res.status() == StatusCode::NOT_FOUND {
        return Ok(None);
    }
    if !res.status().is_success() {
        return Err(failed(res.status().to_string()));
    }
    let bytes = res.bytes().await.map_err(|e| failed(e.to_string()))?;
    let body: Value = serde_json::from_slice(&bytes).map_err(|e| failed(e.to_string()))?;
    Ok(from_off(barcode, &body))
}

/// Numbers sometimes arrive as strings.
fn num(v: &Value) -> Option<f64> {
    v.as_f64().or_else(|| v.as_str()?.trim().parse().ok())
}

/// A serving quantity in millilitres or grams. Liquids are treated as 1 ml = 1 g for nutrient
/// math; the product sheet and logged portion keep the millilitre label.
fn serving_amount(p: &Value) -> Option<(f64, &'static str)> {
    let q = num(&p["serving_quantity"])?;
    let unit = p["serving_quantity_unit"]
        .as_str()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    let (amount, unit) = match unit.as_str() {
        "g" => (q, "g"),
        "ml" => (q, "ml"),
        "cl" => (q * 10.0, "ml"),
        "l" => (q * 1000.0, "ml"),
        "oz" => (q * 28.3495, "g"),
        "fl oz" | "floz" => (q * 29.5735, "ml"),
        _ => return None,
    };
    (amount > 0.0 && amount <= 10_000.0).then_some((amount, unit))
}

fn mentions_unit(text: &str, unit: &str) -> bool {
    let t = text.to_ascii_lowercase();
    if unit == "ml" {
        t.contains("ml")
            || t.contains("millilit")
            || t.contains("litre")
            || t.contains("liter")
            || t.split(|c: char| !c.is_ascii_alphanumeric())
                .any(|w| matches!(w, "l" | "cl" | "fl"))
    } else {
        t.contains('g') || t.contains("gram")
    }
}

/// Maps an Open Food Facts v2 product; `None` without plausible per-100 g values.
fn from_off(barcode: &str, body: &Value) -> Option<Food> {
    if body["status"] != 1 {
        return None;
    }
    let p = &body["product"];
    let n = &p["nutriments"];
    let kj = || num(&n["energy-kj_100g"]).or_else(|| num(&n["energy_100g"]));
    let kcal = num(&n["energy-kcal_100g"]).or_else(|| kj().map(|kj| kj / 4.184))?;
    let (protein_g, carbs_g, fat_g) = (
        num(&n["proteins_100g"])?,
        num(&n["carbohydrates_100g"])?,
        num(&n["fat_100g"])?,
    );
    check_per_100g(kcal, protein_g, carbs_g, fat_g).ok()?;
    let cap = |s: String, n: usize| s.chars().take(n).collect::<String>();
    let serving = serving_amount(p);
    let serving_g = serving.map(|(amount, _)| amount);
    let mut serving_text = text(p["serving_size"].as_str()).map(|s| cap(s, 100));
    if let Some((amount, unit)) = serving
        && !serving_text
            .as_deref()
            .is_some_and(|text| mentions_unit(text, unit))
    {
        serving_text = Some(match serving_text {
            Some(text) => format!("{text} ({amount} {unit})"),
            None => format!("{amount} {unit}"),
        });
    }
    Some(Food {
        id: 0,
        source: "off".into(),
        name: cap(
            text(p["product_name"].as_str()).unwrap_or_else(|| format!("Product {barcode}")),
            200,
        ),
        brand: text(p["brands"].as_str().and_then(|b| b.split(',').next())).map(|b| cap(b, 200)),
        barcode: Some(barcode.into()),
        kcal,
        protein_g,
        carbs_g,
        fat_g,
        serving_g,
        serving_text,
        fetched_at: None,
    })
}

/// Inserts or refreshes the `off` row for the product's barcode.
async fn store_off(db: &SqlitePool, f: Food, now: i64) -> Result<Food> {
    let norm = crate::nutrition::foods::normalize(&f.name);
    let mut tx = db.begin().await?;
    let id = sqlx::query_scalar!(
        "INSERT INTO foods (source, source_ref, name, name_norm, brand, barcode, kcal, protein_g,
             carbs_g, fat_g, serving_g, serving_text, fetched_at)
         VALUES ('off', ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT (source, source_ref) WHERE source IN ('staple', 'usda', 'off')
         DO UPDATE SET name = excluded.name, name_norm = excluded.name_norm,
             brand = excluded.brand, kcal = excluded.kcal, protein_g = excluded.protein_g,
             carbs_g = excluded.carbs_g, fat_g = excluded.fat_g, serving_g = excluded.serving_g,
             serving_text = excluded.serving_text, fetched_at = excluded.fetched_at
         RETURNING id",
        f.barcode,
        f.name,
        norm,
        f.brand,
        f.barcode,
        f.kcal,
        f.protein_g,
        f.carbs_g,
        f.fat_g,
        f.serving_g,
        f.serving_text,
        now
    )
    .fetch_one(&mut *tx)
    .await?;
    set_portions(&mut tx, id, f.serving_g, f.serving_text.as_deref()).await?;
    tx.commit().await?;
    Ok(Food {
        id,
        fetched_at: Some(now),
        ..f
    })
}
