// SPDX-License-Identifier: AGPL-3.0-or-later
//! Read endpoints for logged data and the budget (PLAN.md §6).

use crate::error::Query;
use axum::Json;
use axum::extract::{Path, State};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::SqlitePool;

use crate::AppState;
use crate::auth::User;
use crate::budget::{self, DayActivity};
use crate::error::{Error, Result};
use crate::time::parse_date;

const MAX_RANGE_DAYS: i32 = 400;

struct DayRow {
    date: String,
    steps: Option<i64>,
    steps_source: Option<String>,
    gym_minutes: Option<i64>,
    gym_source: Option<String>,
    trained: Option<String>,
    weight_kg: Option<f64>,
}

impl DayRow {
    fn trained(&self) -> Vec<String> {
        self.trained
            .as_deref()
            .and_then(|t| serde_json::from_str(t).ok())
            .unwrap_or_default()
    }

    fn to_json(&self) -> Value {
        json!({
            "date": self.date,
            "steps": self.steps,
            "steps_source": self.steps_source,
            "gym_minutes": self.gym_minutes,
            "gym_source": self.gym_source,
            "trained": self.trained(),
            "weight_kg": self.weight_kg,
        })
    }
}

async fn days_between(db: &SqlitePool, user_id: i64, from: &str, to: &str) -> Result<Vec<DayRow>> {
    Ok(sqlx::query_as!(
        DayRow,
        "SELECT date, steps, steps_source, gym_minutes, gym_source, trained, weight_kg
         FROM days WHERE user_id = ? AND date BETWEEN ? AND ? ORDER BY date",
        user_id,
        from,
        to
    )
    .fetch_all(db)
    .await?)
}

pub async fn day(
    State(s): State<AppState>,
    user: User,
    Path(date): Path<String>,
) -> Result<Json<Value>> {
    let date = parse_date(&date)?.to_string();
    let row = days_between(&s.db, user.id, &date, &date).await?.pop();
    Ok(Json(
        row.unwrap_or(DayRow {
            date,
            steps: None,
            steps_source: None,
            gym_minutes: None,
            gym_source: None,
            trained: None,
            weight_kg: None,
        })
        .to_json(),
    ))
}

#[derive(Deserialize)]
pub struct Range {
    from: String,
    to: String,
}

pub async fn days(
    State(s): State<AppState>,
    user: User,
    Query(r): Query<Range>,
) -> Result<Json<Value>> {
    let (from, to) = (parse_date(&r.from)?, parse_date(&r.to)?);
    let span = (to - from).get_days();
    if !(0..MAX_RANGE_DAYS).contains(&span) {
        return Err(Error::bad_request(format!(
            "`from` must be on or before `to`, at most {MAX_RANGE_DAYS} days apart"
        )));
    }
    let rows = days_between(&s.db, user.id, &from.to_string(), &to.to_string()).await?;
    Ok(Json(
        json!({ "days": rows.iter().map(DayRow::to_json).collect::<Vec<_>>() }),
    ))
}

#[derive(Deserialize)]
pub struct DateQuery {
    date: String,
}

pub async fn meals(
    State(s): State<AppState>,
    user: User,
    Query(q): Query<DateQuery>,
) -> Result<Json<Value>> {
    let date = parse_date(&q.date)?.to_string();
    // UUIDv7 ids sort by creation time.
    let meals = sqlx::query!(
        "SELECT id, date, meal_type, description, scale, assumptions, confidence, source,
                provider_id, updated_at
         FROM meals WHERE user_id = ? AND date = ? AND deleted_at IS NULL ORDER BY id",
        user.id,
        date
    )
    .fetch_all(&s.db)
    .await?;
    let mut out = Vec::with_capacity(meals.len());
    for m in meals {
        let items = sqlx::query!(
            "SELECT id, name, portion_text, grams, kcal, protein_g, carbs_g, fat_g, food_id, origin
             FROM meal_items WHERE meal_id = ? ORDER BY position",
            m.id
        )
        .fetch_all(&s.db)
        .await?;
        let sum = |f: fn(&_) -> f64| items.iter().map(f).sum::<f64>();
        let totals = json!({
            "kcal": sum(|i| i.kcal),
            "protein_g": sum(|i| i.protein_g),
            "carbs_g": sum(|i| i.carbs_g),
            "fat_g": sum(|i| i.fat_g),
        });
        let items: Vec<Value> = items
            .iter()
            .map(|i| {
                json!({
                    "id": i.id, "name": i.name, "portion_text": i.portion_text, "grams": i.grams,
                    "kcal": i.kcal, "protein_g": i.protein_g, "carbs_g": i.carbs_g,
                    "fat_g": i.fat_g, "food_id": i.food_id, "origin": i.origin,
                })
            })
            .collect();
        out.push(json!({
            "id": m.id, "date": m.date, "meal_type": m.meal_type, "description": m.description,
            "scale": m.scale, "assumptions": m.assumptions, "confidence": m.confidence,
            "source": m.source, "provider_id": m.provider_id, "updated_at": m.updated_at,
            "items": items, "totals": totals,
        }));
    }
    Ok(Json(json!({ "meals": out })))
}

/// Recomputed from current data every time (PLAN.md §7 "History").
pub async fn budget(
    State(s): State<AppState>,
    user: User,
    Path(date): Path<String>,
) -> Result<Json<budget::Budget>> {
    let date = parse_date(&date)?;
    let versions = crate::profile::versions(&s.db, user.id).await?;
    let version = budget::version_on(date, &versions).ok_or_else(|| {
        Error::conflict(format!(
            "no profile in effect on {date}; save your profile first"
        ))
    })?;
    let d = date.to_string();
    let weight = sqlx::query_scalar!(
        "SELECT weight_kg AS \"w!: f64\" FROM days
         WHERE user_id = ? AND date <= ? AND weight_kg IS NOT NULL ORDER BY date DESC LIMIT 1",
        user.id,
        d
    )
    .fetch_optional(&s.db)
    .await?;
    let day = days_between(&s.db, user.id, &d, &d).await?.pop();
    let activity = day.map_or_else(DayActivity::default, |r| DayActivity {
        steps: r.steps,
        gym_minutes: r.gym_minutes,
        trained: r.trained(),
    });
    Ok(Json(budget::compute(date, version, weight, &activity)))
}
