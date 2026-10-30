// SPDX-License-Identifier: AGPL-3.0-or-later
//! JSON backup and restore (PLAN.md §14 M5).
//!
//! `GET /api/backup` writes one self-contained document for a user: profile, profile versions,
//! days, live meals with their items, every food those rows reference, aliases, portions and
//! saved products. Credentials, providers, sync bookkeeping and tombstones never leave.
//! `POST /api/restore` replaces exactly the same set of rows in one transaction. Shared food rows
//! (`user_id IS NULL`) are read-only: a missing one becomes a private copy owned by the target
//! instead of being written back into the shared database.

use std::collections::{HashMap, HashSet};

use axum::Json;
use axum::extract::{Request, State};
use axum::http::header::{CACHE_CONTROL, CONTENT_DISPOSITION, CONTENT_TYPE};
use axum::http::{HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use jiff::civil::Date;
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::SqliteConnection;

use crate::AppState;
use crate::auth::{User, validate_timezone};
use crate::error::{Error, Result};
use crate::nutrition::foods::normalize;
use crate::sync::{BODY_PARTS, ItemIn, MealIn, Outcome, WriteMode};
use crate::time::{now_ms, rfc3339, today_in, user_today};

const FORMAT: &str = "cupful-backup";
const VERSION: u32 = 1;
/// Client clocks may be a little fast; the same slack the sync contract allows (PLAN.md §6).
const MAX_CLOCK_AHEAD_MS: i64 = 5 * 60 * 1000;
const MAX_SOURCE_CHARS: usize = 200;

fn bad(msg: impl Into<String>) -> Error {
    Error::bad_request(msg)
}

fn check(ok: bool, msg: impl Into<String>) -> Result<()> {
    ok.then_some(()).ok_or_else(|| bad(msg))
}

fn field_ok(name: &str, v: f64, lo: f64, hi: f64) -> Result<()> {
    check(
        v.is_finite() && (lo..=hi).contains(&v),
        format!("{name} must be between {lo} and {hi}"),
    )
}

fn text_ok(name: &str, s: &str, max: usize) -> Result<()> {
    let n = s.chars().count();
    check(
        (1..=max).contains(&n),
        format!("{name} must be 1-{max} characters"),
    )
}

fn opt_text_ok(name: &str, s: Option<&str>, max: usize) -> Result<()> {
    if let Some(s) = s {
        check(
            s.chars().count() <= max,
            format!("{name} is longer than {max} characters"),
        )?;
    }
    Ok(())
}

fn doc_date(s: &str, what: &str) -> Result<Date> {
    s.parse()
        .map_err(|_| bad(format!("{what} is not a YYYY-MM-DD date")))
}

/// A timestamp in the backup: RFC 3339 (what export writes) or integer unix milliseconds.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum Ts {
    Text(String),
    Millis(i64),
}

fn ts_ms(t: &Ts, what: &str) -> Result<i64> {
    let ms = match t {
        Ts::Text(s) => s
            .parse::<jiff::Timestamp>()
            .map_err(|_| bad(format!("{what} must be an RFC 3339 timestamp")))?
            .as_millisecond(),
        Ts::Millis(ms) => *ms,
    };
    check(
        ms <= now_ms() + MAX_CLOCK_AHEAD_MS,
        format!("{what} is more than 5 minutes in the future"),
    )?;
    Ok(ms)
}

fn opt_ts_ms(t: &Option<Ts>, what: &str) -> Result<Option<i64>> {
    t.as_ref().map(|t| ts_ms(t, what)).transpose()
}

