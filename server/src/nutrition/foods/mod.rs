// SPDX-License-Identifier: AGPL-3.0-or-later
//! Food database: name normalization, shipped staples and aliases, the USDA importer, and matching
//! (PLAN.md §8 step 4, "Food database").

mod find;
mod seed;
#[cfg(test)]
mod tests;
mod text;
mod usda;

pub use find::{find, visible};
pub use seed::seed;
pub use text::{normalize, portion_unit, unit_keys};
pub use usda::import_usda;

use serde::Serialize;

/// Classification thresholds on the token-overlap score (0..=1). Tuned with the golden tests.
pub const MATCHED_MIN: f64 = 0.6;
pub const AMBIGUOUS_MIN: f64 = 0.3;
/// A match also needs this lead over the runner-up, else it's ambiguous.
pub const MARGIN: f64 = 0.15;
pub const MAX_CANDIDATES: usize = 3;

/// A food row with its per-100 g values.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Food {
    pub id: i64,
    pub name: String,
    pub source: String,
    pub is_prepared: bool,
    pub kcal: f64,
    pub protein_g: f64,
    pub carbs_g: f64,
    pub fat_g: f64,
    pub default_portion_g: Option<f64>,
}

#[derive(Debug, PartialEq)]
pub enum Match<T = Food> {
    Matched(T),
    /// Up to `MAX_CANDIDATES`, best first.
    Ambiguous(Vec<T>),
    Unmatched,
}
