// SPDX-License-Identifier: AGPL-3.0-or-later
//! Tests for the food database.

use sqlx::SqlitePool;

use super::find::{classify, score};
use super::seed::{ALIASES, STAPLES, csv};
use super::*;

#[test]
fn usda_portion_text_becomes_a_unit_and_amount() {
    let u = |t: &str, a: Option<f64>| portion_unit(t, a);
    assert_eq!(u("cup", Some(1.0)), Some(("cup".into(), 1.0)));
    assert_eq!(u("scoop", Some(3.0)), Some(("scoop".into(), 3.0)));
    assert_eq!(u("cups, chopped", Some(0.5)), Some(("cup".into(), 0.5)));
    assert_eq!(
        u("1 piece, medium pizza", None),
        Some(("piece".into(), 1.0))
    );
    assert_eq!(u("2 tablespoons", None), Some(("tbsp".into(), 2.0)));
    assert_eq!(u("1/2 cup", None), Some(("cup".into(), 0.5)));
    assert_eq!(u("fl oz", Some(1.0)), Some(("fl oz".into(), 1.0)));
    assert_eq!(
        u("medium (2-1/4\" to 3\" dia)", Some(1.0)),
        Some(("medium".into(), 1.0))
    );
    assert_eq!(
        u("1 medium or regular slice", None),
        Some(("slice".into(), 1.0))
    );
    assert_eq!(
        u("1 large or thick slice", None),
        Some(("slice".into(), 1.0))
    );
    assert_eq!(
        u("medium whole (2-3/5\" dia)", Some(1.0)),
        Some(("medium".into(), 1.0))
    );
    assert_eq!(u("Quantity not specified", None), None);
    assert_eq!(u("1 surface inch", None), None);
    assert_eq!(u("", Some(1.0)), None);
}

#[test]
fn units_have_canonical_lookup_keys() {
    assert_eq!(unit_keys("Tablespoons"), ["tbsp"]);
    assert_eq!(unit_keys("1 scoop"), ["scoop"]);
    assert_eq!(unit_keys("large egg"), ["large egg", "large"]);
    assert_eq!(unit_keys("slice"), ["slice", "piece"]);
    assert_eq!(unit_keys("fl oz"), ["fl oz"]);
    assert!(unit_keys("").is_empty());
}

#[tokio::test]
async fn staples_get_usda_grams_per_unit() {
    let db = db().await;
    let grams = |name: &'static str, unit: &'static str| {
        let db = db.clone();
        async move {
            sqlx::query_scalar!(
                "SELECT p.grams FROM food_portions p JOIN foods f ON f.id = p.food_id
                     WHERE f.source = 'staple' AND f.name = ? AND p.unit = ?",
                name,
                unit
            )
            .fetch_optional(&db)
            .await
            .unwrap()
        }
    };
    let scoop = grams("Whey protein powder", "scoop").await.unwrap();
    assert!((scoop - 86.0 / 3.0).abs() < 0.01, "{scoop}");
    assert_eq!(
        grams("Oats, rolled, dry", "cup").await,
        Some(81.0),
        "first plain cup wins"
    );
    assert_eq!(grams("Espresso", "fl oz").await, Some(29.6));
    assert_eq!(
        grams("Toast, white bread", "slice").await,
        Some(25.0),
        "a regular slice, not snack-size"
    );
    seed(&db).await.unwrap();
    assert_eq!(
        grams("Oats, rolled, dry", "cup").await,
        Some(81.0),
        "reseeding is idempotent"
    );
}

#[test]
fn normalize_lowercases_strips_punctuation_and_singularizes() {
    assert_eq!(normalize("Cooked White Rice!"), "cooked white rice");
    assert_eq!(
        normalize("Chickpeas (chana), boiled"),
        "chickpea chana boiled"
    );
    assert_eq!(normalize("2 eggs"), "2 egg");
    assert_eq!(normalize("rotis idlis"), "roti idli");
    assert_eq!(normalize("tomatoes"), "tomato");
    assert_eq!(normalize("hummus, glass"), "hummus glass");
    assert_eq!(normalize("  shallow-fried  "), "shallow fried");
    assert_eq!(normalize("berries"), "berry");
    assert_eq!(normalize(""), "");
}

