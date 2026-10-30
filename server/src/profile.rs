// SPDX-License-Identifier: AGPL-3.0-or-later
//! Profile and profile versions (PLAN.md §5). Online-only REST, not sync.

use crate::error::JsonBody;
use axum::Json;
use axum::extract::State;
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::SqlitePool;

use crate::AppState;
use crate::auth::{User, validate_timezone};
use crate::budget::{ProfileVersion, Sex};
use crate::error::{Error, Result};
use crate::time::{parse_date, today_in, user_today};

/// All of a user's profile versions, oldest first.
pub async fn versions(db: &SqlitePool, user_id: i64) -> Result<Vec<ProfileVersion>> {
    let rows = sqlx::query!(
        "SELECT effective_from, sex, birth_year, height_cm, start_weight_kg, goal_kg,
                pace_kg_per_week, typical_steps
         FROM profile_versions WHERE user_id = ? ORDER BY effective_from",
        user_id
    )
    .fetch_all(db)
    .await?;
    rows.into_iter()
        .map(|r| {
            Ok(ProfileVersion {
                effective_from: parse_date(&r.effective_from)?,
                sex: if r.sex == "male" {
                    Sex::Male
                } else {
                    Sex::Female
                },
                birth_year: r.birth_year as i16,
                height_cm: r.height_cm,
                start_weight_kg: r.start_weight_kg,
                goal_kg: r.goal_kg,
                pace_kg_per_week: r.pace_kg_per_week,
                typical_steps: r.typical_steps,
            })
        })
        .collect()
}

pub async fn get(State(s): State<AppState>, user: User) -> Result<Json<Value>> {
    let p = sqlx::query!(
        "SELECT timezone, weight_unit, height_unit FROM profiles WHERE user_id = ?",
        user.id
    )
    .fetch_one(&s.db)
    .await?;
    let today = user_today(&s.db, user.id).await?;
    let versions = versions(&s.db, user.id).await?;
    Ok(Json(json!({
        "timezone": p.timezone,
        "weight_unit": p.weight_unit,
        "height_unit": p.height_unit,
        "version": crate::budget::version_on(today, &versions),
    })))
}

#[derive(Deserialize)]
pub struct ProfileIn {
    timezone: String,
    weight_unit: String,
    height_unit: String,
    sex: Sex,
    birth_year: i16,
    height_cm: f64,
    start_weight_kg: f64,
    goal_kg: Option<f64>,
    pace_kg_per_week: f64,
    typical_steps: i64,
}

fn check(ok: bool, msg: &str) -> Result<()> {
    ok.then_some(()).ok_or_else(|| Error::bad_request(msg))
}

/// Saves units and timezone, and writes the version effective from the user's today.
pub async fn put(
    State(s): State<AppState>,
    user: User,
    JsonBody(p): JsonBody<ProfileIn>,
) -> Result<Json<Value>> {
    validate_timezone(&p.timezone)?;
    let today = today_in(&p.timezone)?;
    check(
        ["kg", "lb"].contains(&p.weight_unit.as_str()),
        "weight_unit must be kg or lb",
    )?;
    check(
        ["cm", "ftin"].contains(&p.height_unit.as_str()),
        "height_unit must be cm or ftin",
    )?;
    check(
        (1900..=today.year()).contains(&p.birth_year),
        "birth_year out of range",
    )?;
    check(
        (50.0..=272.0).contains(&p.height_cm),
        "height_cm must be 50-272",
    )?;
    check(
        (20.0..=400.0).contains(&p.start_weight_kg),
        "start_weight_kg must be 20-400",
    )?;
    check(
        p.goal_kg.is_none_or(|g| (20.0..=400.0).contains(&g)),
        "goal_kg must be 20-400",
    )?;
    check(
        (0.0..=1.0).contains(&p.pace_kg_per_week),
        "pace_kg_per_week must be 0-1",
    )?;
    check(
        (0..=150_000).contains(&p.typical_steps),
        "typical_steps must be 0-150000",
    )?;

    let today_s = today.to_string();
    let sex = match p.sex {
        Sex::Male => "male",
        Sex::Female => "female",
    };
    // See create_alias: a background queue flush may hold the write lock.
    let mut tx = s.db.begin_with("BEGIN IMMEDIATE").await?;
    sqlx::query!(
        "UPDATE profiles SET timezone = ?, weight_unit = ?, height_unit = ? WHERE user_id = ?",
        p.timezone,
        p.weight_unit,
        p.height_unit,
        user.id
    )
    .execute(&mut *tx)
    .await?;
    sqlx::query!(
        "INSERT INTO profile_versions (user_id, effective_from, sex, birth_year, height_cm,
             start_weight_kg, goal_kg, pace_kg_per_week, typical_steps)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT (user_id, effective_from) DO UPDATE SET sex = excluded.sex,
             birth_year = excluded.birth_year, height_cm = excluded.height_cm,
             start_weight_kg = excluded.start_weight_kg, goal_kg = excluded.goal_kg,
             pace_kg_per_week = excluded.pace_kg_per_week, typical_steps = excluded.typical_steps",
        user.id,
        today_s,
        sex,
        p.birth_year,
        p.height_cm,
        p.start_weight_kg,
        p.goal_kg,
        p.pace_kg_per_week,
        p.typical_steps
    )
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(Json(json!({})))
}