fn source_ok(name: &str, s: Option<&str>) -> Result<()> {
    if let Some(s) = s {
        check(
            ["manual", "shortcut", "import", "google"].contains(&s),
            format!("{name} must be manual, shortcut, import or google"),
        )?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// export
// ---------------------------------------------------------------------------------------------

#[derive(sqlx::FromRow)]
struct ProfileRow {
    timezone: String,
    weight_unit: String,
    height_unit: String,
}

#[derive(sqlx::FromRow)]
struct VersionRow {
    effective_from: String,
    sex: String,
    birth_year: i64,
    height_cm: f64,
    start_weight_kg: f64,
    goal_kg: Option<f64>,
    pace_kg_per_week: f64,
    typical_steps: i64,
}

#[derive(sqlx::FromRow)]
struct DayRow {
    date: String,
    steps: Option<i64>,
    steps_source: Option<String>,
    steps_updated_at: Option<i64>,
    gym_minutes: Option<i64>,
    gym_source: Option<String>,
    gym_updated_at: Option<i64>,
    trained: Option<String>,
    trained_updated_at: Option<i64>,
    weight_kg: Option<f64>,
    weight_updated_at: Option<i64>,
}

#[derive(sqlx::FromRow)]
struct MealRow {
    id: String,
    date: String,
    meal_type: String,
    description: String,
    scale: f64,
    assumptions: Option<String>,
    confidence: Option<String>,
    source: String,
    provider_id: Option<i64>,
    updated_at: i64,
    received_at: i64,
}

#[derive(sqlx::FromRow)]
struct ItemRow {
    meal_id: String,
    id: String,
    position: i64,
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
struct FoodRow {
    id: i64,
    source: String,
    source_ref: Option<String>,
    user_id: Option<i64>,
    name: String,
    name_norm: String,
    brand: Option<String>,
    barcode: Option<String>,
    is_prepared: i64,
    kcal: f64,
    protein_g: f64,
    carbs_g: f64,
    fat_g: f64,
    default_portion_g: Option<f64>,
    serving_g: Option<f64>,
    serving_text: Option<String>,
    fetched_at: Option<i64>,
}

#[derive(sqlx::FromRow)]
struct PortionRow {
    food_id: i64,
    unit: String,
    grams: f64,
}

#[derive(sqlx::FromRow)]
struct AliasRow {
    id: i64,
    food_id: i64,
    alias: String,
    alias_norm: String,
}

#[derive(sqlx::FromRow)]
struct UserPortionRow {
    phrase_norm: String,
    food_id: i64,
    grams: f64,
}

#[derive(sqlx::FromRow)]
struct ProductRow {
    food_id: i64,
    last_servings: Option<f64>,
    last_used_at: i64,
}

pub async fn export(s: AppState, user: User) -> Result<Response> {
    let db = &s.db;
    let id = user.id;
    let id_str = id.to_string();
    let today = user_today(db, id).await?;

    let profile = sqlx::query_as!(
        ProfileRow,
        "SELECT timezone, weight_unit, height_unit FROM profiles WHERE user_id = ?",
        id
    )
    .fetch_one(db)
    .await?;
    let versions = sqlx::query_as!(
        VersionRow,
        "SELECT effective_from, sex, birth_year, height_cm, start_weight_kg, goal_kg,
                pace_kg_per_week, typical_steps
         FROM profile_versions WHERE user_id = ? ORDER BY effective_from, id",
        id
    )
    .fetch_all(db)
    .await?;
    let days = sqlx::query_as!(
        DayRow,
        "SELECT date, steps, steps_source, steps_updated_at, gym_minutes, gym_source,
                gym_updated_at, trained, trained_updated_at, weight_kg, weight_updated_at
         FROM days WHERE user_id = ? ORDER BY date",
        id
    )
    .fetch_all(db)
    .await?;
    let meals = sqlx::query_as!(
        MealRow,
        "SELECT id, date, meal_type, description, scale, assumptions, confidence, source,
                provider_id, updated_at, received_at
         FROM meals WHERE user_id = ? AND deleted_at IS NULL ORDER BY date, id",
        id
    )
    .fetch_all(db)
    .await?;
    let items = sqlx::query_as!(
        ItemRow,
        "SELECT i.meal_id, i.id, i.position, i.name, i.portion_text, i.grams, i.kcal,
                i.protein_g, i.carbs_g, i.fat_g, i.food_id, i.origin
         FROM meal_items i JOIN meals m ON m.id = i.meal_id
         WHERE m.user_id = ? AND m.deleted_at IS NULL
         ORDER BY i.meal_id, i.position, i.id",
        id
    )
    .fetch_all(db)
    .await?;
    // Food ids visible to the user that this backup must carry: private foods, their own label
    // entries, and everything referenced by a live meal item, alias, portion or product.
    // Another user's private food can never match.
    let foods = sqlx::query_as!(
        FoodRow,
        "SELECT f.id, f.source, f.source_ref, f.user_id, f.name, f.name_norm, f.brand, f.barcode,
               f.is_prepared, f.kcal, f.protein_g, f.carbs_g, f.fat_g, f.default_portion_g,
               f.serving_g, f.serving_text, f.fetched_at
        FROM foods f
        WHERE (f.user_id IS NULL OR f.user_id = ?)
          AND (
            f.user_id = ?
            OR (f.source = 'user' AND f.user_id IS NULL AND f.source_ref = ?)
            OR f.id IN (
                SELECT i.food_id FROM meal_items i JOIN meals m ON m.id = i.meal_id
                WHERE m.user_id = ? AND m.deleted_at IS NULL AND i.food_id IS NOT NULL
                UNION SELECT food_id FROM food_aliases WHERE user_id = ?
                UNION SELECT food_id FROM user_portions WHERE user_id = ?
                UNION SELECT food_id FROM user_products WHERE user_id = ?
            )
          )
        ORDER BY f.id",
        id,
        id,
        &id_str,
        id,
        id,
        id,
        id
    )
    .fetch_all(db)
    .await?;
    let mut portions_by_food: HashMap<i64, Vec<PortionRow>> = HashMap::new();
    if !foods.is_empty() {
        let portions = sqlx::query_as!(
            PortionRow,
            "SELECT p.food_id, p.unit, p.grams FROM food_portions p
             WHERE p.food_id IN (
                 SELECT f.id FROM foods f
                 WHERE (f.user_id IS NULL OR f.user_id = ?)
                   AND (
                     f.user_id = ?
                     OR (f.source = 'user' AND f.user_id IS NULL AND f.source_ref = ?)
                     OR f.id IN (
                         SELECT i.food_id FROM meal_items i JOIN meals m ON m.id = i.meal_id
                         WHERE m.user_id = ? AND m.deleted_at IS NULL AND i.food_id IS NOT NULL
                         UNION SELECT food_id FROM food_aliases WHERE user_id = ?
                         UNION SELECT food_id FROM user_portions WHERE user_id = ?
                         UNION SELECT food_id FROM user_products WHERE user_id = ?
                     )
                   )
             )
             ORDER BY p.food_id, p.unit",
            id,
            id,
            &id_str,
            id,
            id,
            id,
            id
        )
        .fetch_all(db)
        .await?;
        for p in portions {
            portions_by_food.entry(p.food_id).or_default().push(p);
        }
    }
    let aliases = sqlx::query_as!(
        AliasRow,
        "SELECT id, food_id, alias, alias_norm FROM food_aliases
         WHERE user_id = ? ORDER BY id",
        id
    )
    .fetch_all(db)
    .await?;
    let user_portions = sqlx::query_as!(
        UserPortionRow,
        "SELECT phrase_norm, food_id, grams FROM user_portions
         WHERE user_id = ? ORDER BY phrase_norm",
        id
    )
    .fetch_all(db)
    .await?;
    let products = sqlx::query_as!(
        ProductRow,
        "SELECT food_id, last_servings, last_used_at FROM user_products
         WHERE user_id = ? ORDER BY food_id",
        id
    )
    .fetch_all(db)
    .await?;
    let username = sqlx::query_scalar!("SELECT username FROM users WHERE id = ?", id)
        .fetch_one(db)
        .await?;

    let food_meta: HashMap<i64, (String, Option<String>)> = foods
        .iter()
        .map(|f| (f.id, (f.source.clone(), f.source_ref.clone())))
        .collect();

    let mut items_by_meal: HashMap<&str, Vec<&ItemRow>> = HashMap::new();
    for item in &items {
        items_by_meal
            .entry(item.meal_id.as_str())
            .or_default()
            .push(item);
    }

    let food_values: Vec<Value> = foods
        .iter()
        .map(|f| {
            let portions: Vec<Value> = portions_by_food
                .get(&f.id)
                .map(|ps| {
                    ps.iter()
                        .map(|p| json!({"unit": p.unit, "grams": p.grams}))
                        .collect()
                })
                .unwrap_or_default();
            let fetched_at = f.fetched_at.map(rfc3339).transpose()?;
            Ok(json!({
                "id": f.id,
                "source": f.source,
                "source_ref": f.source_ref,
                "user_id": f.user_id,
                "name": f.name,
                "name_norm": f.name_norm,
                "brand": f.brand,
                "barcode": f.barcode,
                "is_prepared": f.is_prepared != 0,
                "kcal": f.kcal,
                "protein_g": f.protein_g,
                "carbs_g": f.carbs_g,
                "fat_g": f.fat_g,
                "default_portion_g": f.default_portion_g,
                "serving_g": f.serving_g,
                "serving_text": f.serving_text,
                "fetched_at": fetched_at,
                "portions": portions,
            }))
        })
        .collect::<Result<_>>()?;

    let version_values: Vec<Value> = versions
        .iter()
        .map(|v| {
            json!({
                "effective_from": v.effective_from,
                "sex": v.sex,
                "birth_year": v.birth_year,
                "height_cm": v.height_cm,
                "start_weight_kg": v.start_weight_kg,
                "goal_kg": v.goal_kg,
                "pace_kg_per_week": v.pace_kg_per_week,
                "typical_steps": v.typical_steps,
            })
        })
        .collect();

    let day_values: Vec<Value> = days
        .iter()
        .map(|d| {
            Ok(json!({
                "date": d.date,
                "steps": d.steps,
                "steps_source": d.steps_source,
                "steps_updated_at": d.steps_updated_at.map(rfc3339).transpose()?,
                "gym_minutes": d.gym_minutes,
                "gym_source": d.gym_source,
                "gym_updated_at": d.gym_updated_at.map(rfc3339).transpose()?,
                "trained": d.trained.as_deref()
                    .and_then(|t| serde_json::from_str::<Vec<String>>(t).ok())
                    .unwrap_or_default(),
                "trained_updated_at": d.trained_updated_at.map(rfc3339).transpose()?,
                "weight_kg": d.weight_kg,
                "weight_updated_at": d.weight_updated_at.map(rfc3339).transpose()?,
            }))
        })
        .collect::<Result<_>>()?;

    let meal_values: Vec<Value> = meals
        .iter()
        .map(|m| {
            let its: Vec<Value> = items_by_meal
                .get(m.id.as_str())
                .map(Vec::as_slice)
                .unwrap_or_default()
                .iter()
                .map(|i| {
                    let food = i
                        .food_id
                        .and_then(|fid| food_meta.get(&fid))
                        .map(|(s, r)| json!({"source": s, "source_ref": r}));
                    json!({
                        "id": i.id,
                        "position": i.position,
                        "name": i.name,
                        "portion_text": i.portion_text,
                        "grams": i.grams,
                        "kcal": i.kcal,
                        "protein_g": i.protein_g,
                        "carbs_g": i.carbs_g,
                        "fat_g": i.fat_g,
                        "food_id": i.food_id,
                        "food": food,
                        "origin": i.origin,
                    })
                })
                .collect();
            Ok(json!({
                "id": m.id,
                "date": m.date,
                "meal_type": m.meal_type,
                "description": m.description,
                "scale": m.scale,
                "assumptions": m.assumptions,
                "confidence": m.confidence,
                "source": m.source,
                "provider_id": m.provider_id,
                "updated_at": rfc3339(m.updated_at)?,
                "received_at": rfc3339(m.received_at)?,
                "items": its,
            }))
        })
        .collect::<Result<_>>()?;

    let alias_values: Vec<Value> = aliases
        .iter()
        .map(|a| {
            json!({"id": a.id, "food_id": a.food_id, "alias": a.alias, "alias_norm": a.alias_norm})
        })
        .collect();
    let portion_values: Vec<Value> = user_portions
        .iter()
        .map(|p| json!({"phrase_norm": p.phrase_norm, "food_id": p.food_id, "grams": p.grams}))
        .collect();
    let product_values: Vec<Value> = products
        .iter()
        .map(|p| {
            Ok(json!({
                "food_id": p.food_id,
                "last_servings": p.last_servings,
                "last_used_at": rfc3339(p.last_used_at)?,
            }))
        })
        .collect::<Result<_>>()?;

    let doc = json!({
        "format": FORMAT,
        "version": VERSION,
        "exported_at": rfc3339(now_ms())?,
        "username": username,
        "profile": {
            "timezone": profile.timezone,
            "weight_unit": profile.weight_unit,
            "height_unit": profile.height_unit,
        },
        "profile_versions": version_values,
        "days": day_values,
        "meals": meal_values,
        "foods": food_values,
        "aliases": alias_values,
        "portions": portion_values,
        "products": product_values,
    });

    let mut res = Json(doc).into_response();
    let headers = res.headers_mut();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    headers.insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    headers.insert(
        CONTENT_DISPOSITION,
        HeaderValue::from_str(&format!(
            "attachment; filename=\"cupful-backup-{today}.json\""
        ))
        .map_err(|e| Error::Internal(e.to_string()))?,
    );
    Ok(res)
}

// ---------------------------------------------------------------------------------------------
// restore
// ---------------------------------------------------------------------------------------------

#[derive(Deserialize)]
struct Envelope {
    confirm: bool,
    backup: BackupDoc,
}

/// A backup is a whole account; anything bigger than this is not one.
const MAX_BODY: usize = 10 << 20;

#[derive(Deserialize)]
struct BackupDoc {
    format: String,
    version: u32,
    exported_at: String,
    #[serde(default)]
    username: Option<String>,
    profile: ProfileDoc,
    profile_versions: Vec<VersionDoc>,
    days: Vec<DayDoc>,
    meals: Vec<MealDoc>,
    foods: Vec<FoodDoc>,
    aliases: Vec<AliasDoc>,
    portions: Vec<PortionDoc>,
    products: Vec<ProductDoc>,
}

#[derive(Deserialize)]
struct ProfileDoc {
    timezone: String,
    weight_unit: String,
    height_unit: String,
}

#[derive(Deserialize)]
struct VersionDoc {
    effective_from: String,
    sex: String,
    birth_year: i64,
    height_cm: f64,
    start_weight_kg: f64,
    goal_kg: Option<f64>,
    pace_kg_per_week: f64,
    typical_steps: i64,
}

#[derive(Deserialize)]
struct DayDoc {
    date: String,
    steps: Option<i64>,
    steps_source: Option<String>,
    steps_updated_at: Option<Ts>,
    gym_minutes: Option<i64>,
    gym_source: Option<String>,
    gym_updated_at: Option<Ts>,
    trained: Option<Vec<String>>,
    trained_updated_at: Option<Ts>,
    weight_kg: Option<f64>,
    weight_updated_at: Option<Ts>,
}

#[derive(Deserialize)]
struct MealDoc {
    id: String,
    date: String,
    meal_type: String,
    description: String,
    scale: f64,
    assumptions: Option<String>,
    confidence: Option<String>,
    source: String,
    updated_at: Ts,
    #[serde(default)]
    received_at: Option<Ts>,
    items: Vec<ItemDoc>,
}

#[derive(Deserialize)]
struct ItemDoc {
    id: String,
    name: String,
    #[serde(default)]
    portion_text: Option<String>,
    grams: Option<f64>,
    kcal: f64,
    protein_g: f64,
    carbs_g: f64,
    fat_g: f64,
    food_id: Option<i64>,
    origin: String,
    #[serde(default)]
    food: Option<Value>,
}

#[derive(Deserialize)]
struct FoodDoc {
    id: i64,
    source: String,
    source_ref: Option<String>,
    user_id: Option<i64>,
    name: String,
    brand: Option<String>,
    barcode: Option<String>,
    #[serde(default)]
    is_prepared: bool,
    kcal: f64,
    protein_g: f64,
    carbs_g: f64,
    fat_g: f64,
    default_portion_g: Option<f64>,
    serving_g: Option<f64>,
    serving_text: Option<String>,
    fetched_at: Option<Ts>,
    #[serde(default)]
    portions: Vec<FoodPortionDoc>,
}

#[derive(Deserialize)]
struct FoodPortionDoc {
    unit: String,
    grams: f64,
}

#[derive(Deserialize)]
struct AliasDoc {
    food_id: i64,
    alias: String,
    #[serde(default)]
    alias_norm: Option<String>,
}

#[derive(Deserialize)]
struct PortionDoc {
    phrase_norm: String,
    food_id: i64,
    grams: f64,
}

#[derive(Deserialize)]
struct ProductDoc {
    food_id: i64,
    last_servings: Option<f64>,
    last_used_at: Ts,
}

/// Everything parsed and validated, ready to be written.
struct Checked {
    doc: BackupDoc,
    /// Original meal/item ids, for the ownership check inside the transaction.
    meal_ids: Vec<String>,
    item_ids: Vec<String>,
    /// Backup food ids, so every item/alias/portion/product reference can be checked.
    food_ids: HashSet<i64>,
}

fn check_doc(doc: BackupDoc, user_id: i64) -> Result<Checked> {
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
    let _ = &doc.username; // informational only

    // ---- profile ----
    let p = &doc.profile;
    validate_timezone(&p.timezone)?;
    check(
        ["kg", "lb"].contains(&p.weight_unit.as_str()),
        "weight_unit must be kg or lb",
    )?;
    check(
        ["cm", "ftin"].contains(&p.height_unit.as_str()),
        "height_unit must be cm or ftin",
    )?;
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
        check(
            ["male", "female"].contains(&v.sex.as_str()),
            "sex must be male or female",
        )?;
        check(
            (1900..=today.year() as i64).contains(&v.birth_year),
            "birth_year out of range",
        )?;
        field_ok("height_cm", v.height_cm, 50.0, 272.0)?;
        field_ok("start_weight_kg", v.start_weight_kg, 20.0, 400.0)?;
        if let Some(g) = v.goal_kg {
            field_ok("goal_kg", g, 20.0, 400.0)?;
        }
        field_ok("pace_kg_per_week", v.pace_kg_per_week, 0.0, 1.0)?;
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
            field_ok("days[].weight_kg", w, 20.0, 400.0)?;
        }
        source_ok(
            &format!("days[{n}].steps_source"),
            d.steps_source.as_deref(),
        )?;
        source_ok(&format!("days[{n}].gym_source"), d.gym_source.as_deref())?;
        opt_ts_ms(&d.steps_updated_at, &format!("days[{n}].steps_updated_at"))?;
        opt_ts_ms(&d.gym_updated_at, &format!("days[{n}].gym_updated_at"))?;
        opt_ts_ms(
            &d.trained_updated_at,
            &format!("days[{n}].trained_updated_at"),
        )?;
        opt_ts_ms(
            &d.weight_updated_at,
            &format!("days[{n}].weight_updated_at"),
        )?;
        if let Some(t) = &d.trained {
            for part in t {
                check(
                    BODY_PARTS.contains(&part.as_str()),
                    format!("days[{n}].trained has unknown body part {part:?}"),
                )?;
            }
        }
    }

    // ---- foods ----
    let mut food_ids = HashSet::new();
    for (n, f) in doc.foods.iter().enumerate() {
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
        let name = f.name.trim();
        text_ok(&format!("foods[{n}].name"), name, 200)?;
        opt_text_ok(&format!("foods[{n}].brand"), f.brand.as_deref(), 200)?;
        opt_text_ok(
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
        if let Some(sr) = &f.source_ref {
            check(
                sr.chars().count() <= MAX_SOURCE_CHARS,
                format!("foods[{n}].source_ref is too long"),
            )?;
        }
        if let Some(b) = &f.barcode {
            // Existing Open Food Facts rows are not guaranteed to carry a valid check digit,
            // so a restore only requires the GTIN shape.
            check(
                (8..=14).contains(&b.len()) && b.bytes().all(|c| c.is_ascii_digit()),
                format!("foods[{n}].barcode must be 8-14 digits"),
            )?;
        }
        field_ok(&format!("foods[{n}].kcal"), f.kcal, 0.0, 900.0)?;
        field_ok(&format!("foods[{n}].protein_g"), f.protein_g, 0.0, 100.0)?;
        field_ok(&format!("foods[{n}].carbs_g"), f.carbs_g, 0.0, 100.0)?;
        field_ok(&format!("foods[{n}].fat_g"), f.fat_g, 0.0, 100.0)?;
        if let Some(g) = f.default_portion_g {
            field_ok(
                &format!("foods[{n}].default_portion_g"),
                g,
                f64::MIN_POSITIVE,
                2000.0,
            )?;
        }
        if let Some(g) = f.serving_g {
            field_ok(
                &format!("foods[{n}].serving_g"),
                g,
                f64::MIN_POSITIVE,
                2000.0,
            )?;
        }
        opt_ts_ms(&f.fetched_at, &format!("foods[{n}].fetched_at"))?;
        let mut units = HashSet::new();
        for (pn, p) in f.portions.iter().enumerate() {
            check(
                !p.unit.trim().is_empty() && p.unit.chars().count() <= 50,
                format!("foods[{n}].portions[{pn}].unit must be 1-50 characters"),
            )?;
            field_ok(
                &format!("foods[{n}].portions[{pn}].grams"),
                p.grams,
                f64::MIN_POSITIVE,
                10_000.0,
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
        let _ = &a.alias_norm;
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
        field_ok(
            &format!("portions[{n}].grams"),
            p.grams,
            f64::MIN_POSITIVE,
            10_000.0,
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
            field_ok(&format!("products[{n}].last_servings"), s, 0.0, 1000.0)?;
        }
        ts_ms(&p.last_used_at, &format!("products[{n}].last_used_at"))?;
    }

    // ---- meals and items ----
    let mut meal_ids = Vec::with_capacity(doc.meals.len());
    let mut seen_meals = HashSet::new();
    let mut item_ids = Vec::new();
    let mut seen_items = HashSet::new();
    for (n, m) in doc.meals.iter().enumerate() {
        check(
            seen_meals.insert(m.id.clone()),
            format!("meals[{n}] repeats id {:?}", m.id),
        )?;
        ts_ms(&m.updated_at, &format!("meals[{n}].updated_at"))?;
        if let Some(r) = &m.received_at {
            ts_ms(r, &format!("meals[{n}].received_at"))?;
        }
        let mut meal = MealIn {
            id: m.id.clone(),
            date: doc_date(&m.date, "meals[].date")?,
            meal_type: m.meal_type.clone(),
            description: m.description.clone(),
            scale: m.scale,
            assumptions: m.assumptions.clone(),
            confidence: m.confidence.clone(),
            source: m.source.clone(),
            provider_id: None,
            items: Vec::with_capacity(m.items.len()),
        };
        for (in_, item) in m.items.iter().enumerate() {
            check(
                seen_items.insert(item.id.clone()),
                format!("meals[{n}].items[{in_}] repeats an item id"),
            )?;
            if let Some(fid) = item.food_id {
                check(
                    food_ids.contains(&fid),
                    format!("meals[{n}].items[{in_}] references food {fid} not in the backup"),
                )?;
            }
            let _ = &item.food;
            meal.items.push(ItemIn {
                id: item.id.clone(),
                name: item.name.clone(),
                portion_text: item.portion_text.clone().unwrap_or_default(),
                grams: item.grams,
                kcal: item.kcal,
                protein_g: item.protein_g,
                carbs_g: item.carbs_g,
                fat_g: item.fat_g,
                food_id: item.food_id,
                origin: item.origin.clone(),
            });
            item_ids.push(item.id.clone());
        }
        meal.validate(tomorrow).map_err(bad)?;
        meal_ids.push(m.id.clone());
    }

    Ok(Checked {
        doc,
        meal_ids,
        item_ids,
        food_ids,
    })
}

async fn insert_private_food(
    conn: &mut SqliteConnection,
    user_id: i64,
    f: &FoodDoc,
) -> Result<i64> {
    let name = f.name.trim().to_string();
    let norm = normalize(&name);
    let existing = sqlx::query_scalar!(
        "SELECT id FROM foods WHERE user_id = ? AND name_norm = ? AND barcode IS ?
           AND kcal = ? AND protein_g = ? AND carbs_g = ? AND fat_g = ? LIMIT 1",
        user_id,
        &norm,
        &f.barcode,
        f.kcal,
        f.protein_g,
        f.carbs_g,
        f.fat_g
    )
    .fetch_optional(&mut *conn)
    .await?;
    let id = match existing {
        Some(id) => id,
        None => {
            sqlx::query_scalar!(
                "INSERT INTO foods (source, source_ref, user_id, name, name_norm, brand, barcode,
                     is_prepared, kcal, protein_g, carbs_g, fat_g, default_portion_g, serving_g,
                     serving_text, fetched_at)
                 VALUES ('user', ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                 RETURNING id",
                user_id.to_string(),
                user_id,
                &name,
                &norm,
                &f.brand,
                &f.barcode,
                i64::from(f.is_prepared),
                f.kcal,
                f.protein_g,
                f.carbs_g,
                f.fat_g,
                f.default_portion_g,
                f.serving_g,
                &f.serving_text,
                f.fetched_at.as_ref().map(|t| {
                    match t {
                        Ts::Millis(ms) => *ms,
                        Ts::Text(s) => s
                            .parse::<jiff::Timestamp>()
                            .expect("validated timestamp")
                            .as_millisecond(),
                    }
                })
            )
            .fetch_one(&mut *conn)
            .await?
        }
    };
    for p in &f.portions {
        sqlx::query!(
            "INSERT INTO food_portions (food_id, unit, grams) VALUES (?, ?, ?)
             ON CONFLICT (food_id, unit) DO UPDATE SET grams = excluded.grams",
            id,
            &p.unit,
            p.grams
        )
        .execute(&mut *conn)
        .await?;
    }
    Ok(id)
}

/// Resolve one backup food to a live `foods.id`, reading shared rows and writing private ones.
async fn resolve_food(conn: &mut SqliteConnection, user_id: i64, f: &FoodDoc) -> Result<i64> {
    let shared = if f.source == "user" && f.user_id.is_none() {
        // A label entry: the owner's shared row for this barcode, if it still exists.
        match &f.barcode {
            Some(b) => {
                sqlx::query_scalar!(
                    "SELECT id FROM foods
                     WHERE barcode = ? AND source = 'user' AND source_ref = ? AND user_id IS NULL
                     LIMIT 1",
                    b,
                    user_id.to_string()
                )
                .fetch_optional(&mut *conn)
                .await?
            }
            None => None,
        }
    } else if f.source == "user" {
        None
    } else {
        match &f.source_ref {
            Some(sr) => {
                sqlx::query_scalar!(
                    "SELECT id FROM foods
                     WHERE source = ? AND source_ref = ? AND user_id IS NULL LIMIT 1",
                    &f.source,
                    sr
                )
                .fetch_optional(&mut *conn)
                .await?
            }
            None => None,
        }
    };
    match shared {
        Some(id) => Ok(id),
        None => insert_private_food(conn, user_id, f).await,
    }
}

pub async fn restore(State(s): State<AppState>, user: User, req: Request) -> Result<Json<Value>> {
    // Parsed straight into the typed document: no intermediate `Value` copy.
    let bytes = axum::body::to_bytes(req.into_body(), MAX_BODY)
        .await
        .map_err(|_| Error::Status(StatusCode::PAYLOAD_TOO_LARGE, "backup is over 10 MB".into()))?;
    let env: Envelope =
        serde_json::from_slice(&bytes).map_err(|e| bad(format!("invalid backup request: {e}")))?;
    drop(bytes);
    check(env.confirm, "confirm must be true")?;
    let doc = env.backup;
    let Checked {
        doc,
        meal_ids,
        item_ids,
        food_ids,
    } = check_doc(doc, user.id)?;

    let now = now_ms();
    let counts = Counts {
        profile_versions: doc.profile_versions.len(),
        days: doc.days.len(),
        meals: doc.meals.len(),
        items: item_ids.len(),
        foods: doc.foods.len(),
        aliases: doc.aliases.len(),
        portions: doc.portions.len(),
        products: doc.products.len(),
    };

    let mut tx = s.db.begin_with("BEGIN IMMEDIATE").await?;

    // Ids owned by another account are a conflict, never an overwrite. Checked before any delete.
    for id in &meal_ids {
        let owner = sqlx::query_scalar!("SELECT user_id FROM meals WHERE id = ?", id)
            .fetch_optional(&mut *tx)
            .await?;
        if owner.is_some_and(|o| o != user.id) {
            return Err(Error::conflict(
                "a meal id in the backup belongs to another user",
            ));
        }
    }
    for id in &item_ids {
        let owner = sqlx::query_scalar!(
            "SELECT m.user_id FROM meal_items i JOIN meals m ON m.id = i.meal_id WHERE i.id = ?",
            id
        )
        .fetch_optional(&mut *tx)
        .await?;
        if owner.is_some_and(|o| o != user.id) {
            return Err(Error::conflict(
                "an item id in the backup belongs to another user",
            ));
        }
    }

    // Full replacement of this user's logged data and private foods only.
    // Tombstones and `sync_ops` stay: a device replaying its offline queue after a restore must
    // not resurrect deleted meals, and old op ids stay duplicates. A backed-up meal whose id is
    // tombstoned here wins (restore is "go back to the snapshot") and is revived below.
    sqlx::query!(
        "DELETE FROM meals WHERE user_id = ? AND deleted_at IS NULL",
        user.id
    )
    .execute(&mut *tx)
    .await?;
    sqlx::query!("DELETE FROM days WHERE user_id = ?", user.id)
        .execute(&mut *tx)
        .await?;
    sqlx::query!("DELETE FROM food_aliases WHERE user_id = ?", user.id)
        .execute(&mut *tx)
        .await?;
    sqlx::query!("DELETE FROM user_portions WHERE user_id = ?", user.id)
        .execute(&mut *tx)
        .await?;
    sqlx::query!("DELETE FROM user_products WHERE user_id = ?", user.id)
        .execute(&mut *tx)
        .await?;
    sqlx::query!("DELETE FROM profile_versions WHERE user_id = ?", user.id)
        .execute(&mut *tx)
        .await?;
    sqlx::query!("DELETE FROM foods WHERE user_id = ?", user.id)
        .execute(&mut *tx)
        .await?;

    sqlx::query!(
        "UPDATE profiles SET timezone = ?, weight_unit = ?, height_unit = ? WHERE user_id = ?",
        &doc.profile.timezone,
        &doc.profile.weight_unit,
        &doc.profile.height_unit,
        user.id
    )
    .execute(&mut *tx)
    .await?;

    for v in &doc.profile_versions {
        sqlx::query!(
            "INSERT INTO profile_versions (user_id, effective_from, sex, birth_year, height_cm,
                 start_weight_kg, goal_kg, pace_kg_per_week, typical_steps)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            user.id,
            &v.effective_from,
            &v.sex,
            v.birth_year,
            v.height_cm,
            v.start_weight_kg,
            v.goal_kg,
            v.pace_kg_per_week,
            v.typical_steps
        )
        .execute(&mut *tx)
        .await?;
    }

    // Shared rows are read only. A missing one becomes a private copy for this user.
    let mut food_map: HashMap<i64, i64> = HashMap::with_capacity(doc.foods.len());
    for f in &doc.foods {
        let id = resolve_food(&mut tx, user.id, f).await?;
        food_map.insert(f.id, id);
    }
    debug_assert_eq!(food_map.len(), food_ids.len());

    for d in &doc.days {
        let trained = d
            .trained
            .as_ref()
            .map(|t| serde_json::to_string(t).expect("strings serialize"));
        sqlx::query!(
            "INSERT INTO days (user_id, date, steps, steps_source, steps_updated_at, gym_minutes,
                 gym_source, gym_updated_at, trained, trained_updated_at, weight_kg,
                 weight_updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            user.id,
            &d.date,
            d.steps,
            &d.steps_source,
            opt_ts_ms(&d.steps_updated_at, "steps_updated_at")?,
            d.gym_minutes,
            &d.gym_source,
            opt_ts_ms(&d.gym_updated_at, "gym_updated_at")?,
            &trained,
            opt_ts_ms(&d.trained_updated_at, "trained_updated_at")?,
            d.weight_kg,
            opt_ts_ms(&d.weight_updated_at, "weight_updated_at")?
        )
        .execute(&mut *tx)
        .await?;
    }

    for m in &doc.meals {
        let items = m
            .items
            .iter()
            .map(|i| {
                Ok(ItemIn {
                    id: i.id.clone(),
                    name: i.name.clone(),
                    portion_text: i.portion_text.clone().unwrap_or_default(),
                    grams: i.grams,
                    kcal: i.kcal,
                    protein_g: i.protein_g,
                    carbs_g: i.carbs_g,
                    fat_g: i.fat_g,
                    food_id: match i.food_id {
                        Some(fid) => Some(*food_map.get(&fid).ok_or_else(|| {
                            bad(format!("meal item references unresolved food {fid}"))
                        })?),
                        None => None,
                    },
                    origin: i.origin.clone(),
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let meal = MealIn {
            id: m.id.clone(),
            date: doc_date(&m.date, "meal date")?,
            meal_type: m.meal_type.clone(),
            description: m.description.clone(),
            scale: m.scale,
            assumptions: m.assumptions.clone(),
            confidence: m.confidence.clone(),
            source: m.source.clone(),
            provider_id: None,
            items,
        };
        // Preserve the effective time, but never trust a client clock into the future.
        let updated = ts_ms(&m.updated_at, "meal updated_at")?.min(now);
        let outcome =
            crate::sync::write_meal_on(&mut tx, user.id, &meal, updated, now, WriteMode::Restore)
                .await?;
        if outcome != Outcome::Applied {
            return Err(bad("a meal in the backup could not be restored"));
        }
        sqlx::query!("UPDATE meals SET deleted_at = NULL WHERE id = ?", m.id)
            .execute(&mut *tx)
            .await?;
    }

    // `write_meal_on` records barcoded items as products; the backup is the source of truth.
    sqlx::query!("DELETE FROM user_products WHERE user_id = ?", user.id)
        .execute(&mut *tx)
        .await?;

    for a in &doc.aliases {
        let alias = a.alias.trim();
        sqlx::query!(
            "INSERT INTO food_aliases (food_id, alias, alias_norm, user_id) VALUES (?, ?, ?, ?)",
            food_map[&a.food_id],
            alias,
            normalize(alias),
            user.id
        )
        .execute(&mut *tx)
        .await?;
    }

    for p in &doc.portions {
        sqlx::query!(
            "INSERT INTO user_portions (user_id, phrase_norm, food_id, grams) VALUES (?, ?, ?, ?)",
            user.id,
            p.phrase_norm.trim(),
            food_map[&p.food_id],
            p.grams
        )
        .execute(&mut *tx)
        .await?;
    }

    for p in &doc.products {
        sqlx::query!(
            "INSERT INTO user_products (user_id, food_id, last_servings, last_used_at)
             VALUES (?, ?, ?, ?)",
            user.id,
            food_map[&p.food_id],
            p.last_servings,
            ts_ms(&p.last_used_at, "product last_used_at")?
        )
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(Json(counts.json()))
}

struct Counts {
    profile_versions: usize,
    days: usize,
    meals: usize,
    items: usize,
    foods: usize,
    aliases: usize,
    portions: usize,
    products: usize,
}

impl Counts {
    fn json(&self) -> Value {
        json!({
            "profile_versions": self.profile_versions,
            "days": self.days,
            "meals": self.meals,
            "items": self.items,
            "foods": self.foods,
            "aliases": self.aliases,
            "portions": self.portions,
            "products": self.products,
        })
    }
}
