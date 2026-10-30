// SPDX-License-Identifier: AGPL-3.0-or-later
//! Tests for the nutrition pipeline.

use super::*;

fn m(kcal: f64, protein: f64, carbs: f64, fat: f64) -> Macros {
    Macros {
        kcal,
        protein,
        carbs,
        fat,
    }
}

#[test]
fn schema_is_strict_on_every_object() {
    let s = schema();
    let item = &s["properties"]["items"]["items"];
    for obj in [&s, item, &item["properties"]["estimate"]] {
        assert_eq!(obj["additionalProperties"], false);
        let props = obj["properties"].as_object().unwrap();
        let required: Vec<&str> = obj["required"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert!(
            props.keys().all(|k| required.contains(&k.as_str())),
            "{obj}"
        );
    }
}

#[test]
fn grams_must_be_positive_and_at_most_2000() {
    assert_eq!(valid_grams(Some(150.0)), Some(150.0));
    assert_eq!(valid_grams(Some(2000.0)), Some(2000.0));
    assert_eq!(valid_grams(Some(0.0)), None);
    assert_eq!(valid_grams(Some(-5.0)), None);
    assert_eq!(valid_grams(Some(2500.0)), None);
    assert_eq!(valid_grams(None), None);
}

#[test]
fn consistent_estimate_is_kept_per_100g() {
    // 4*4 + 4*43 + 9*0.4 = 191.6, within 20 % + 10 of 195.
    let p = per_100g(m(195.0, 4.0, 43.0, 0.4), 150.0).unwrap();
    assert_eq!(
        (r1(p.kcal), r1(p.protein), r1(p.carbs), r1(p.fat)),
        (130.0, 2.7, 28.7, 0.3)
    );
}

#[test]
fn inconsistent_kcal_is_recomputed_from_macros() {
    let p = per_100g(m(500.0, 10.0, 20.0, 5.0), 100.0).unwrap();
    assert_eq!(p.kcal, 165.0);
}

#[test]
fn implausible_energy_density_drops_the_estimate() {
    assert_eq!(per_100g(m(1000.0, 0.0, 0.0, 111.0), 100.0), None); // 999 kcal/100 g
    assert_eq!(per_100g(m(-50.0, -5.0, -5.0, 0.0), 100.0), None);
    assert!(per_100g(m(884.0, 0.0, 0.0, 98.0), 100.0).is_some()); // oil is fine
}

#[test]
fn household_measures() {
    let g = |u: &str, f: &str| unit_grams(&normalize(u), &normalize(f));
    assert_eq!(g("small cup", "cooked white rice"), Some(150.0));
    assert_eq!(g("cup", "rice"), Some(200.0));
    assert_eq!(g("plate", "rice"), Some(300.0));
    assert_eq!(g("bowl", "dal"), Some(250.0));
    assert_eq!(g("bowl", "chicken curry"), Some(250.0));
    assert_eq!(g("", "roti"), Some(40.0));
    assert_eq!(g("pieces", "chapatis"), Some(40.0));
    assert_eq!(g("tsp", "oil"), Some(4.5));
    assert_eq!(g("tablespoon", "ghee"), Some(13.5));
    assert_eq!(g("", "chicken breast"), Some(130.0));
    assert_eq!(g("g", "paneer"), Some(1.0));
    assert_eq!(g("bowl", "salad"), None);
    assert_eq!(g("lb", "steak"), Some(453.6));
    assert_eq!(g("pounds", "ground beef"), Some(453.6));
    assert_eq!(g("cup", "orange juice"), Some(240.0));
    assert_eq!(g("glass", "milk"), Some(250.0));
    assert_eq!(g("mug", "coffee"), Some(250.0));
    assert_eq!(g("can", "cola"), Some(355.0));
    assert_eq!(g("can", "beer"), Some(355.0));
    assert_eq!(g("bowl", "beef stew"), Some(250.0));
    assert_eq!(g("bowl", "chili"), Some(250.0));
    assert_eq!(spoon_grams("tsp"), Some(5.0));
    assert_eq!(spoon_grams("tbsp"), Some(15.0));
    assert_eq!(spoon_grams("cup"), None);
    assert_eq!(
        g("tbsp", "olive oil"),
        Some(13.5),
        "fat keeps its own weight"
    );
    assert_eq!(g("can", "tuna"), None, "not a drink");
    assert_eq!(g("shot", "espresso"), Some(30.0));
    assert_eq!(g("splash", "lactose free milk"), Some(30.0));
    assert_eq!(g("shot", "vodka"), None, "not in the drink list");
    assert_eq!(
        g("tsp", "chia seeds"),
        None,
        "generic spoons come after USDA portions"
    );
}

#[test]
fn oil_methods() {
    for c in [
        "shallow-fried",
        "pan fried",
        "Sautéed",
        "tadka",
        "roasted in oil",
        "stir-fried",
    ] {
        assert!(uses_oil(c), "{c}");
    }
    for c in ["none", "boiled", "grilled", "deep-fried", "steamed", ""] {
        assert!(!uses_oil(c), "{c}");
    }
}
