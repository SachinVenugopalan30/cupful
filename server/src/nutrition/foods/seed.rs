// SPDX-License-Identifier: AGPL-3.0-or-later
//! Shipped staples, aliases and portions, seeded at startup.

use std::collections::HashMap;

use sqlx::SqlitePool;

use super::text::{normalize, portion_unit};

pub(super) static STAPLES: &str = include_str!("../../../../data/staples.csv");
pub(super) static ALIASES: &str = include_str!("../../../../data/aliases.csv");
static PORTIONS: &str = include_str!("../../../../data/portions.csv");

/// Rows of a CSV (header included); quotes and `""` escapes handled by the `csv` crate.
pub(super) fn csv(s: &str) -> Vec<Vec<String>> {
    csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .from_reader(s.as_bytes())
        .records()
        .map(|r| r.expect("shipped csv").iter().map(String::from).collect())
        .collect()
}

/// Upserts the shipped staples and replaces the shipped aliases. Idempotent; runs at startup.
pub async fn seed(db: &SqlitePool) -> Result<(), sqlx::Error> {
    let mut tx = db.begin().await?;
    let mut by_name = HashMap::new();
    // name,is_prepared,kcal,protein_g,carbs_g,fat_g,default_portion_g,portion_text,fdc_id,fdc_description
    for r in csv(STAPLES).into_iter().skip(1) {
        let num = |i: usize| -> f64 { r[i].parse().expect("staples.csv: number") };
        let (name, norm, prepared) = (&r[0], normalize(&r[0]), r[1] == "1");
        let (kcal, protein, carbs, fat) = (num(2), num(3), num(4), num(5));
        let portion = r[6].parse::<f64>().ok();
        let id = sqlx::query_scalar!(
            "INSERT INTO foods (source, source_ref, name, name_norm, is_prepared, kcal, protein_g,
                                carbs_g, fat_g, default_portion_g)
             VALUES ('staple', ?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT (source, source_ref) WHERE source IN ('staple', 'usda', 'off') DO UPDATE SET
                name = excluded.name, name_norm = excluded.name_norm,
                is_prepared = excluded.is_prepared, kcal = excluded.kcal,
                protein_g = excluded.protein_g, carbs_g = excluded.carbs_g, fat_g = excluded.fat_g,
                default_portion_g = excluded.default_portion_g
             RETURNING id",
            r[8],
            name,
            norm,
            prepared,
            kcal,
            protein,
            carbs,
            fat,
            portion
        )
        .fetch_one(&mut *tx)
        .await?;
        by_name.insert(name.clone(), id);
    }
    // fdc_id,amount,unit,grams
    sqlx::query!(
        "DELETE FROM food_portions WHERE food_id IN (SELECT id FROM foods WHERE source = 'staple')"
    )
    .execute(&mut *tx)
    .await?;
    let mut portions = csv(PORTIONS).into_iter().skip(1).collect::<Vec<_>>();
    // Within a food and unit the first row wins: plain before modified, then medium/regular sizes.
    portions.sort_by_key(|r| {
        let t = r[2].to_lowercase();
        (
            t.contains(','),
            !(t.contains("medium") || t.contains("regular")),
        )
    });
    for r in portions {
        let Some((unit, amount)) = portion_unit(&r[2], r[1].parse().ok()) else {
            continue;
        };
        let grams = r[3].parse::<f64>().expect("portions.csv: grams") / amount;
        sqlx::query!(
            "INSERT OR IGNORE INTO food_portions (food_id, unit, grams)
             SELECT id, ?, ? FROM foods WHERE source = 'staple' AND source_ref = ?",
            unit,
            grams,
            r[0]
        )
        .execute(&mut *tx)
        .await?;
    }
    sqlx::query!("DELETE FROM food_aliases WHERE user_id IS NULL")
        .execute(&mut *tx)
        .await?;
    for r in csv(ALIASES).into_iter().skip(1) {
        let food_id = match r[1].strip_prefix("fdc:") {
            // USDA targets exist only after `cupful import-foods`.
            Some(fdc) => {
                sqlx::query_scalar!(
                    "SELECT id FROM foods WHERE source = 'usda' AND source_ref = ?",
                    fdc
                )
                .fetch_optional(&mut *tx)
                .await?
            }
            None => Some(*by_name.get(&r[1]).expect("aliases.csv: unknown staple")),
        };
        if let Some(food_id) = food_id {
            let norm = normalize(&r[0]);
            sqlx::query!(
                "INSERT INTO food_aliases (food_id, alias, alias_norm) VALUES (?, ?, ?)",
                food_id,
                r[0],
                norm
            )
            .execute(&mut *tx)
            .await?;
        }
    }
    tx.commit().await
}
