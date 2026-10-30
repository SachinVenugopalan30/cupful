// SPDX-License-Identifier: AGPL-3.0-or-later
//! Household measures: grams for a unit, the oil rule, amount formatting.

use sqlx::SqlitePool;

use super::foods::{self, normalize};

pub(super) fn has(text: &str, words: &[&str]) -> bool {
    text.split(' ').any(|w| words.contains(&w))
}

/// Grams for one `unit` of `food` (both normalized), from the household measures table.
pub(super) fn unit_grams(unit: &str, food: &str) -> Option<f64> {
    let rice = has(food, &["rice", "chawal"]);
    let wet = has(
        food,
        &[
            "curry", "dal", "daal", "dhal", "soup", "sambar", "rajma", "chole", "rasam", "kadhi",
            "stew", "chili", "chilli",
        ],
    );
    let drink = has(
        food,
        &[
            "juice",
            "milk",
            "coffee",
            "tea",
            "latte",
            "cappuccino",
            "smoothie",
            "shake",
            "soda",
            "cola",
            "beer",
            "wine",
            "water",
            "lassi",
            "chai",
            "espresso",
        ],
    );
    let fat = has(food, &["oil", "ghee", "butter"]);
    let roti = has(food, &["roti", "chapati", "chapathi", "phulka"]);
    let piece =
        unit.is_empty() || has(unit, &["piece", "pc", "whole", "roti", "chapati", "breast"]);
    Some(match unit {
        "g" | "gm" | "gram" | "gr" => 1.0,
        "kg" | "kilogram" => 1000.0,
        // ponytail: ml as grams (water density); fine for drinks and curries, not for oil or honey.
        "ml" | "milliliter" | "millilitre" => 1.0,
        "l" | "liter" | "litre" => 1000.0,
        "oz" | "ounce" => 28.35,
        "lb" | "pound" => 453.6,
        "small cup" | "katori" if rice => 150.0,
        "cup" if rice => 200.0,
        "plate" if rice => 300.0,
        "bowl" | "katori" | "cup" if wet => 250.0,
        "tsp" | "teaspoon" if fat => 4.5,
        "tbsp" | "tablespoon" if fat => 13.5,
        "cup" if drink => 240.0,
        "glass" | "mug" if drink => 250.0,
        "can" if drink => 355.0,
        "shot" | "splash" if drink => 30.0,
        _ if roti && piece => 40.0,
        _ if has(food, &["chicken"]) && has(food, &["breast"]) && piece => 130.0,
        _ => return None,
    })
}

/// The food name without size words, for matching: "medium fries" is fries.
pub(super) fn without_sizes(food: &str) -> String {
    const SIZES: [&str; 6] = ["small", "medium", "large", "regular", "big", "extra"];
    let kept: Vec<&str> = food.split(' ').filter(|w| !SIZES.contains(w)).collect();
    if kept.is_empty() {
        food.to_string()
    } else {
        kept.join(" ")
    }
}

pub(super) fn is_weight(unit: &str) -> bool {
    matches!(
        unit,
        "g" | "gm"
            | "gram"
            | "gr"
            | "kg"
            | "kilogram"
            | "ml"
            | "milliliter"
            | "millilitre"
            | "l"
            | "liter"
            | "litre"
            | "oz"
            | "ounce"
            | "lb"
            | "pound"
    )
}

/// A level spoon of anything, when neither the measures table nor USDA has the food's own spoon.
pub(super) fn spoon_grams(unit: &str) -> Option<f64> {
    match unit {
        "tsp" | "teaspoon" => Some(5.0),
        "tbsp" | "tablespoon" => Some(15.0),
        _ => None,
    }
}

/// USDA grams for one `unit` of the matched food ("scoop" of whey → 28.7 g).
pub(super) async fn food_portion(
    db: &SqlitePool,
    food_id: i64,
    unit: &str,
) -> std::result::Result<Option<f64>, sqlx::Error> {
    for key in foods::unit_keys(unit) {
        let g = sqlx::query_scalar!(
            "SELECT grams FROM food_portions WHERE food_id = ? AND unit = ?",
            food_id,
            key
        )
        .fetch_optional(db)
        .await?;
        if g.is_some() {
            return Ok(g);
        }
    }
    Ok(None)
}
/// Cooking methods that use added oil (the oil rule, §8 step 5).
pub(super) fn uses_oil(cooking: &str) -> bool {
    let c = normalize(cooking);
    [
        "shallow fried",
        "pan fried",
        "stir fried",
        "saut",
        "roasted in oil",
        "tadka",
        "tempered",
        "tempering",
    ]
    .iter()
    .any(|m| c.contains(m))
}

pub(super) fn fmt_amount(a: f64) -> String {
    if a.fract() == 0.0 {
        format!("{a:.0}")
    } else {
        format!("{a}")
    }
}
