// SPDX-License-Identifier: AGPL-3.0-or-later
//! Packaged products by barcode (PLAN.md §8 "Packaged products").

use std::time::{Duration, Instant};

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::{SqliteConnection, SqliteExecutor, SqlitePool};

use crate::auth::{RateLimiter, User};
use crate::error::{Error, JsonBody, Result};
use crate::nutrition::{MAX_KCAL_PER_100G, r1};
use crate::time::now_ms;
use crate::{AppState, Config};

const FIELDS: &str =
    "product_name,brands,nutriments,serving_size,serving_quantity,serving_quantity_unit";
const REFETCH_AFTER_MS: i64 = 90 * 24 * 60 * 60 * 1000;
const ATTRIBUTION: &str = "Product data from Open Food Facts (ODbL)";

/// The Open Food Facts client and the per-user lookup limit.
pub struct Lookup {
    http: reqwest::Client,
    limiter: RateLimiter,
}

impl Lookup {
    pub fn new(config: &Config) -> Self {
        let http = reqwest::Client::builder()
            .connect_timeout(config.off_timeout.min(Duration::from_secs(5)))
            .timeout(config.off_timeout)
            // Open Food Facts asks every client to identify itself.
            .user_agent(format!(
                "Cupful/{} (+{})",
                env!("CARGO_PKG_VERSION"),
                config.source_url
            ))
            .build()
            .expect("HTTP client");
        Self {
            http,
            // Only outbound requests count; stored products are free.
            limiter: RateLimiter::new(60, Duration::from_secs(60 * 60)),
        }
    }
}

/// 8–14 digits whose last digit is the GTIN check digit.
pub fn gtin_ok(code: &str) -> bool {
    if !(8..=14).contains(&code.len()) || !code.bytes().all(|b| b.is_ascii_digit()) {
        return false;
    }
    // Weights 1, 3, 1, 3, … from the right, check digit included; the sum is a multiple of 10.
    let sum: u32 = code
        .bytes()
        .rev()
        .enumerate()
        .map(|(i, b)| u32::from(b - b'0') * if i % 2 == 0 { 1 } else { 3 })
        .sum();
    sum.is_multiple_of(10)
}

/// A `foods` row as products use it. Nutrients are per 100 g.
#[derive(Debug)]
struct Food {
    id: i64,
    source: String,
    name: String,
    brand: Option<String>,
    barcode: Option<String>,
    kcal: f64,
    protein_g: f64,
    carbs_g: f64,
    fat_g: f64,
    serving_g: Option<f64>,
    serving_text: Option<String>,
    fetched_at: Option<i64>,
}

impl Food {
    fn json(&self) -> Value {
        let attribution = (self.source == "off").then(|| {
            json!({
                "text": ATTRIBUTION,
                "url": format!("https://world.openfoodfacts.org/product/{}",
                    self.barcode.as_deref().unwrap_or_default()),
            })
        });
        json!({
            "food_id": self.id,
            "source": self.source,
            "barcode": self.barcode,
            "name": self.name,
            "brand": self.brand,
            "kcal": r1(self.kcal),
            "protein_g": r1(self.protein_g),
            "carbs_g": r1(self.carbs_g),
            "fat_g": r1(self.fat_g),
            "serving_g": self.serving_g.map(r1),
            "serving_text": self.serving_text,
            "attribution": attribution,
        })
    }
}

/// Same plausibility rule as the nutrition pipeline: 0–900 kcal per 100 g.
fn check_per_100g(
    kcal: f64,
    protein: f64,
    carbs: f64,
    fat: f64,
) -> std::result::Result<(), String> {
    let ok = |v: f64, hi: f64| v.is_finite() && (0.0..=hi).contains(&v);
    if !ok(kcal, MAX_KCAL_PER_100G) {
        return Err("kcal per 100 g must be between 0 and 900".into());
    }
    if !(ok(protein, 100.0) && ok(carbs, 100.0) && ok(fat, 100.0)) {
        return Err("protein, carbs and fat per 100 g must be between 0 and 100 g".into());
    }
    Ok(())
}

/// Trimmed, non-empty text, or `None`.
fn text(s: Option<&str>) -> Option<String> {
    s.map(str::trim).filter(|s| !s.is_empty()).map(String::from)
}

// ---- lookup ----

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
        Some(f) if f.source != "off" || f.fetched_at.unwrap_or(0) > now - REFETCH_AFTER_MS => f,
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

