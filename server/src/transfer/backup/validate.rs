// SPDX-License-Identifier: AGPL-3.0-or-later
//! Validation of a parsed backup document, before anything is written.

use std::collections::HashSet;

use jiff::civil::Date;

use super::doc::*;
use crate::auth::validate_timezone;
use crate::error::{Error, Result};
use crate::nutrition::foods::normalize;
use crate::sync::{BODY_PARTS, ItemIn, MealIn, finite_in, one_of};
use crate::time::{now_ms, today_in};

/// Client clocks may be a little fast; the same slack the sync contract allows (PLAN.md §6).
const MAX_CLOCK_AHEAD_MS: i64 = 5 * 60 * 1000;
const MAX_SOURCE_CHARS: usize = 200;

pub(super) fn bad(msg: impl Into<String>) -> Error {
    Error::bad_request(msg)
}

pub(super) fn check(ok: bool, msg: impl Into<String>) -> Result<()> {
    ok.then_some(()).ok_or_else(|| bad(msg))
}

pub(super) fn doc_date(s: &str, what: &str) -> Result<Date> {
    s.parse()
        .map_err(|_| bad(format!("{what} is not a YYYY-MM-DD date")))
}

fn num(v: f64, lo: f64, hi: f64, name: &str) -> Result<()> {
    finite_in(v, lo, hi, name).map_err(bad)
}

fn text_ok(name: &str, s: &str, max: usize) -> Result<()> {
    check(
        (1..=max).contains(&s.chars().count()),
        format!("{name} must be 1-{max} characters"),
    )
}

fn max_chars(name: &str, s: Option<&str>, max: usize) -> Result<()> {
    check(
        s.is_none_or(|s| s.chars().count() <= max),
        format!("{name} is longer than {max} characters"),
    )
}

fn ts_ok(t: Option<Ts>, what: &str) -> Result<()> {
    check(
        t.is_none_or(|t| t.0 <= now_ms() + MAX_CLOCK_AHEAD_MS),
        format!("{what} is more than 5 minutes in the future"),
    )
}

fn source_ok(name: &str, s: Option<&str>) -> Result<()> {
    match s {
        Some(s) => one_of(s, &["manual", "shortcut", "import", "google"], name).map_err(bad),
        None => Ok(()),
    }
}

/// The sync-contract form of a backup meal. `food` maps each backup food id to the id to store.
pub(super) fn meal_in(m: &Meal, food: impl Fn(i64) -> Result<i64>) -> Result<MealIn> {
    let c = &m.core;
    Ok(MealIn {
        id: c.id.clone(),
        date: doc_date(&c.date, "meals[].date")?,
        meal_type: c.meal_type.clone(),
        description: c.description.clone(),
        scale: c.scale,
        assumptions: c.assumptions.clone(),
        confidence: c.confidence.clone(),
        source: c.source.clone(),
        provider_id: None,
        items: m
            .items
            .iter()
            .map(|i| {
                let i = &i.core;
                Ok(ItemIn {
                    id: i.id.clone(),
                    name: i.name.clone(),
                    portion_text: i.portion_text.clone(),
                    grams: i.grams,
                    kcal: i.kcal,
                    protein_g: i.protein_g,
                    carbs_g: i.carbs_g,
                    fat_g: i.fat_g,
                    food_id: i.food_id.map(&food).transpose()?,
                    origin: i.origin.clone(),
                })
            })
            .collect::<Result<_>>()?,
    })
}

