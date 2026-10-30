// SPDX-License-Identifier: AGPL-3.0-or-later
//! Label entry: validation, per-serving conversion, portions and privacy.
use axum::http::StatusCode;
use serde_json::json;

use crate::common::*;
use crate::{BC, label, lookup, nutriments, off_returns, parle, portions, setup};

#[tokio::test]
async fn product_servings_become_portions_the_estimate_uses() {
    let (_, _, state, mut a) = setup(false).await;
    let mut body = label("96385074", "Chocolate protein powder");
    body["kcal"] = json!(400.0);
    body["serving_g"] = json!(31.0);
    body["serving_text"] = json!("1 scoop (31 g)");
    let (_, p) = a.post("/api/products", body.clone()).await;
    let id = p["food_id"].as_i64().unwrap();
    assert_eq!(
        portions(&state, id).await,
        [("scoop".into(), 31.0), ("serving".into(), 31.0)]
    );
    let default: f64 = sqlx::query_scalar("SELECT default_portion_g FROM foods WHERE id = ?")
        .bind(id)
        .fetch_one(&state.db)
        .await
        .unwrap();
    assert_eq!(default, 31.0);

    let (status, _) = a
        .post(
            "/api/aliases",
            json!({"alias": "chocolate protein powder", "food_id": id}),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let uid: i64 = sqlx::query_scalar("SELECT id FROM users WHERE username = 'admin'")
        .fetch_one(&state.db)
        .await
        .unwrap();
    let reply = json!({"items": [{"food": "chocolate protein powder", "amount": 1, "unit": "scoop",
        "grams": 15, "cooking": "none", "mentions_oil": false, "estimate": null}],
        "assumptions": null, "confidence": "high"});
    let est = cupful::nutrition::estimate(&state.db, uid, &FixedReply(reply), "a scoop", vec![])
        .await
        .unwrap();
    let it = &est.items[0];
    assert_eq!((it.food_id, it.grams), (Some(id), 31.0));
    assert!((it.kcal.unwrap() - 124.0).abs() < 0.5, "{:?}", it.kcal);

    // Re-entering the label replaces the portions; no serving leaves none.
    body["serving_text"] = json!("2 biscuits (24 g)");
    body["serving_g"] = json!(24.0);
    a.post("/api/products", body.clone()).await;
    assert_eq!(
        portions(&state, id).await,
        [("biscuit".into(), 12.0), ("serving".into(), 24.0)]
    );
    body["serving_g"] = json!(null);
    a.post("/api/products", body).await;
    assert!(portions(&state, id).await.is_empty());
}

#[tokio::test]
async fn label_entry_per_serving_is_converted_and_private() {
    let (_, app, state, mut a) = setup(false).await;
    let (status, p) = a
        .post(
            "/api/products",
            json!({"name": "  Parle-G  ", "brand": null, "barcode": BC, "basis": "serving",
                   "kcal": 54.0, "protein_g": 0.9, "carbs_g": 9.0, "fat_g": 1.5,
                   "serving_g": 12.0, "serving_text": "1 biscuit"}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{p}");
    assert_eq!(p["name"], "Parle-G");
    assert_eq!(p["kcal"], 450.0);
    assert_eq!(p["protein_g"], 7.5);
    assert_eq!(p["carbs_g"], 75.0);
    assert_eq!(p["fat_g"], 12.5);
    assert_eq!(p["serving_g"], 12.0);
    assert_eq!(p["source"], "user");

    let (_, list) = a.get("/api/products").await;
    assert_eq!(list["products"][0]["food_id"], p["food_id"]);

    // Private: another user's scan does not find it.
    let mut b = user(&app, &state, "bob").await;
    let (status, _) = lookup(&mut b, BC).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let id = p["food_id"].as_i64().unwrap();
    let bob = sqlx::query_scalar::<_, i64>("SELECT id FROM users WHERE username = 'bob'")
        .fetch_one(&state.db)
        .await
        .unwrap();
    assert!(
        cupful::nutrition::foods::visible(&state.db, bob, id)
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn label_entry_is_validated() {
    let (_, _, _, mut a) = setup(false).await;
    let bad = [
        ("name", json!("")),
        ("name", json!("x".repeat(201))),
        ("barcode", json!("4006381333932")),
        ("basis", json!("cup")),
        ("kcal", json!(950.0)),
        ("kcal", json!(-1.0)),
        ("protein_g", json!(101.0)),
        ("serving_g", json!(0.0)),
        ("serving_text", json!("x".repeat(101))),
    ];
    for (field, value) in bad {
        let mut body = label(BC, "Parle-G");
        body[field] = value.clone();
        let (status, _) = a.post("/api/products", body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{field} = {value}");
    }
    // Per serving needs the serving's grams; 200 kcal in a 12 g serving is 1667 kcal/100 g.
    let mut body = label(BC, "Parle-G");
    body["basis"] = json!("serving");
    body["serving_g"] = json!(null);
    assert_eq!(
        a.post("/api/products", body).await.0,
        StatusCode::BAD_REQUEST
    );
    let mut body = label(BC, "Parle-G");
    body["basis"] = json!("serving");
    body["kcal"] = json!(200.0);
    assert_eq!(
        a.post("/api/products", body).await.0,
        StatusCode::BAD_REQUEST
    );
}

#[tokio::test]
async fn own_label_entry_beats_off_and_others_stay_private() {
    let (off, app, state, mut a) = setup(true).await;
    off_returns(parle(nutriments())).mount(&off).await;
    let (_, from_off) = lookup(&mut a, BC).await;
    assert_eq!(from_off["source"], "off");

    let mut b = user(&app, &state, "bob").await;
    let (_, bobs) = b.post("/api/products", label(BC, "Bob's Parle")).await;
    let (_, mine) = a.post("/api/products", label(BC, "My Parle")).await;

    let (_, p) = lookup(&mut a, BC).await;
    assert_eq!(p["food_id"], mine["food_id"]);
    let (_, p) = lookup(&mut b, BC).await;
    assert_eq!(p["food_id"], bobs["food_id"]);
    // A third user has no entry of their own: they get Open Food Facts, never someone's label.
    let mut c = user(&app, &state, "carol").await;
    let (_, p) = lookup(&mut c, BC).await;
    assert_eq!(p["food_id"], from_off["food_id"]);
    assert_eq!(p["source"], "off");
}

#[tokio::test]
async fn re_entering_a_label_updates_the_users_entry() {
    let (_, _, _, mut a) = setup(false).await;
    let (_, first) = a.post("/api/products", label(BC, "Parle")).await;
    let (_, second) = a.post("/api/products", label(BC, "Parle-G")).await;
    assert_eq!(first["food_id"], second["food_id"]);
    let (_, list) = a.get("/api/products").await;
    assert_eq!(list["products"].as_array().unwrap().len(), 1);
    assert_eq!(list["products"][0]["name"], "Parle-G");
}

#[tokio::test]
async fn any_count_of_a_product_without_its_own_unit_is_a_serving() {
    // Label entry has no serving text, so "scoop" isn't a portion; a scoop is still one serving,
    // but a spoon stays a spoon.
    let (_, _, state, mut a) = setup(false).await;
    let mut body = label("96385074", "Chocolate protein powder");
    body["kcal"] = json!(400.0);
    body["serving_g"] = json!(31.0);
    body["serving_text"] = json!(null);
    let (_, p) = a.post("/api/products", body).await;
    let id = p["food_id"].as_i64().unwrap();
    a.post(
        "/api/aliases",
        json!({"alias": "chocolate protein powder", "food_id": id}),
    )
    .await;
    let uid: i64 = sqlx::query_scalar("SELECT id FROM users WHERE username = 'admin'")
        .fetch_one(&state.db)
        .await
        .unwrap();
    for (unit, grams) in [("scoop", 31.0), ("tbsp", 15.0)] {
        let reply = json!({"items": [{"food": "chocolate protein powder", "amount": 1, "unit": unit,
            "grams": 15, "cooking": "none", "mentions_oil": false, "estimate": null}],
            "assumptions": null, "confidence": "high"});
        let est =
            cupful::nutrition::estimate(&state.db, uid, &FixedReply(reply), "a scoop", vec![])
                .await
                .unwrap();
        assert_eq!(
            (est.items[0].food_id, est.items[0].grams),
            (Some(id), grams),
            "{unit}"
        );
    }
}
