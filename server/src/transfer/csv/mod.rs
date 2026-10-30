// SPDX-License-Identifier: AGPL-3.0-or-later
//! CSV export and Fuel Ledger import (PLAN.md §14 M5).

mod export;
mod import;
mod parse;

pub use export::export;
pub use import::import;

const HEADER: [&str; 15] = [
    "date",
    "meal_id",
    "item_index",
    "meal",
    "description",
    "food",
    "portion",
    "kcal",
    "protein_g",
    "carbs_g",
    "fat_g",
    "steps",
    "gym_minutes",
    "trained",
    "weight_kg",
];
