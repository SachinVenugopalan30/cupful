// SPDX-License-Identifier: AGPL-3.0-or-later
//! USDA FoodData Central importer.

use std::collections::HashMap;

use serde::Deserialize;
use sqlx::SqlitePool;

use super::seed::seed;
use super::text::normalize;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UsdaFood {
    fdc_id: i64,
    description: String,
    #[serde(default)]
    food_nutrients: Vec<UsdaNutrient>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UsdaNutrient {
    nutrient: UsdaNutrientId,
    amount: Option<f64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UsdaNutrientId {
    number: String,
    #[serde(default)]
    unit_name: String,
}

impl UsdaFood {
    /// kcal, protein, carbs, fat per 100 g. Foundation foods often give energy only as Atwater
    /// factors (958 specific, 957 general) and carbs only "by summation" (205.2).
    fn macros(&self) -> Option<[f64; 4]> {
        let get = |number: &str, unit: &str| {
            self.food_nutrients
                .iter()
                .find(|n| {
                    n.nutrient.number == number && n.nutrient.unit_name.eq_ignore_ascii_case(unit)
                })
                .and_then(|n| n.amount)
        };
        Some([
            get("208", "kcal")
                .or_else(|| get("958", "kcal"))
                .or_else(|| get("957", "kcal"))?,
            get("203", "g")?,
            get("205", "g").or_else(|| get("205.2", "g"))?,
            get("204", "g")?,
        ])
    }
}

/// Imports a FoodData Central Foundation or SR Legacy JSON download (the unzipped `.json`) as
/// `usda` foods. Idempotent: rows upsert on the FDC id. Returns (imported, skipped).
pub async fn import_usda(
    db: &SqlitePool,
    path: &std::path::Path,
) -> Result<(usize, usize), String> {
    let file = std::fs::File::open(path).map_err(|e| format!("open {}: {e}", path.display()))?;
    // One top-level key: "FoundationFoods" or "SRLegacyFoods". The Foundation file has null entries.
    let data: HashMap<String, Vec<Option<UsdaFood>>> =
        serde_json::from_reader(std::io::BufReader::new(file)).map_err(|e| {
            format!(
                "{}: not a FoodData Central JSON download: {e}",
                path.display()
            )
        })?;
    let foods = match (data.get("FoundationFoods"), data.get("SRLegacyFoods")) {
        (Some(f), _) | (None, Some(f)) => f,
        _ => return Err("expected a Foundation or SR Legacy JSON file".into()),
    };
    let db_err = |e: sqlx::Error| e.to_string();
    let mut tx = db.begin().await.map_err(db_err)?;
    let (mut imported, mut skipped) = (0, 0);
    for f in foods.iter().flatten() {
        let Some([kcal, protein, carbs, fat]) = f
            .macros()
            .filter(|m| (0.0..=crate::nutrition::MAX_KCAL_PER_100G).contains(&m[0]))
        else {
            skipped += 1;
            continue;
        };
        let norm = normalize(&f.description);
        // ponytail: "fried" in the name is the only cooking-fat signal USDA names give us.
        let prepared = norm.split(' ').any(|w| w == "fried");
        let fdc = f.fdc_id.to_string();
        sqlx::query!(
            "INSERT INTO foods (source, source_ref, name, name_norm, is_prepared, kcal, protein_g, carbs_g, fat_g)
             VALUES ('usda', ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT (source, source_ref) WHERE source IN ('staple', 'usda', 'off') DO UPDATE SET
                name = excluded.name, name_norm = excluded.name_norm,
                is_prepared = excluded.is_prepared, kcal = excluded.kcal,
                protein_g = excluded.protein_g, carbs_g = excluded.carbs_g, fat_g = excluded.fat_g",
            fdc,
            f.description,
            norm,
            prepared,
            kcal,
            protein,
            carbs,
            fat
        )
        .execute(&mut *tx)
        .await
        .map_err(db_err)?;
        imported += 1;
    }
    tx.commit().await.map_err(db_err)?;
    // Shipped aliases that point at USDA entries can resolve now.
    seed(db).await.map_err(db_err)?;
    Ok((imported, skipped))
}
