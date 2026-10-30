// SPDX-License-Identifier: AGPL-3.0-or-later
//! `GET /api/export.csv`.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use axum::body::Body;
use axum::extract::State;
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::Response;

use super::HEADER;
use crate::AppState;
use crate::auth::User;
use crate::error::{Error, Result};
use crate::time::user_today;

struct ExportMeal {
    id: String,
    date: String,
    meal_type: String,
    description: String,
}

struct ExportItem {
    meal_id: String,
    position: i64,
    name: String,
    portion_text: String,
    kcal: f64,
    protein_g: f64,
    carbs_g: f64,
    fat_g: f64,
}

struct ExportDay {
    date: String,
    steps: Option<i64>,
    gym_minutes: Option<i64>,
    trained: Option<String>,
    weight_kg: Option<f64>,
}

fn opt_num<T: ToString>(v: Option<T>) -> String {
    v.map(|v| v.to_string()).unwrap_or_default()
}

/// A leading `'` keeps spreadsheet apps from evaluating a text cell as a formula.
fn guard(s: &str) -> String {
    if s.starts_with(['=', '+', '-', '@']) {
        format!("'{s}")
    } else {
        s.to_string()
    }
}

fn trained_text(json: Option<&str>) -> String {
    json.and_then(|t| serde_json::from_str::<Vec<String>>(t).ok())
        .unwrap_or_default()
        .join(";")
}

pub async fn export(State(s): State<AppState>, user: User) -> Result<Response> {
    let today = user_today(&s.db, user.id).await?;

    let meals = sqlx::query_as!(
        ExportMeal,
        "SELECT id, date, meal_type, description FROM meals
         WHERE user_id = ? AND deleted_at IS NULL ORDER BY date, id",
        user.id
    )
    .fetch_all(&s.db)
    .await?;

    let items = sqlx::query_as!(
        ExportItem,
        "SELECT i.meal_id, i.position, i.name, i.portion_text, i.kcal, i.protein_g, i.carbs_g,
                i.fat_g
         FROM meal_items i JOIN meals m ON m.id = i.meal_id
         WHERE m.user_id = ? AND m.deleted_at IS NULL
         ORDER BY i.meal_id, i.position",
        user.id
    )
    .fetch_all(&s.db)
    .await?;

    let days = sqlx::query_as!(
        ExportDay,
        "SELECT date, steps, gym_minutes, trained, weight_kg FROM days
         WHERE user_id = ? AND (steps IS NOT NULL OR gym_minutes IS NOT NULL
             OR trained IS NOT NULL OR weight_kg IS NOT NULL)
         ORDER BY date",
        user.id
    )
    .fetch_all(&s.db)
    .await?;

    let mut items_by_meal: HashMap<&str, Vec<&ExportItem>> = HashMap::new();
    for item in &items {
        items_by_meal
            .entry(item.meal_id.as_str())
            .or_default()
            .push(item);
    }
    let mut meals_by_date: BTreeMap<&str, Vec<&ExportMeal>> = BTreeMap::new();
    for meal in &meals {
        meals_by_date
            .entry(meal.date.as_str())
            .or_default()
            .push(meal);
    }
    let days_by_date: BTreeMap<&str, &ExportDay> =
        days.iter().map(|d| (d.date.as_str(), d)).collect();
    let dates: BTreeSet<&str> = meals_by_date
        .keys()
        .chain(days_by_date.keys())
        .copied()
        .collect();

    let mut writer = csv::WriterBuilder::new()
        .terminator(csv::Terminator::Any(b'\n'))
        .from_writer(Vec::new());
    writer
        .write_record(HEADER)
        .map_err(|e| Error::Internal(e.to_string()))?;

    for date in dates {
        let day = days_by_date.get(date).copied();
        let meals = meals_by_date.get(date).map(Vec::as_slice).unwrap_or(&[]);
        if meals.is_empty() {
            write_row(&mut writer, date, None, None, day)?;
            continue;
        }
        for &meal in meals {
            match items_by_meal.get(meal.id.as_str()).map(Vec::as_slice) {
                None | Some([]) => write_row(&mut writer, date, Some(meal), None, day)?,
                Some(items) => {
                    for &item in items {
                        write_row(&mut writer, date, Some(meal), Some(item), day)?;
                    }
                }
            }
        }
    }

    let bytes = writer
        .into_inner()
        .map_err(|e| Error::Internal(e.to_string()))?;
    let mut res = Response::new(Body::from(bytes));
    *res.status_mut() = StatusCode::OK;
    let h = res.headers_mut();
    h.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/csv; charset=utf-8"),
    );
    h.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&format!("attachment; filename=\"cupful-{today}.csv\""))
            .map_err(|e| Error::Internal(e.to_string()))?,
    );
    h.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    Ok(res)
}

fn write_row(
    writer: &mut csv::Writer<Vec<u8>>,
    date: &str,
    meal: Option<&ExportMeal>,
    item: Option<&ExportItem>,
    day: Option<&ExportDay>,
) -> Result<()> {
    let record = [
        date.to_string(),
        meal.map(|m| m.id.clone()).unwrap_or_default(),
        opt_num(item.map(|i| i.position)),
        meal.map(|m| m.meal_type.clone()).unwrap_or_default(),
        guard(meal.map_or("", |m| &m.description)),
        guard(item.map_or("", |i| &i.name)),
        guard(item.map_or("", |i| &i.portion_text)),
        opt_num(item.map(|i| i.kcal)),
        opt_num(item.map(|i| i.protein_g)),
        opt_num(item.map(|i| i.carbs_g)),
        opt_num(item.map(|i| i.fat_g)),
        opt_num(day.and_then(|d| d.steps)),
        opt_num(day.and_then(|d| d.gym_minutes)),
        day.map(|d| trained_text(d.trained.as_deref()))
            .unwrap_or_default(),
        opt_num(day.and_then(|d| d.weight_kg)),
    ];
    writer
        .write_record(record)
        .map_err(|e| Error::Internal(e.to_string()))
}
