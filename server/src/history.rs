// SPDX-License-Identifier: AGPL-3.0-or-later
//! Meal history grouping and the recent-meals endpoint.

use std::collections::{HashMap, hash_map::Entry};

#[allow(dead_code)] // every field is read by meal_json in Task 7
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

#[allow(dead_code)] // every field is read by meal_json in Task 7
pub(crate) struct MealRow {
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
    items: Vec<ItemRow>,
}

#[allow(dead_code)] // read by the recent-meals loader
struct MealGroup {
    label: String,
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

#[allow(dead_code)] // read by the recent-meals loader
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
                let newer = (meal.date.as_str(), meal.updated_at, meal.id.as_str())
                    > (
                        g.last_date.as_str(),
                        g.latest.updated_at,
                        g.latest.id.as_str(),
                    );
                if newer {
                    g.label = label;
                    g.last_date = meal.date.clone();
                    g.latest = meal;
                }
            }
            Entry::Vacant(e) => {
                e.insert(MealGroup {
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
    }
    out
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
    fn keys_collapse_case_and_whitespace() {
        assert_eq!(meal_key("  Oats   With  Banana "), "oats with banana");
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
    fn latest_representative_uses_updated_at() {
        let meals = vec![
            meal("m1", "2026-11-02", 10, "Rice", vec![item("Rice", 100.0)]),
            meal("m2", "2026-11-02", 20, "Rice", vec![item("Rice", 200.0)]),
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
