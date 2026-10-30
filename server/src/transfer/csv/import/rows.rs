// SPDX-License-Identifier: AGPL-3.0-or-later
//! Reads the CSV text into parsed meal rows and per-day activity.

use std::collections::BTreeMap;

use jiff::civil::Date;

use super::super::HEADER;
use super::super::parse::{
    DayAccum, ParsedRow, line_err, merge, normalized_type, parse_f64, parse_i64, parse_index,
    unguard,
};
use crate::error::{Error, Result};
use crate::sync::{BODY_PARTS, date_ok, finite_in};

const MAX_RECORDS: usize = 50_000;
const MAX_NAME: usize = 200;
const MAX_PORTION: usize = 200;

/// Every record's activity columns accumulate into `days`; its meal columns into `rows`.
pub(super) fn parse(
    text: &str,
    tomorrow: Date,
) -> Result<(Vec<ParsedRow>, BTreeMap<String, DayAccum>)> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(true)
        .from_reader(text.as_bytes());

    let header = reader
        .headers()
        .map_err(|e| Error::bad_request(format!("invalid CSV header: {e}")))?
        .clone();
    let got: Vec<&str> = header.iter().map(str::trim).collect();
    if got != HEADER {
        return Err(Error::bad_request(format!(
            "CSV header must be: {}",
            HEADER.join(",")
        )));
    }

    let mut rows: Vec<ParsedRow> = Vec::new();
    let mut days: BTreeMap<String, DayAccum> = BTreeMap::new();
    let mut record_no = 0usize;

    for rec in reader.records() {
        let rec = rec.map_err(|e| {
            Error::bad_request(format!(
                "line {}: {e}",
                e.position().map(|p| p.line()).unwrap_or(0)
            ))
        })?;
        record_no += 1;
        let line = rec.position().map(|p| p.line()).unwrap_or(0);
        if record_no > MAX_RECORDS {
            return Err(line_err(line, format!("at most {MAX_RECORDS} records")));
        }
        if rec.len() != HEADER.len() {
            return Err(line_err(
                line,
                format!("expected {} columns, found {}", HEADER.len(), rec.len()),
            ));
        }
        let cells: Vec<String> = rec.iter().map(|c| c.trim().to_string()).collect();
        let date = if cells[0].is_empty() {
            return Err(line_err(line, "date is required"));
        } else {
            cells[0]
                .parse::<Date>()
                .map_err(|_| line_err(line, "date must be YYYY-MM-DD"))?
        };
        date_ok(date, tomorrow).map_err(|e| line_err(line, e))?;

        // Activity applies to the date whichever kind of row carries it.
        let ds = date.to_string();
        let acc = days.entry(ds.clone()).or_default();
        if let Some(v) = parse_i64(&cells[11], line, "steps")? {
            finite_in(v as f64, 0.0, 150_000.0, "steps").map_err(|e| line_err(line, e))?;
            merge(&mut acc.steps, v, line, "steps", &ds)?;
        }
        if let Some(v) = parse_i64(&cells[12], line, "gym_minutes")? {
            finite_in(v as f64, 0.0, 600.0, "gym_minutes").map_err(|e| line_err(line, e))?;
            merge(&mut acc.gym_minutes, v, line, "gym_minutes", &ds)?;
        }
        if !cells[13].is_empty() {
            let parts: Vec<String> = cells[13]
                .split(';')
                .map(str::trim)
                .filter(|p| !p.is_empty())
                .map(str::to_string)
                .collect();
            for p in &parts {
                if !BODY_PARTS.contains(&p.as_str()) {
                    return Err(line_err(line, format!("unknown trained body part {p:?}")));
                }
            }
            merge(&mut acc.trained, parts, line, "trained", &ds)?;
        }
        if let Some(v) = parse_f64(&cells[14], line, "weight_kg")? {
            finite_in(v, 20.0, 400.0, "weight_kg").map_err(|e| line_err(line, e))?;
            merge(&mut acc.weight_kg, v, line, "weight_kg", &ds)?;
        }

        let meal_cols = (1..=10).any(|i| !cells[i].is_empty());
        if !meal_cols {
            continue;
        }

        let kcal = parse_f64(&cells[7], line, "kcal")?;
        let protein_g = parse_f64(&cells[8], line, "protein_g")?;
        let carbs_g = parse_f64(&cells[9], line, "carbs_g")?;
        let fat_g = parse_f64(&cells[10], line, "fat_g")?;
        let name = unguard(&cells[5]);
        let has_nutrients =
            kcal.is_some() || protein_g.is_some() || carbs_g.is_some() || fat_g.is_some();
        if name.is_empty() && has_nutrients {
            return Err(line_err(
                line,
                "food is required when nutrients are present",
            ));
        }
        if !name.is_empty() && kcal.is_none() {
            return Err(line_err(line, "kcal is required when food is present"));
        }
        if name.chars().count() > MAX_NAME {
            return Err(line_err(
                line,
                format!("food is longer than {MAX_NAME} characters"),
            ));
        }
        let portion = unguard(&cells[6]);
        if portion.chars().count() > MAX_PORTION {
            return Err(line_err(
                line,
                format!("portion is longer than {MAX_PORTION} characters"),
            ));
        }
        let description = unguard(&cells[4]);
        if description.chars().count() > 1000 {
            return Err(line_err(line, "description is longer than 1000 characters"));
        }
        rows.push(ParsedRow {
            line,
            date,
            meal_id: (!cells[1].is_empty()).then(|| cells[1].clone()),
            item_index: parse_index(&cells[2], line)?,
            meal_type: normalized_type(&cells[3], line)?,
            description,
            name,
            portion,
            kcal,
            protein_g,
            carbs_g,
            fat_g,
        });
    }
    Ok((rows, days))
}