#[test]
fn csv_handles_quotes_and_escapes() {
    assert_eq!(
        csv("a,\"b, c\",\"say \"\"hi\"\"\"\n\nx,,y\n"),
        vec![vec!["a", "b, c", "say \"hi\""], vec!["x", "", "y"]]
    );
}

#[test]
fn score_is_token_overlap() {
    assert_eq!(score("rice white cooked", "cooked white rice"), 1.0);
    assert_eq!(score("chicken", "chicken curry"), 0.75);
    assert!(score("paneer", "paneer") - score("paneer", "palak paneer") >= MARGIN);
    assert_eq!(score("pizza", "rice"), 0.0);
}

#[test]
fn classify_matched_needs_a_high_score_and_a_margin() {
    assert_eq!(
        classify(vec![(1.0, true, "a"), (0.5, true, "b")]),
        Match::Matched("a")
    );
    assert_eq!(
        classify(vec![(0.7, true, "a"), (0.65, true, "b")]),
        Match::Ambiguous(vec!["a", "b"])
    );
}

#[test]
fn classify_between_thresholds_is_ambiguous_with_at_most_three() {
    let c = vec![
        (0.5, true, "a"),
        (0.5, true, "b"),
        (0.4, true, "c"),
        (0.35, true, "d"),
        (0.1, true, "e"),
    ];
    assert_eq!(classify(c), Match::Ambiguous(vec!["a", "b", "c"]));
}

#[test]
fn classify_low_or_empty_is_unmatched() {
    assert_eq!(classify(vec![(0.2, true, "a")]), Match::Unmatched);
    assert_eq!(classify::<&str>(vec![]), Match::Unmatched);
}

#[test]
fn classify_prefers_staples_over_equal_usda_entries() {
    assert_eq!(
        classify(vec![(1.0, false, "usda"), (1.0, true, "staple")]),
        Match::Matched("staple")
    );
}

#[test]
fn shipped_data_parses() {
    let staples = csv(STAPLES);
    assert_eq!(staples[0][0], "name");
    for r in &staples[1..] {
        assert_eq!(r.len(), 10, "{r:?}");
        let kcal: f64 = r[2].parse().unwrap();
        assert!((0.0..=900.0).contains(&kcal), "{r:?}");
        assert!(
            r[8].parse::<i64>().is_ok(),
            "every staple cites an FDC id: {r:?}"
        );
    }
    for r in &csv(ALIASES)[1..] {
        assert_eq!(r.len(), 2, "{r:?}");
    }
}

async fn db() -> SqlitePool {
    crate::db::memory().await // seeds
}

#[tokio::test]
async fn seed_is_idempotent() {
    let db = db().await;
    let count = || async {
        sqlx::query_scalar!("SELECT (SELECT count(*) FROM foods) + (SELECT count(*) FROM food_aliases) + (SELECT count(*) FROM foods_fts)")
            .fetch_one(&db)
            .await
            .unwrap()
    };
    let before = count().await;
    seed(&db).await.unwrap();
    assert_eq!(count().await, before);
}

#[tokio::test]
async fn find_uses_aliases_then_search() {
    let db = db().await;
    let name = |m: Match| match m {
        Match::Matched(f) => f.name,
        other => panic!("{other:?}"),
    };
    assert_eq!(
        name(find(&db, 0, "chapati").await.unwrap()),
        "Roti (chapati), whole wheat"
    );
    assert_eq!(
        name(find(&db, 0, &normalize("cooked white rice")).await.unwrap()),
        "Rice, white, cooked"
    );
    assert!(matches!(find(&db, 0, "chicken").await.unwrap(), Match::Ambiguous(c) if c.len() > 1));
    assert_eq!(find(&db, 0, "xyzzy").await.unwrap(), Match::Unmatched);
}