/// The best stored entry for `barcode`: the user's own label entry, then anyone's (newest first),
/// then the Open Food Facts copy. Other users' private foods are never visible.
async fn stored(db: &SqlitePool, user_id: i64, barcode: &str) -> Result<Option<Food>> {
    let me = user_id.to_string();
    Ok(sqlx::query_as!(
        Food,
        "SELECT id, source, name, brand, barcode, kcal, protein_g, carbs_g, fat_g, serving_g,
                serving_text, fetched_at
         FROM foods
         WHERE barcode = ? AND source IN ('user', 'off') AND (user_id IS NULL OR user_id = ?)
         ORDER BY CASE WHEN source = 'off' THEN 2 WHEN source_ref = ? THEN 0 ELSE 1 END, id DESC
         LIMIT 1",
        barcode,
        user_id,
        me
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
    let serving_g = (p["serving_quantity_unit"] == "g")
        .then(|| num(&p["serving_quantity"]))
        .flatten()
        .filter(|g| *g > 0.0 && *g <= 2000.0);
    let cap = |s: String, n: usize| s.chars().take(n).collect::<String>();
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
        serving_text: text(p["serving_size"].as_str()).map(|s| cap(s, 100)),
        fetched_at: None,
    })
}

/// Inserts or refreshes the `off` row for the product's barcode.
async fn store_off(db: &SqlitePool, f: Food, now: i64) -> Result<Food> {
    let norm = crate::nutrition::foods::normalize(&f.name);
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
    .fetch_one(db)
    .await?;
    Ok(Food {
        id,
        fetched_at: Some(now),
        ..f
    })
}

/// Upserts the user's saved product; `servings` is kept when unknown.
async fn save(
    db: impl SqliteExecutor<'_>,
    user_id: i64,
    food_id: i64,
    servings: Option<f64>,
    now: i64,
) -> Result<()> {
    sqlx::query!(
        "INSERT INTO user_products (user_id, food_id, last_servings, last_used_at)
         VALUES (?, ?, ?, ?)
         ON CONFLICT (user_id, food_id) DO UPDATE SET
             last_servings = coalesce(excluded.last_servings, last_servings),
             last_used_at = excluded.last_used_at",
        user_id,
        food_id,
        servings,
        now
    )
    .execute(db)
    .await?;
    Ok(())
}

/// Sync hook for each `meal.upsert` item with a `food_id`. Returns false when the user may not see
/// that food. A barcoded food lands in the user's saved products.
pub(crate) async fn item_logged(
    conn: &mut SqliteConnection,
    user_id: i64,
    food_id: i64,
    grams: Option<f64>,
    now: i64,
) -> Result<bool> {
    let food = sqlx::query!(
        "SELECT barcode, serving_g FROM foods WHERE id = ? AND (user_id IS NULL OR user_id = ?)",
        food_id,
        user_id
    )
    .fetch_optional(&mut *conn)
    .await?;
    let Some(food) = food else {
        return Ok(false);
    };
    if food.barcode.is_some() {
        let servings = grams.zip(food.serving_g).map(|(g, s)| g / s);
        save(&mut *conn, user_id, food_id, servings, now).await?;
    }
    Ok(true)
}

// ---- label entry ----

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

/// "Enter from the label": a shared `user` food for the barcode, saved to the user's products.
/// Entering the same barcode again updates the user's earlier entry.
pub async fn create(
    State(s): State<AppState>,
    user: User,
    JsonBody(body): JsonBody<LabelIn>,
) -> Result<Json<Value>> {
    let f = label_food(body).map_err(Error::bad_request)?;
    let (me, norm, now) = (
        user.id.to_string(),
        crate::nutrition::foods::normalize(&f.name),
        now_ms(),
    );
    let mut tx = s.db.begin_with("BEGIN IMMEDIATE").await?;
    let existing = sqlx::query_scalar!(
        "SELECT id FROM foods
         WHERE barcode = ? AND source = 'user' AND source_ref = ? AND user_id IS NULL",
        f.barcode,
        me
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
                "INSERT INTO foods (source, source_ref, name, name_norm, brand, barcode, kcal,
                     protein_g, carbs_g, fat_g, serving_g, serving_text)
                 VALUES ('user', ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                 RETURNING id",
                me,
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
    save(&mut *tx, user.id, id, None, now).await?;
    tx.commit().await?;
    Ok(Json(Food { id, ..f }.json()))
}

// ---- saved products ----

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gtin_check_digit() {
        assert!(gtin_ok("4006381333931")); // EAN-13
        assert!(gtin_ok("036000291452")); // UPC-A
        assert!(gtin_ok("96385074")); // EAN-8
        assert!(gtin_ok("00012345600012")); // GTIN-14
        assert!(!gtin_ok("4006381333932")); // bad check digit
        assert!(!gtin_ok("1234565")); // too short
        assert!(!gtin_ok("123456789012345")); // too long
        assert!(!gtin_ok("40063813339a1"));
        assert!(!gtin_ok("+4006381333931"));
        assert!(!gtin_ok(""));
    }
}
