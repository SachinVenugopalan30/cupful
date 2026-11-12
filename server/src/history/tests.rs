// SPDX-License-Identifier: AGPL-3.0-or-later

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

fn product_item(name: &str, kcal: f64) -> ItemRow {
    ItemRow {
        origin: "food_db".into(),
        food_id: Some(1),
        ..item(name, kcal)
    }
}

fn meal(id: &str, date: &str, updated_at: i64, description: &str, items: Vec<ItemRow>) -> MealRow {
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

fn typed(mut m: MealRow, meal_type: &str) -> MealRow {
    m.meal_type = meal_type.into();
    m
}

#[test]
fn labels_fall_back_to_sorted_item_names() {
    let m = meal(
        "m1",
        "2026-11-01",
        1,
        "  ",
        vec![item("Rice", 200.0), item("Chicken", 300.0)],
    );
    assert_eq!(meal_label(&m), "Chicken, Rice");
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
    let mut groups = group_meals(meals, false);
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
fn groups_separate_meal_types() {
    let meals = vec![
        typed(
            meal("m1", "2026-11-01", 1, "Rice", vec![item("Rice", 100.0)]),
            "breakfast",
        ),
        typed(
            meal("m2", "2026-11-02", 2, "Rice", vec![item("Rice", 200.0)]),
            "lunch",
        ),
    ];
    let groups = group_meals(meals, true);
    assert_eq!(groups.len(), 2);
    assert_eq!(
        groups
            .iter()
            .filter(|g| g.label == "Rice" && g.times == 1)
            .count(),
        2
    );
}

#[test]
fn groups_merge_meal_types_when_not_split() {
    let meals = vec![
        typed(
            meal("m1", "2026-11-01", 1, "Rice", vec![item("Rice", 100.0)]),
            "breakfast",
        ),
        typed(
            meal("m2", "2026-11-02", 2, "Rice", vec![item("Rice", 200.0)]),
            "lunch",
        ),
    ];
    let groups = group_meals(meals, false);
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].label, "Rice");
    assert_eq!(groups[0].times, 2);
}

#[test]
fn group_label_prefers_the_most_common_spelling() {
    let meals = vec![
        meal("m1", "2026-11-01", 1, "rice", vec![item("Rice", 100.0)]),
        meal("m2", "2026-11-02", 2, "Rice", vec![item("Rice", 200.0)]),
        meal("m3", "2026-11-03", 3, "rice", vec![item("Rice", 300.0)]),
    ];
    let groups = group_meals(meals, false);
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
    let groups = group_meals(meals, false);
    assert_eq!(groups[0].latest.id, "m2");
}

#[test]
fn latest_representative_prefers_a_later_date() {
    let meals = vec![
        meal("m1", "2026-11-03", 5, "Rice", vec![item("Rice", 100.0)]),
        meal("m2", "2026-11-02", 100, "Rice", vec![item("Rice", 200.0)]),
    ];
    let groups = group_meals(meals, false);
    assert_eq!(groups[0].latest.id, "m1");
    assert_eq!(groups[0].last_date, "2026-11-03");
}

#[test]
fn latest_representative_breaks_ties_on_larger_id() {
    let meals = vec![
        meal("m2", "2026-11-02", 10, "Rice", vec![item("Rice", 200.0)]),
        meal("m1", "2026-11-02", 10, "Rice", vec![item("Rice", 100.0)]),
    ];
    let groups = group_meals(meals, false);
    assert_eq!(groups[0].latest.id, "m2");
}

#[test]
fn skips_itemless_meals() {
    let meals = vec![meal("m1", "2026-11-02", 1, "Rice", vec![])];
    assert!(group_meals(meals, false).is_empty());
}

#[test]
fn frequent_sort_prefers_the_group_with_product_items_on_a_tie() {
    let ai =
        |id: &str, date: &str, at: i64| meal(id, date, at, "AI bowl", vec![item("Bowl", 100.0)]);
    let product = |id: &str, date: &str, at: i64| {
        meal(
            id,
            date,
            at,
            "Product bowl",
            vec![product_item("Bowl", 100.0)],
        )
    };
    let mut groups = group_meals(
        vec![
            ai("a1", "2026-11-01", 1),
            ai("a2", "2026-11-02", 2),
            ai("a3", "2026-11-03", 3),
            product("p1", "2026-10-30", 4),
            product("p2", "2026-10-31", 5),
            product("p3", "2026-10-31", 6),
        ],
        false,
    );
    assert_eq!(groups.len(), 2);
    sort_groups(&mut groups, HistorySort::Frequent);
    // Equal frequency: the more recently seen AI group would win on date alone, but its latest
    // meal has no product items, so the built product meal comes first.
    assert_eq!(groups[0].label, "Product bowl");
    assert_eq!(groups[1].label, "AI bowl");
}

#[test]
fn frequent_sort_orders_by_times_before_product_items() {
    let mut groups = group_meals(
        vec![
            meal("a1", "2026-11-01", 1, "AI bowl", vec![item("Bowl", 100.0)]),
            meal("a2", "2026-11-02", 2, "AI bowl", vec![item("Bowl", 100.0)]),
            meal(
                "p1",
                "2026-11-03",
                3,
                "Product bowl",
                vec![product_item("Bowl", 100.0)],
            ),
        ],
        false,
    );
    sort_groups(&mut groups, HistorySort::Frequent);
    assert_eq!(groups[0].label, "AI bowl");
    assert_eq!(groups[1].label, "Product bowl");
}

#[test]
fn fallback_labels_sort_item_names_ignoring_case() {
    let lower = meal(
        "m1",
        "2026-11-01",
        1,
        "",
        vec![item("Apple", 50.0), item("banana", 100.0)],
    );
    let mixed = meal(
        "m2",
        "2026-11-02",
        2,
        "",
        vec![item("apple", 60.0), item("Banana", 90.0)],
    );
    // The same two foods in a different case order must produce one group, not two.
    let groups = group_meals(vec![lower, mixed], true);
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].times, 2);
}

#[test]
fn sort_breaks_a_full_tie_on_meal_type() {
    let breakfast = || {
        typed(
            meal("m1", "2026-11-02", 2, "Rice", vec![item("Rice", 100.0)]),
            "breakfast",
        )
    };
    let snack = || {
        typed(
            meal("m2", "2026-11-02", 2, "Rice", vec![item("Rice", 100.0)]),
            "snack",
        )
    };

    // Same label, times, date and items: only the slot tells the two groups apart.
    for reverse in [false, true] {
        for sort in [HistorySort::Recent, HistorySort::Frequent] {
            let meals = if reverse {
                vec![snack(), breakfast()]
            } else {
                vec![breakfast(), snack()]
            };
            let mut groups = group_meals(meals, true);
            assert_eq!(groups.len(), 2);
            sort_groups(&mut groups, sort);
            assert_eq!(groups[0].latest.meal_type, "breakfast");
        }
    }
}
