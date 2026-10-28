// SPDX-License-Identifier: AGPL-3.0-or-later
//! Golden cases (`data/golden/*.json`): fixed descriptions, the expected items, and a recorded
//! model reply. CI replays the recording; `cupful eval` asks a live provider instead.

use std::path::Path;

use serde::Deserialize;
use serde_json::Value;

use super::Estimate;

pub const DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../data/golden");
/// Grams may be off by this fraction.
pub const GRAMS_TOLERANCE: f64 = 0.15;

#[derive(Deserialize)]
pub struct Case {
    pub description: String,
    pub recorded: Recorded,
    pub expected: Vec<Expected>,
    /// Items the pipeline should drop with a warning.
    #[serde(default)]
    pub expected_warnings: usize,
}

/// A model reply with where it came from, so stale recordings show.
#[derive(Deserialize)]
pub struct Recorded {
    pub provider: String,
    pub model: String,
    pub recorded_at: String,
    pub output: Value,
}

#[derive(Deserialize)]
pub struct Expected {
    /// The food's name in the database; for an ambiguous item, one of the candidates; `None` when unmatched.
    pub food: Option<String>,
    #[serde(rename = "match")]
    pub status: String,
    pub grams: f64,
}

/// Every case in `dir`, by file name.
pub fn load(dir: &Path) -> Result<Vec<(String, Case)>, String> {
    let mut cases = vec![];
    for entry in std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))? {
        let path = entry.map_err(|e| e.to_string())?.path();
        if path.extension().is_some_and(|e| e == "json") {
            let text = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
            let case =
                serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
            cases.push((
                path.file_name().unwrap().to_string_lossy().into_owned(),
                case,
            ));
        }
    }
    cases.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(cases)
}

/// (items right, items expected, what went wrong). Extra items count against the total.
pub fn check(case: &Case, est: &Estimate) -> (usize, usize, Vec<String>) {
    let mut failures = vec![];
    let mut right = 0;
    for (i, want) in case.expected.iter().enumerate() {
        let Some(got) = est.items.get(i) else {
            failures.push(format!("item {i}: missing, expected {:?}", want.food));
            continue;
        };
        let food_ok = match (want.status.as_str(), &want.food) {
            ("ambiguous", Some(f)) => got.candidates.iter().any(|c| &c.name == f),
            (_, f) => got.food_name.as_ref() == f.as_ref(),
        };
        let grams_ok = (got.grams - want.grams).abs() <= GRAMS_TOLERANCE * want.grams;
        if got.status == want.status && food_ok && grams_ok {
            right += 1;
        } else {
            failures.push(format!(
                "item {i} ({}): got {} {:?} {} g, candidates {:?}; expected {} {:?} {} g",
                got.name,
                got.status,
                got.food_name,
                got.grams,
                got.candidates.iter().map(|c| &c.name).collect::<Vec<_>>(),
                want.status,
                want.food,
                want.grams
            ));
        }
    }
    let total = case.expected.len().max(est.items.len());
    for extra in est.items.iter().skip(case.expected.len()) {
        failures.push(format!(
            "unexpected item {} ({:?})",
            extra.name, extra.food_name
        ));
    }
    if est.warnings.len() != case.expected_warnings {
        failures.push(format!(
            "warnings: got {:?}, expected {}",
            est.warnings, case.expected_warnings
        ));
    }
    (right, total, failures)
}
