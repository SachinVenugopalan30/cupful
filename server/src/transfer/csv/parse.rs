// SPDX-License-Identifier: AGPL-3.0-or-later
//! Cell parsing and the row/draft types the importer builds from a CSV file.

use sha2::{Digest, Sha256};

use crate::error::{Error, Result};
use jiff::civil::Date;

pub(super) fn line_err(line: u64, msg: impl std::fmt::Display) -> Error {
    Error::bad_request(format!("line {line}: {msg}"))
}

#[derive(Default)]
pub(super) struct DayAccum {
    pub(super) steps: Option<(i64, u64)>,
    pub(super) gym_minutes: Option<(i64, u64)>,
    pub(super) trained: Option<(Vec<String>, u64)>,
    pub(super) weight_kg: Option<(f64, u64)>,
}

pub(super) struct DraftItem {
    pub(super) index: u32,
    pub(super) name: String,
    pub(super) portion: String,
    pub(super) kcal: f64,
    pub(super) protein_g: f64,
    pub(super) carbs_g: f64,
    pub(super) fat_g: f64,
}

pub(super) struct Draft {
    pub(super) line: u64,
    pub(super) date: Date,
    pub(super) meal_type: String,
    pub(super) description: String,
    /// The CSV `meal_id` cell for present-id files; empty for legacy files.
    pub(super) raw_id: String,
    pub(super) legacy: bool,
    pub(super) legacy_hash: String,
    pub(super) legacy_occurrence: u32,
    pub(super) items: Vec<DraftItem>,
}

pub(super) struct ParsedRow {
    pub(super) line: u64,
    pub(super) date: Date,
    pub(super) meal_id: Option<String>,
    pub(super) item_index: Option<u32>,
    pub(super) meal_type: String,
    pub(super) description: String,
    pub(super) name: String,
    pub(super) portion: String,
    pub(super) kcal: Option<f64>,
    pub(super) protein_g: Option<f64>,
    pub(super) carbs_g: Option<f64>,
    pub(super) fat_g: Option<f64>,
}

pub(super) fn parse_f64(cell: &str, line: u64, what: &str) -> Result<Option<f64>> {
    if cell.is_empty() {
        return Ok(None);
    }
    cell.parse::<f64>()
        .map(Some)
        .map_err(|_| line_err(line, format!("{what} must be a number")))
}

pub(super) fn parse_i64(cell: &str, line: u64, what: &str) -> Result<Option<i64>> {
    if cell.is_empty() {
        return Ok(None);
    }
    cell.parse::<i64>()
        .map(Some)
        .map_err(|_| line_err(line, format!("{what} must be an integer")))
}

/// Records one day's value for `what`, rejecting a second, different value on that date.
pub(super) fn merge<T: PartialEq>(
    slot: &mut Option<(T, u64)>,
    value: T,
    line: u64,
    what: &str,
    date: &str,
) -> Result<()> {
    if let Some((old, _)) = slot
        && *old != value
    {
        return Err(line_err(line, format!("conflicting {what} for {date}")));
    }
    slot.get_or_insert((value, line));
    Ok(())
}

pub(super) fn parse_index(cell: &str, line: u64) -> Result<Option<u32>> {
    if cell.is_empty() {
        return Ok(None);
    }
    cell.parse::<u32>()
        .map(Some)
        .map_err(|_| line_err(line, "item_index must be a non-negative integer"))
}

pub(super) fn normalized_type(cell: &str, line: u64) -> Result<String> {
    let t = cell.trim().to_ascii_lowercase();
    if !["breakfast", "lunch", "dinner", "snack"].contains(&t.as_str()) {
        return Err(line_err(
            line,
            format!("unknown meal type {cell:?}; use breakfast, lunch, dinner or snack"),
        ));
    }
    Ok(t)
}

pub(super) fn canonical_row(r: &ParsedRow) -> String {
    let n = |v: Option<f64>| v.map(|v| v.to_string()).unwrap_or_default();
    [
        r.date.to_string(),
        r.meal_type.clone(),
        r.description.clone(),
        r.item_index.map(|i| i.to_string()).unwrap_or_default(),
        r.name.clone(),
        r.portion.clone(),
        n(r.kcal),
        n(r.protein_g),
        n(r.carbs_g),
        n(r.fat_g),
    ]
    .join("\u{1f}")
}

pub(super) fn sha256_hex(s: &str) -> String {
    Sha256::digest(s.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

pub(super) fn unguard(s: &str) -> String {
    let mut chars = s.chars();
    if chars.next() == Some('\'') && chars.next().is_some_and(|c| "=+-@".contains(c)) {
        s[1..].to_string()
    } else {
        s.to_string()
    }
}
