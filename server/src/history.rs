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
    m.items
        .iter()
        .map(|i| i.name.trim())
        .filter(|n| !n.is_empty())
        .collect::<Vec<_>>()
        .join(", ")
}

fn meal_key(label: &str) -> String {
    label
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// Returns one group per normalized label, in no particular order.
fn group_meals(meals: Vec<MealRow>) -> Vec<MealGroup> {
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

/// The most-eaten meals as compact summaries, grouped by normalized label before the limit.
pub(crate) fn top_meals(meals: Vec<MealRow>, limit: usize) -> Vec<Value> {
    let mut groups = group_meals(meals);
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
    let to = crate::time::user_today(&s.db, user.id).await?;
    let from = to - 364.days();
    let meals = load_meals(&s.db, user.id, &from.to_string(), &to.to_string()).await?;
    let mut groups = group_meals(meals);
    groups.sort_by(|a, b| {
        b.last_date
            .cmp(&a.last_date)
            .then_with(|| b.times.cmp(&a.times))
            .then_with(|| a.label.cmp(&b.label))
    });
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
mod tests {
    use super::*;

    fn item(name: &str, kcal: f64) -> ItemRow {
        ItemRow {
            id: format!("i-{name}-{kcal}"),
            name: name.into(),
            portion_text: "1 serving".into(),
            grams: Some(100.0),
            kcal,
            protein_g: 1.0,
            carbs_g: 1.0,
            fat_g: 1.0,
            food_id: None,
            origin: "manual".into(),
        }
    }

    fn meal(
        id: &str,
        date: &str,
        updated_at: i64,
        description: &str,
        items: Vec<ItemRow>,
    ) -> MealRow {
        MealRow {
            id: id.into(),
            date: date.into(),
            meal_type: "lunch".into(),
            description: description.into(),
            scale: 1.0,
            assumptions: None,
            confidence: None,
            source: "manual".into(),
            provider_id: None,
            updated_at,
            items,
        }
    }

    #[test]
    fn labels_fall_back_to_item_names() {
        let m = meal(
            "m1",
            "2026-11-01",
            1,
            "  ",
            vec![item("Rice", 200.0), item("Chicken", 300.0)],
        );
        assert_eq!(meal_label(&m), "Rice, Chicken");
    }

    #[test]
    fn labels_skip_blank_item_names() {
        let m = meal(
            "m1",
            "2026-11-01",
            1,
            "",
            vec![item("  ", 10.0), item("Rice", 200.0), item("\t", 5.0)],
        );
        assert_eq!(meal_label(&m), "Rice");
    }

    #[test]
    fn keys_collapse_case_and_whitespace() {
        assert_eq!(meal_key("  Oats   With  Banana "), "oats with banana");
    }

    #[test]
    fn keys_collapse_tabs_and_newlines() {
        assert_eq!(meal_key("oats\twith\n banana"), "oats with banana");
    }

    #[test]
    fn groups_by_normalized_description_and_averages_kcal() {
        let meals = vec![
            meal(
                "m1",
                "2026-11-01",
                1,
                "oats with banana",
                vec![item("Oats", 400.0)],
            ),
            meal(
                "m2",
                "2026-11-02",
                2,
                "Oats with banana",
                vec![item("Oats", 500.0)],
            ),
            meal(
                "m3",
                "2026-11-02",
                3,
                "Chicken rice",
                vec![item("Rice", 600.0)],
            ),
        ];
        let mut groups = group_meals(meals);
        groups.sort_by(|a, b| a.label.cmp(&b.label));
        assert_eq!(groups.len(), 2);
        let oats = groups
            .iter()
            .find(|g| g.label == "Oats with banana")
            .unwrap();
        assert_eq!(oats.times, 2);
        assert_eq!(oats.avg_kcal, 450.0);
        assert_eq!(oats.last_date, "2026-11-02");
        assert_eq!(oats.latest.id, "m2");
    }

    #[test]
    fn group_label_prefers_the_most_common_spelling() {
        let meals = vec![
            meal("m1", "2026-11-01", 1, "rice", vec![item("Rice", 100.0)]),
            meal("m2", "2026-11-02", 2, "Rice", vec![item("Rice", 200.0)]),
            meal("m3", "2026-11-03", 3, "rice", vec![item("Rice", 300.0)]),
        ];
        let groups = group_meals(meals);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].label, "rice");
        assert_eq!(groups[0].latest.id, "m3");
    }

    #[test]
    fn latest_representative_uses_updated_at() {
        let meals = vec![
            meal("m1", "2026-11-02", 10, "Rice", vec![item("Rice", 100.0)]),
            meal("m2", "2026-11-02", 20, "Rice", vec![item("Rice", 200.0)]),
        ];
        let groups = group_meals(meals);
        assert_eq!(groups[0].latest.id, "m2");
    }

    #[test]
    fn latest_representative_prefers_a_later_date() {
        let meals = vec![
            meal("m1", "2026-11-03", 5, "Rice", vec![item("Rice", 100.0)]),
            meal("m2", "2026-11-02", 100, "Rice", vec![item("Rice", 200.0)]),
        ];
        let groups = group_meals(meals);
        assert_eq!(groups[0].latest.id, "m1");
        assert_eq!(groups[0].last_date, "2026-11-03");
    }

    #[test]
    fn latest_representative_breaks_ties_on_larger_id() {
        let meals = vec![
            meal("m2", "2026-11-02", 10, "Rice", vec![item("Rice", 200.0)]),
            meal("m1", "2026-11-02", 10, "Rice", vec![item("Rice", 100.0)]),
        ];
        let groups = group_meals(meals);
        assert_eq!(groups[0].latest.id, "m2");
    }

    #[test]
    fn skips_itemless_meals() {
        let meals = vec![meal("m1", "2026-11-02", 1, "Rice", vec![])];
        assert!(group_meals(meals).is_empty());
    }
}
