// SPDX-License-Identifier: AGPL-3.0-or-later
//! Meal history grouping and the recent-meals endpoint.

use std::collections::{HashMap, hash_map::Entry};

use axum::Json;
use axum::extract::State;
use jiff::ToSpan;
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::SqlitePool;

use crate::AppState;
use crate::auth::User;
use crate::error::{Error, Query, Result};

struct ItemRow {
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

pub(crate) struct MealRow {
    pub(crate) id: String,
    date: String,
    meal_type: String,
    description: String,
    scale: f64,
    assumptions: Option<String>,
    confidence: Option<String>,
    source: String,
    provider_id: Option<i64>,
    updated_at: i64,
    items: Vec<ItemRow>,
}

struct MealGroup {
    label: String,
    /// Exact spellings seen in the group and how often, to pick a canonical display label.
    labels: HashMap<String, usize>,
    times: usize,
    avg_kcal: f64,
    last_date: String,
    latest: MealRow,
}

fn meal_label(m: &MealRow) -> String {
    let description = m.description.trim();
    if !description.is_empty() {
        return description.to_string();
    }
    let mut names: Vec<_> = m
        .items
        .iter()
        .map(|i| i.name.trim())
        .filter(|n| !n.is_empty())
        .collect();
    names.sort_unstable();
    names.join(", ")
}

fn meal_key(label: &str) -> String {
    label
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// Returns one group per normalized label, in no particular order. When
/// `split_meal_type` is set, the same label in different slots stays separate.
fn group_meals(meals: Vec<MealRow>, split_meal_type: bool) -> Vec<MealGroup> {
    let mut groups: HashMap<String, MealGroup> = HashMap::new();
    for meal in meals {
        if meal.items.is_empty() {
            continue;
        }
        let label = meal_label(&meal);
        if label.is_empty() {
            continue;
        }
        let key = meal_key(&label);
        let key = if split_meal_type {
            format!("{}|{}", meal.meal_type, key)
        } else {
            key
        };
        let kcal = meal.items.iter().map(|i| i.kcal).sum::<f64>();
        match groups.entry(key) {
            Entry::Occupied(mut e) => {
                let g = e.get_mut();
                g.times += 1;
                g.avg_kcal += kcal;
                *g.labels.entry(label).or_insert(0) += 1;
                let newer = (meal.date.as_str(), meal.updated_at, meal.id.as_str())
                    > (
                        g.last_date.as_str(),
                        g.latest.updated_at,
                        g.latest.id.as_str(),
                    );
                if newer {
                    g.last_date = meal.date.clone();
                    g.latest = meal;
                }
            }
            Entry::Vacant(e) => {
                e.insert(MealGroup {
                    labels: HashMap::from([(label.clone(), 1)]),
                    label,
                    times: 1,
                    avg_kcal: kcal,
                    last_date: meal.date.clone(),
                    latest: meal,
                });
            }
        }
    }
    let mut out: Vec<_> = groups.into_values().collect();
    for group in &mut out {
        group.avg_kcal /= group.times as f64;
        // Show the spelling used most often; ties go to the lexicographically first.
        group.label = group
            .labels
            .iter()
            .max_by(|a, b| a.1.cmp(b.1).then_with(|| b.0.cmp(a.0)))
            .map(|(label, _)| label.clone())
            .expect("a group has at least one label");
    }
    out
}

#[derive(Clone, Copy, Default, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum HistorySort {
    #[default]
    Recent,
    Frequent,
}

fn product_items(g: &MealGroup) -> usize {
    g.latest
        .items
        .iter()
        .filter(|i| i.origin == "food_db")
        .count()
}

/// Recent first, or frequency first with the built product meal winning ties.
fn sort_groups(groups: &mut [MealGroup], sort: HistorySort) {
    match sort {
        HistorySort::Recent => groups.sort_by(|a, b| {
            b.last_date
                .cmp(&a.last_date)
                .then_with(|| b.times.cmp(&a.times))
                .then_with(|| a.label.cmp(&b.label))
        }),
        HistorySort::Frequent => groups.sort_by(|a, b| {
            b.times
                .cmp(&a.times)
                .then_with(|| product_items(b).cmp(&product_items(a)))
                .then_with(|| b.last_date.cmp(&a.last_date))
                .then_with(|| a.label.cmp(&b.label))
        }),
    }
}

/// The most-eaten meals as compact summaries, grouped by normalized label before the limit.
pub(crate) fn top_meals(meals: Vec<MealRow>, limit: usize) -> Vec<Value> {
    let mut groups = group_meals(meals, false);
    groups.sort_by(|a, b| {
        b.times
            .cmp(&a.times)
            .then_with(|| b.last_date.cmp(&a.last_date))
            .then_with(|| a.label.cmp(&b.label))
    });
    groups.truncate(limit);
    groups
        .into_iter()
        .map(|g| {
            json!({
                "label": g.label,
                "meal_type": g.latest.meal_type,
                "times": g.times,
                "avg_kcal": g.avg_kcal.round() as i64,
                "last_date": g.last_date,
            })
        })
        .collect()
}

#[derive(Deserialize)]
pub struct HistoryQuery {
    limit: Option<i64>,
    days: Option<i64>,
    meal_type: Option<String>,
    #[serde(default)]
    sort: HistorySort,
}

pub async fn meal_history(
    State(s): State<AppState>,
    user: User,
    Query(q): Query<HistoryQuery>,
) -> Result<Json<Value>> {
    let limit = q.limit.unwrap_or(20);
    if !(1..=50).contains(&limit) {
        return Err(Error::bad_request("limit must be between 1 and 50"));
    }
    let days = q.days.unwrap_or(365);
    if !(1..=365).contains(&days) {
        return Err(Error::bad_request("days must be between 1 and 365"));
    }
    let meal_type = match q.meal_type.as_deref() {
        None => None,
        Some(t @ ("breakfast" | "lunch" | "dinner" | "snack")) => Some(t),
        Some(_) => return Err(Error::bad_request("unknown meal_type")),
    };
    let to = crate::time::user_today(&s.db, user.id).await?;
    let from = to - (days - 1).days();
    let mut meals = load_meals(&s.db, user.id, &from.to_string(), &to.to_string()).await?;
    if let Some(meal_type) = meal_type {
        meals.retain(|m| m.meal_type == meal_type);
    }
    let mut groups = group_meals(meals, true);
    sort_groups(&mut groups, q.sort);
    groups.truncate(limit as usize);
    let out: Vec<Value> = groups
        .into_iter()
        .map(|g| {
            let mut v = meal_json(&g.latest);
            v["label"] = json!(g.label);
            v["times"] = json!(g.times);
            v["last_date"] = json!(g.last_date);
            v
        })
        .collect();
    Ok(Json(json!({ "meals": out })))
}

pub(crate) async fn load_meals(
    db: &SqlitePool,
    user_id: i64,
    from: &str,
    to: &str,
) -> Result<Vec<MealRow>> {
    let meals = sqlx::query!(
        "SELECT id, date, meal_type, description, scale, assumptions, confidence, source,
                provider_id, updated_at
         FROM meals
         WHERE user_id = ? AND date BETWEEN ? AND ? AND deleted_at IS NULL
         ORDER BY date, updated_at, id",
        user_id,
        from,
        to
    )
    .fetch_all(db)
    .await?;
    let items = sqlx::query!(
        "SELECT i.meal_id, i.id, i.name, i.portion_text, i.grams, i.kcal, i.protein_g,
                i.carbs_g, i.fat_g, i.food_id, i.origin
         FROM meal_items i JOIN meals m ON m.id = i.meal_id
         WHERE m.user_id = ? AND m.date BETWEEN ? AND ? AND m.deleted_at IS NULL
         ORDER BY i.meal_id, i.position",
        user_id,
        from,
        to
    )
    .fetch_all(db)
    .await?;
    let mut by_meal: HashMap<String, Vec<ItemRow>> = HashMap::new();
    for i in items {
        by_meal.entry(i.meal_id.clone()).or_default().push(ItemRow {
            id: i.id,
            name: i.name,
            portion_text: i.portion_text,
            grams: i.grams,
            kcal: i.kcal,
            protein_g: i.protein_g,
            carbs_g: i.carbs_g,
            fat_g: i.fat_g,
            food_id: i.food_id,
            origin: i.origin,
        });
    }
    Ok(meals
        .into_iter()
        .map(|m| {
            let items = by_meal.remove(&m.id).unwrap_or_default();
            MealRow {
                id: m.id,
                date: m.date,
                meal_type: m.meal_type,
                description: m.description,
                scale: m.scale,
                assumptions: m.assumptions,
                confidence: m.confidence,
                source: m.source,
                provider_id: m.provider_id,
                updated_at: m.updated_at,
                items,
            }
        })
        .collect())
}

pub(crate) fn meal_json(m: &MealRow) -> Value {
    let sum = |f: fn(&ItemRow) -> f64| m.items.iter().map(f).sum::<f64>();
    json!({
        "id": m.id,
        "date": m.date,
        "meal_type": m.meal_type,
        "description": m.description,
        "scale": m.scale,
        "assumptions": m.assumptions,
        "confidence": m.confidence,
        "source": m.source,
        "provider_id": m.provider_id,
        "updated_at": m.updated_at,
        "items": m.items.iter().map(|i| json!({
            "id": i.id,
            "name": i.name,
            "portion_text": i.portion_text,
            "grams": i.grams,
            "kcal": i.kcal,
            "protein_g": i.protein_g,
            "carbs_g": i.carbs_g,
            "fat_g": i.fat_g,
            "food_id": i.food_id,
            "origin": i.origin,
        })).collect::<Vec<_>>(),
        "totals": {
            "kcal": sum(|i| i.kcal),
            "protein_g": sum(|i| i.protein_g),
            "carbs_g": sum(|i| i.carbs_g),
            "fat_g": sum(|i| i.fat_g),
        },
    })
}

#[cfg(test)]
mod tests;