pub(super) fn check_doc(doc: &Doc, user_id: i64) -> Result<()> {
    check(
        doc.format == FORMAT,
        format!("backup format must be {FORMAT:?}"),
    )?;
    check(
        doc.version == VERSION,
        format!("backup version must be {VERSION}"),
    )?;
    check(
        doc.exported_at.parse::<jiff::Timestamp>().is_ok(),
        "exported_at must be an RFC 3339 timestamp",
    )?;

    // ---- profile ----
    let p = &doc.profile;
    validate_timezone(&p.timezone)?;
    one_of(&p.weight_unit, &["kg", "lb"], "weight_unit").map_err(bad)?;
    one_of(&p.height_unit, &["cm", "ftin"], "height_unit").map_err(bad)?;
    let today = today_in(&p.timezone)?;
    let tomorrow = today
        .tomorrow()
        .map_err(|e| Error::Internal(e.to_string()))?;

    // ---- profile versions ----
    let mut seen_dates = HashSet::new();
    for (n, v) in doc.profile_versions.iter().enumerate() {
        let at = doc_date(&v.effective_from, "profile_versions[].effective_from")?;
        check(
            at <= tomorrow,
            format!("profile_versions[{n}].effective_from is in the future"),
        )?;
        check(
            seen_dates.insert(v.effective_from.clone()),
            "profile_versions has the same effective_from twice",
        )?;
        one_of(&v.sex, &["male", "female"], "sex").map_err(bad)?;
        check(
            (1900..=today.year() as i64).contains(&v.birth_year),
            "birth_year out of range",
        )?;
        num(v.height_cm, 50.0, 272.0, "height_cm")?;
        num(v.start_weight_kg, 20.0, 400.0, "start_weight_kg")?;
        if let Some(g) = v.goal_kg {
            num(g, 20.0, 400.0, "goal_kg")?;
        }
        num(v.pace_kg_per_week, 0.0, 1.0, "pace_kg_per_week")?;
        check(
            (0..=150_000).contains(&v.typical_steps),
            "typical_steps must be 0-150000",
        )?;
    }

    // ---- days ----
    let mut seen_days = HashSet::new();
    for (n, d) in doc.days.iter().enumerate() {
        let date = doc_date(&d.date, "days[].date")?;
        check(
            date <= tomorrow,
            format!("days[{n}] {} is in the future", d.date),
        )?;
        check(
            seen_days.insert(d.date.clone()),
            format!("days[{n}] duplicates date {}", d.date),
        )?;
        if let Some(s) = d.steps {
            check(
                (0..=150_000).contains(&s),
                format!("days[{n}].steps must be 0-150000"),
            )?;
        }
        if let Some(m) = d.gym_minutes {
            check(
                (0..=600).contains(&m),
                format!("days[{n}].gym_minutes must be 0-600"),
            )?;
        }
        if let Some(w) = d.weight_kg {
            num(w, 20.0, 400.0, "days[].weight_kg")?;
        }
        source_ok(
            &format!("days[{n}].steps_source"),
            d.steps_source.as_deref(),
        )?;
        source_ok(&format!("days[{n}].gym_source"), d.gym_source.as_deref())?;
        ts_ok(d.steps_updated_at, &format!("days[{n}].steps_updated_at"))?;
        ts_ok(d.gym_updated_at, &format!("days[{n}].gym_updated_at"))?;
        ts_ok(
            d.trained_updated_at,
            &format!("days[{n}].trained_updated_at"),
        )?;
        ts_ok(d.weight_updated_at, &format!("days[{n}].weight_updated_at"))?;
        let parts: Vec<String> = d
            .trained
            .as_deref()
            .and_then(|t| serde_json::from_str(t).ok())
            .unwrap_or_default();
        for part in parts {
            check(
                BODY_PARTS.contains(&part.as_str()),
                format!("days[{n}].trained has unknown body part {part:?}"),
            )?;
        }
    }

    // ---- foods ----
    let mut food_ids = HashSet::new();
    for (n, f) in doc.foods.iter().enumerate() {
        let f = &f.core;
        check(
            food_ids.insert(f.id),
            format!("foods[{n}] repeats id {}", f.id),
        )?;
        check(
            ["staple", "usda", "user", "off"].contains(&f.source.as_str()),
            format!("foods[{n}].source must be staple, usda, user or off"),
        )?;
        check(
            f.user_id.is_none() || f.user_id == Some(user_id),
            format!("foods[{n}] is private to another user"),
        )?;
        text_ok(&format!("foods[{n}].name"), f.name.trim(), 200)?;
        max_chars(&format!("foods[{n}].brand"), f.brand.as_deref(), 200)?;
        max_chars(
            &format!("foods[{n}].serving_text"),
            f.serving_text.as_deref(),
            100,
        )?;
        if f.source != "user" {
            check(
                f.source_ref.as_deref().is_some_and(|s| !s.is_empty()),
                format!("foods[{n}] needs a source_ref"),
            )?;
        }
        check(
            f.source_ref
                .as_deref()
                .is_none_or(|s| s.chars().count() <= MAX_SOURCE_CHARS),
            format!("foods[{n}].source_ref is too long"),
        )?;
        if let Some(b) = &f.barcode {
            // Existing Open Food Facts rows are not guaranteed to carry a valid check digit,
            // so a restore only requires the GTIN shape.
            check(
                (8..=14).contains(&b.len()) && b.bytes().all(|c| c.is_ascii_digit()),
                format!("foods[{n}].barcode must be 8-14 digits"),
            )?;
        }
        num(f.kcal, 0.0, 900.0, &format!("foods[{n}].kcal"))?;
        num(f.protein_g, 0.0, 100.0, &format!("foods[{n}].protein_g"))?;
        num(f.carbs_g, 0.0, 100.0, &format!("foods[{n}].carbs_g"))?;
        num(f.fat_g, 0.0, 100.0, &format!("foods[{n}].fat_g"))?;
        if let Some(g) = f.default_portion_g {
            num(
                g,
                f64::MIN_POSITIVE,
                2000.0,
                &format!("foods[{n}].default_portion_g"),
            )?;
        }
        if let Some(g) = f.serving_g {
            num(
                g,
                f64::MIN_POSITIVE,
                2000.0,
                &format!("foods[{n}].serving_g"),
            )?;
        }
        ts_ok(f.fetched_at, &format!("foods[{n}].fetched_at"))?;
        let mut units = HashSet::new();
        for (pn, p) in doc.foods[n].portions.iter().enumerate() {
            check(
                !p.unit.trim().is_empty() && p.unit.chars().count() <= 50,
                format!("foods[{n}].portions[{pn}].unit must be 1-50 characters"),
            )?;
            num(
                p.grams,
                f64::MIN_POSITIVE,
                10_000.0,
                &format!("foods[{n}].portions[{pn}].grams"),
            )?;
            check(
                units.insert(p.unit.clone()),
                format!("foods[{n}] repeats portion unit {:?}", p.unit),
            )?;
        }
    }

    // ---- aliases ----
    let mut alias_norms = HashSet::new();
    for (n, a) in doc.aliases.iter().enumerate() {
        check(
            food_ids.contains(&a.food_id),
            format!(
                "aliases[{n}] references food {} not in the backup",
                a.food_id
            ),
        )?;
        let alias = a.alias.trim();
        text_ok(&format!("aliases[{n}].alias"), alias, 200)?;
        check(
            alias_norms.insert(normalize(alias)),
            format!("aliases[{n}] repeats an earlier alias"),
        )?;
    }

    // ---- portions ----
    let mut phrases = HashSet::new();
    for (n, p) in doc.portions.iter().enumerate() {
        check(
            food_ids.contains(&p.food_id),
            format!(
                "portions[{n}] references food {} not in the backup",
                p.food_id
            ),
        )?;
        let phrase = p.phrase_norm.trim();
        text_ok(&format!("portions[{n}].phrase_norm"), phrase, 200)?;
        check(
            phrases.insert(phrase.to_string()),
            format!("portions[{n}] repeats a phrase"),
        )?;
        num(
            p.grams,
            f64::MIN_POSITIVE,
            10_000.0,
            &format!("portions[{n}].grams"),
        )?;
    }

    // ---- products ----
    let mut product_foods = HashSet::new();
    for (n, p) in doc.products.iter().enumerate() {
        check(
            food_ids.contains(&p.food_id),
            format!(
                "products[{n}] references food {} not in the backup",
                p.food_id
            ),
        )?;
        check(
            product_foods.insert(p.food_id),
            format!("products[{n}] repeats food {}", p.food_id),
        )?;
        if let Some(s) = p.last_servings {
            num(s, 0.0, 1000.0, &format!("products[{n}].last_servings"))?;
        }
        ts_ok(Some(p.last_used_at), &format!("products[{n}].last_used_at"))?;
    }

    // ---- meals and items ----
    let mut seen_meals = HashSet::new();
    let mut seen_items = HashSet::new();
    for (n, m) in doc.meals.iter().enumerate() {
        check(
            seen_meals.insert(m.core.id.as_str()),
            format!("meals[{n}] repeats id {:?}", m.core.id),
        )?;
        ts_ok(Some(m.core.updated_at), &format!("meals[{n}].updated_at"))?;
        ts_ok(Some(m.core.received_at), &format!("meals[{n}].received_at"))?;
        for (i, item) in m.items.iter().enumerate() {
            check(
                seen_items.insert(item.core.id.as_str()),
                format!("meals[{n}].items[{i}] repeats an item id"),
            )?;
            if let Some(fid) = item.core.food_id {
                check(
                    food_ids.contains(&fid),
                    format!("meals[{n}].items[{i}] references food {fid} not in the backup"),
                )?;
            }
        }
        meal_in(m, Ok)?.validate(tomorrow).map_err(bad)?;
    }
    Ok(())
}
