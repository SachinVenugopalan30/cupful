// SPDX-License-Identifier: AGPL-3.0-or-later
//! Groups parsed rows into meals: by explicit `meal_id`, or by legacy runs of equal date, meal and
//! description.

use std::collections::{BTreeSet, HashMap};

use super::super::parse::{Draft, DraftItem, ParsedRow, canonical_row, line_err, sha256_hex};
use crate::error::Result;

const MAX_ITEMS: usize = 100;

pub(super) fn group(rows: &[ParsedRow]) -> Result<Vec<Draft>> {
    // Mixed-id rule: applies to meal rows only.
    if let Some(first) = rows.first() {
        let by_id = first.meal_id.is_some();
        if let Some(bad) = rows.iter().find(|r| r.meal_id.is_some() != by_id) {
            return Err(line_err(
                bad.line,
                "meal_id must be present on every meal row or none",
            ));
        }
    }
    if rows.first().is_some_and(|r| r.meal_id.is_some()) {
        by_meal_id(rows)
    } else {
        legacy(rows)
    }
}

fn item(r: &ParsedRow, index: u32) -> DraftItem {
    DraftItem {
        index,
        name: r.name.clone(),
        portion: r.portion.clone(),
        kcal: r.kcal.unwrap_or(0.0),
        protein_g: r.protein_g.unwrap_or(0.0),
        carbs_g: r.carbs_g.unwrap_or(0.0),
        fat_g: r.fat_g.unwrap_or(0.0),
    }
}

/// One draft per distinct `meal_id`, in first-seen order.
fn by_meal_id(rows: &[ParsedRow]) -> Result<Vec<Draft>> {
    let mut order: Vec<String> = Vec::new();
    let mut groups: HashMap<String, Vec<usize>> = HashMap::new();
    for (i, r) in rows.iter().enumerate() {
        let id = r.meal_id.clone().expect("checked by the caller");
        match groups.get_mut(&id) {
            Some(idxs) => idxs.push(i),
            None => {
                order.push(id.clone());
                groups.insert(id, vec![i]);
            }
        }
    }

    let mut drafts = Vec::with_capacity(order.len());
    for raw_id in order {
        let idxs = &groups[&raw_id];
        let first = &rows[idxs[0]];
        let mut items = Vec::with_capacity(idxs.len());
        let mut seen = BTreeSet::new();
        for &i in idxs {
            let r = &rows[i];
            if r.date != first.date
                || r.meal_type != first.meal_type
                || r.description != first.description
            {
                return Err(line_err(
                    r.line,
                    format!("meal {raw_id:?} has an inconsistent date, meal or description"),
                ));
            }
            let index = r.item_index.ok_or_else(|| {
                line_err(r.line, "item_index is required when meal_id is present")
            })?;
            if !seen.insert(index) {
                return Err(line_err(r.line, format!("duplicate item_index {index}")));
            }
            if !r.name.is_empty() {
                items.push(item(r, index));
            }
        }
        items.sort_by_key(|i| i.index);
        if items.len() > MAX_ITEMS {
            return Err(line_err(
                first.line,
                format!("at most {MAX_ITEMS} items per meal"),
            ));
        }
        drafts.push(Draft {
            line: first.line,
            date: first.date,
            meal_type: first.meal_type.clone(),
            description: first.description.clone(),
            raw_id,
            legacy: false,
            legacy_hash: String::new(),
            legacy_occurrence: 0,
            items,
        });
    }
    Ok(drafts)
}

/// Legacy files have no ids: consecutive rows with the same date, meal and description form a meal.
fn legacy(rows: &[ParsedRow]) -> Result<Vec<Draft>> {
    let mut drafts = Vec::new();
    let mut start = 0;
    while start < rows.len() {
        let mut end = start + 1;
        while end < rows.len() {
            let a = &rows[end - 1];
            let b = &rows[end];
            let same =
                a.date == b.date && a.meal_type == b.meal_type && a.description == b.description;
            let increasing = match (a.item_index, b.item_index) {
                (Some(x), Some(y)) => y > x,
                _ => true,
            };
            if !(same && increasing) {
                break;
            }
            end += 1;
        }
        let group = &rows[start..end];
        let mut items = Vec::new();
        let mut used = BTreeSet::new();
        for (ordinal, r) in group.iter().enumerate() {
            if r.name.is_empty() {
                continue;
            }
            let mut index = r.item_index.unwrap_or(ordinal as u32);
            while used.contains(&index) {
                index += 1;
            }
            used.insert(index);
            items.push(item(r, index));
        }
        items.sort_by_key(|i| i.index);
        if items.len() > MAX_ITEMS {
            return Err(line_err(
                group[0].line,
                format!("at most {MAX_ITEMS} items per meal"),
            ));
        }
        let canonical = group
            .iter()
            .map(canonical_row)
            .collect::<Vec<_>>()
            .join("\n");
        drafts.push(Draft {
            line: group[0].line,
            date: group[0].date,
            meal_type: group[0].meal_type.clone(),
            description: group[0].description.clone(),
            raw_id: String::new(),
            legacy: true,
            legacy_hash: sha256_hex(&canonical),
            legacy_occurrence: 0,
            items,
        });
        start = end;
    }
    // Occurrence counts identical hashes in file order.
    let mut seen: HashMap<String, u32> = HashMap::new();
    for d in &mut drafts {
        let n = seen.entry(d.legacy_hash.clone()).or_insert(0);
        d.legacy_occurrence = *n;
        *n += 1;
    }
    Ok(drafts)
}
