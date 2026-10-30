// SPDX-License-Identifier: AGPL-3.0-or-later
//! `POST /api/import.csv`: Fuel Ledger (and Cupful's own) CSV into meals and day activity.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::OnceLock;

use axum::Json;
use axum::extract::State;
use jiff::civil::Date;
use serde_json::{Value, json};
use uuid::Uuid;

use super::HEADER;
use super::parse::*;
use crate::AppState;
use crate::auth::User;
use crate::error::{Error, Result, TextBody};
use crate::sync::{
    BODY_PARTS, DayPatch, ItemIn, MealIn, Source, WriteMode, date_ok, finite_in, patch_day_on,
    write_meal_on,
};
use crate::time::{now_ms, user_today};

const MAX_RECORDS: usize = 50_000;
const MAX_ITEMS: usize = 100;
const MAX_NAME: usize = 200;
const MAX_PORTION: usize = 200;

#[derive(sqlx::FromRow)]
struct StoredMeal {
    date: String,
    meal_type: String,
    description: String,
    scale: f64,
    assumptions: Option<String>,
    confidence: Option<String>,
    source: String,
    provider_id: Option<i64>,
    deleted_at: Option<i64>,
    edited: i64,
}

#[derive(sqlx::FromRow)]
struct StoredItem {
    id: String,
    name: String,
    portion_text: String,
    grams: Option<f64>,
    kcal: f64,
    protein_g: f64,
    carbs_g: f64,
    fat_g: f64,
    food_id: Option<i64>,
    origin: String,
}

#[derive(sqlx::FromRow)]
struct StoredDay {
    steps: Option<i64>,
    gym_minutes: Option<i64>,
    trained: Option<String>,
    weight_kg: Option<f64>,
}

fn import_ns() -> Uuid {
    static NS: OnceLock<Uuid> = OnceLock::new();
    *NS.get_or_init(|| Uuid::new_v5(&Uuid::NAMESPACE_OID, b"cupful fuel-ledger import v1"))
}

