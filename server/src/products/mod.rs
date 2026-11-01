// SPDX-License-Identifier: AGPL-3.0-or-later
//! Packaged products by barcode (PLAN.md §8 "Packaged products").

mod label;
mod off;
mod saved;

pub use label::create;
pub use off::lookup;
pub use saved::{list, remove};

use std::time::Duration;

use serde_json::{Value, json};
use sqlx::{SqliteConnection, SqliteExecutor};

use crate::Config;
use crate::auth::RateLimiter;
use crate::error::Result;
use crate::nutrition::foods::portion_unit;
use crate::nutrition::{MAX_KCAL_PER_100G, r1};

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
    /// Liquids keep a millilitre serving; everything else is grams.
    fn serving_unit(&self) -> &'static str {
        let Some(text) = self.serving_text.as_deref() else {
            return "g";
        };
        let t = text.to_ascii_lowercase();
        let volume = t.contains("ml")
            || t.contains("millilit")
            || t.contains("litre")
            || t.contains("liter")
            || t.split(|c: char| !c.is_ascii_alphanumeric())
                .any(|w| matches!(w, "l" | "cl" | "fl"));
        if volume { "ml" } else { "g" }
    }

    /// Older liquid rows stored before millilitre servings were supported have no amount.
    fn needs_liquid_refetch(&self) -> bool {
        self.source == "off" && self.serving_g.is_none() && self.serving_unit() == "ml"
    }

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
            "serving_unit": self.serving_unit(),
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

/// Makes the serving usable by the estimate pipeline: the food's default portion, a `serving`
/// portion, and the unit in `serving_text` ("1 scoop (31 g)" -> `scoop` = 31 g).
async fn set_portions(
    conn: &mut SqliteConnection,
    food_id: i64,
    serving_g: Option<f64>,
    serving_text: Option<&str>,
) -> Result<()> {
    sqlx::query!(
        "UPDATE foods SET default_portion_g = ? WHERE id = ?",
        serving_g,
        food_id
    )
    .execute(&mut *conn)
    .await?;
    sqlx::query!("DELETE FROM food_portions WHERE food_id = ?", food_id)
        .execute(&mut *conn)
        .await?;
    let Some(g) = serving_g else {
        return Ok(());
    };
    let mut portions = vec![("serving".to_string(), g)];
    if let Some((unit, amount)) = serving_text.and_then(|t| portion_unit(t, None))
        && !matches!(unit.as_str(), "g" | "ml")
    {
        portions.push((unit, g / amount));
    }
    for (unit, grams) in portions {
        sqlx::query!(
            "INSERT OR IGNORE INTO food_portions (food_id, unit, grams) VALUES (?, ?, ?)",
            food_id,
            unit,
            grams
        )
        .execute(&mut *conn)
        .await?;
    }
    Ok(())
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