pub async fn import(State(s): State<AppState>, user: User, body: TextBody) -> Result<Json<Value>> {
    let today = user_today(&s.db, user.id).await?;
    let tomorrow = today
        .tomorrow()
        .map_err(|e| Error::Internal(e.to_string()))?;

    let text = body.0.strip_prefix('\u{feff}').unwrap_or(&body.0);
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
            if let Some((old, _)) = acc.steps
                && old != v
            {
                return Err(line_err(line, format!("conflicting steps for {ds}")));
            }
            acc.steps.get_or_insert((v, line));
        }
        if let Some(v) = parse_i64(&cells[12], line, "gym_minutes")? {
            finite_in(v as f64, 0.0, 600.0, "gym_minutes").map_err(|e| line_err(line, e))?;
            if let Some((old, _)) = acc.gym_minutes
                && old != v
            {
                return Err(line_err(line, format!("conflicting gym_minutes for {ds}")));
            }
            acc.gym_minutes.get_or_insert((v, line));
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
            if let Some((old, _)) = &acc.trained
                && *old != parts
            {
                return Err(line_err(line, format!("conflicting trained for {ds}")));
            }
            acc.trained.get_or_insert((parts, line));
        }
        if let Some(v) = parse_f64(&cells[14], line, "weight_kg")? {
            finite_in(v, 20.0, 400.0, "weight_kg").map_err(|e| line_err(line, e))?;
            if let Some((old, _)) = acc.weight_kg
                && old != v
            {
                return Err(line_err(line, format!("conflicting weight_kg for {ds}")));
            }
            acc.weight_kg.get_or_insert((v, line));
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
        let meal_id = (!cells[1].is_empty()).then(|| cells[1].clone());
        let item_index = parse_index(&cells[2], line)?;
        rows.push(ParsedRow {
            line,
            date,
            meal_id,
            item_index,
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

    // Mixed-id rule: applies to meal rows only.
    if let Some(first) = rows.first() {
        let mode = first.meal_id.is_some();
        if let Some(bad) = rows.iter().find(|r| r.meal_id.is_some() != mode) {
            return Err(line_err(
                bad.line,
                "meal_id must be present on every meal row or none",
            ));
        }
    }

    let mut drafts: Vec<Draft> = Vec::new();
    if rows.first().is_some_and(|r| r.meal_id.is_some()) {
        let mut order: Vec<String> = Vec::new();
        let mut groups: HashMap<String, Vec<usize>> = HashMap::new();
        for (i, r) in rows.iter().enumerate() {
            let id = r.meal_id.clone().expect("checked above");
            match groups.get(&id) {
                Some(_) => groups.get_mut(&id).unwrap().push(i),
                None => {
                    order.push(id.clone());
                    groups.insert(id, vec![i]);
                }
            }
        }
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
                    items.push(DraftItem {
                        index,
                        name: r.name.clone(),
                        portion: r.portion.clone(),
                        kcal: r.kcal.unwrap_or(0.0),
                        protein_g: r.protein_g.unwrap_or(0.0),
                        carbs_g: r.carbs_g.unwrap_or(0.0),
                        fat_g: r.fat_g.unwrap_or(0.0),
                    });
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
    } else {
        // Legacy: consecutive rows with the same date, meal and description.
        let mut start = 0;
        while start < rows.len() {
            let mut end = start + 1;
            while end < rows.len() {
                let a = &rows[end - 1];
                let b = &rows[end];
                let same = a.date == b.date
                    && a.meal_type == b.meal_type
                    && a.description == b.description;
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
                items.push(DraftItem {
                    index,
                    name: r.name.clone(),
                    portion: r.portion.clone(),
                    kcal: r.kcal.unwrap_or(0.0),
                    protein_g: r.protein_g.unwrap_or(0.0),
                    carbs_g: r.carbs_g.unwrap_or(0.0),
                    fat_g: r.fat_g.unwrap_or(0.0),
                });
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
    }

    let now = now_ms();
    let user_id = user.id;
    let mut tx = s.db.begin_with("BEGIN IMMEDIATE").await?;

    let (mut created, mut updated, mut unchanged) = (0i64, 0i64, 0i64);
    let (mut skipped_deleted, mut skipped_edited) = (0i64, 0i64);
    let mut item_count = 0i64;

    for d in &drafts {
        let uuid = if d.legacy {
            Uuid::new_v5(
                &import_ns(),
                format!("legacy:{user_id}:{}:{}", d.legacy_hash, d.legacy_occurrence).as_bytes(),
            )
        } else {
            match Uuid::parse_str(&d.raw_id) {
                Ok(u) => {
                    let owner = sqlx::query_scalar!(
                        "SELECT user_id FROM meals WHERE id = ?",
                        u.to_string()
                    )
                    .fetch_optional(&mut *tx)
                    .await?;
                    if owner == Some(user_id) {
                        u
                    } else {
                        Uuid::new_v5(&import_ns(), format!("{user_id}:{}", d.raw_id).as_bytes())
                    }
                }
                Err(_) => Uuid::new_v5(&import_ns(), format!("{user_id}:{}", d.raw_id).as_bytes()),
            }
        };
        let meal_id = uuid.to_string();

        let existing: Option<StoredMeal> = sqlx::query_as!(
            StoredMeal,
            "SELECT date, meal_type, description, scale, assumptions, confidence, source,
                    provider_id, deleted_at, edited
             FROM meals WHERE id = ? AND user_id = ?",
            &meal_id,
            user_id
        )
        .fetch_optional(&mut *tx)
        .await?;

        // PLAN §6: an upsert of a deleted id is stale, and import is never newer than the app.
        if existing.as_ref().is_some_and(|e| e.deleted_at.is_some()) {
            skipped_deleted += 1;
            continue;
        }

        let existing_items: Vec<StoredItem> = if existing.is_some() {
            sqlx::query_as!(
                StoredItem,
                "SELECT id, name, portion_text, grams, kcal, protein_g, carbs_g, fat_g,
                        food_id, origin
                 FROM meal_items WHERE meal_id = ? ORDER BY position",
                &meal_id
            )
            .fetch_all(&mut *tx)
            .await?
        } else {
            Vec::new()
        };

        let live = existing.as_ref().is_some_and(|e| e.deleted_at.is_none());
        let mut items = Vec::with_capacity(d.items.len());
        let (source, scale, assumptions, confidence, provider_id) = if live {
            let e = existing.as_ref().unwrap();
            (
                e.source.clone(),
                e.scale,
                e.assumptions.clone(),
                e.confidence.clone(),
                e.provider_id,
            )
        } else {
            ("import".to_string(), 1.0, None, None, None)
        };

        // Match by position, not the raw column: a 1-based Fuel Ledger file still maps onto the
        // item created at that ordinal by an earlier import.
        for (ordinal, it) in d.items.iter().enumerate() {
            let matched = if live {
                existing_items.get(ordinal)
            } else {
                None
            };
            match matched {
                Some(e) => items.push(ItemIn {
                    id: e.id.clone(),
                    name: it.name.clone(),
                    portion_text: it.portion.clone(),
                    grams: e.grams,
                    kcal: it.kcal,
                    protein_g: it.protein_g,
                    carbs_g: it.carbs_g,
                    fat_g: it.fat_g,
                    food_id: e.food_id,
                    origin: e.origin.clone(),
                }),
                None => items.push(ItemIn {
                    id: Uuid::new_v5(&uuid, it.index.to_string().as_bytes()).to_string(),
                    name: it.name.clone(),
                    portion_text: it.portion.clone(),
                    grams: None,
                    kcal: it.kcal,
                    protein_g: it.protein_g,
                    carbs_g: it.carbs_g,
                    fat_g: it.fat_g,
                    food_id: None,
                    origin: "manual".to_string(),
                }),
            }
        }

        let meal = MealIn {
            id: meal_id.clone(),
            date: d.date,
            meal_type: d.meal_type.clone(),
            description: d.description.clone(),
            scale,
            assumptions,
            confidence,
            source,
            provider_id,
            items,
        };
        meal.validate(tomorrow).map_err(|e| line_err(d.line, e))?;

        if live {
            let e = existing.as_ref().unwrap();
            let same_meal = e.date == d.date.to_string()
                && e.meal_type == meal.meal_type
                && e.description == meal.description
                && e.scale == meal.scale
                && e.assumptions == meal.assumptions
                && e.confidence == meal.confidence
                && e.source == meal.source
                && e.provider_id == meal.provider_id;
            let same_items = existing_items.len() == meal.items.len()
                && existing_items.iter().zip(&meal.items).all(|(a, b)| {
                    a.id == b.id
                        && a.name == b.name
                        && a.portion_text == b.portion_text
                        && a.grams == b.grams
                        && a.kcal == b.kcal
                        && a.protein_g == b.protein_g
                        && a.carbs_g == b.carbs_g
                        && a.fat_g == b.fat_g
                        && a.food_id == b.food_id
                        && a.origin == b.origin
                });
            if same_meal && same_items {
                unchanged += 1;
                item_count += meal.items.len() as i64;
                continue;
            }
            // Edited in Cupful since it was logged or imported: the app's version wins.
            if existing.as_ref().is_some_and(|e| e.edited != 0) {
                skipped_edited += 1;
                continue;
            }
        }

        match write_meal_on(&mut tx, user_id, &meal, now, now, WriteMode::Import).await? {
            crate::sync::Outcome::Applied => {
                if live {
                    updated += 1;
                } else {
                    created += 1;
                }
                item_count += meal.items.len() as i64;
            }
            crate::sync::Outcome::Rejected(e) => {
                return Err(Error::conflict(format!("line {}: {e}", d.line)));
            }
            crate::sync::Outcome::Duplicate | crate::sync::Outcome::Stale => {
                unchanged += 1;
            }
        }
    }

    let mut day_count = 0i64;
    for (date, acc) in &days {
        let date = date.parse::<Date>().expect("parsed above");
        let stored: Option<StoredDay> = sqlx::query_as!(
            StoredDay,
            "SELECT steps, gym_minutes, trained, weight_kg FROM days
                 WHERE user_id = ? AND date = ?",
            user_id,
            date.to_string()
        )
        .fetch_optional(&mut *tx)
        .await?;
        let (s_steps, s_gym, s_trained, s_weight) = stored
            .map(|d| (d.steps, d.gym_minutes, d.trained, d.weight_kg))
            .unwrap_or((None, None, None, None));
        let mut patch = DayPatch::default();

        if let Some((v, _)) = acc.steps
            && s_steps != Some(v)
        {
            patch.steps = Some(Some(v));
        }
        if let Some((v, _)) = acc.gym_minutes
            && s_gym != Some(v)
        {
            patch.gym_minutes = Some(Some(v));
        }
        if let Some((v, _)) = &acc.weight_kg
            && s_weight.is_none()
        {
            patch.weight_kg = Some(Some(*v));
        }
        if let Some((v, _)) = &acc.trained {
            let stored_parts: Vec<String> = s_trained
                .as_deref()
                .and_then(|t| serde_json::from_str(t).ok())
                .unwrap_or_default();
            if stored_parts != *v && (s_trained.is_none() || stored_parts.is_empty()) {
                patch.trained = Some(Some(v.clone()));
            }
        }

        let any = patch.steps.is_some()
            || patch.gym_minutes.is_some()
            || patch.trained.is_some()
            || patch.weight_kg.is_some();
        if any && patch_day_on(&mut tx, user_id, date, &patch, Source::Import, now).await? {
            day_count += 1;
        }
    }

    tx.commit().await?;

    Ok(Json(json!({
        "meals_created": created,
        "meals_updated": updated,
        "meals_unchanged": unchanged,
        "meals_skipped_deleted": skipped_deleted,
        "meals_skipped_edited": skipped_edited,
        "items": item_count,
        "days": day_count,
    })))
}
