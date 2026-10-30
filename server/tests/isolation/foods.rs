// SPDX-License-Identifier: AGPL-3.0-or-later
//! Private foods, aliases and portions (§8).
use axum::http::StatusCode;
use serde_json::json;

use crate::common::*;
use crate::{id_of, matched};

#[tokio::test]
async fn b_never_matches_a_private_foods_aliases_or_portions() {
    let (app, state) = app().await;
    let mut a = admin(&app).await;
    let mut b = user(&app, &state, "bob").await;
    let (_, food) = a
        .post(
            "/api/foods",
            json!({"name": "Zebra cake", "kcal": 400, "protein_g": 5, "carbs_g": 60, "fat_g": 15}),
        )
        .await;
    let food_id = food["id"].as_i64().unwrap();
    let rice: i64 = sqlx::query_scalar("SELECT id FROM foods WHERE name = 'Rice, white, cooked'")
        .fetch_one(&state.db)
        .await
        .unwrap();
    let (s1, _) = a
        .post("/api/aliases", json!({"alias": "zc", "food_id": food_id}))
        .await;
    let (s2, _) = a
        .post("/api/aliases", json!({"alias": "dinner", "food_id": rice}))
        .await;
    let (s3, _) = a
        .post(
            "/api/portions",
            json!({"phrase": "my cake", "food_id": food_id, "grams": 80}),
        )
        .await;
    assert_eq!([s1, s2, s3], [StatusCode::OK; 3]);
    let (ida, idb) = (id_of(&state, "admin").await, id_of(&state, "bob").await);
    for food in ["zebra cake", "zc", "my cake"] {
        assert_eq!(
            matched(&state.db, ida, food).await,
            Some(food_id),
            "A: {food}"
        );
    }
    assert_eq!(matched(&state.db, ida, "dinner").await, Some(rice));
    // B may match shipped foods (e.g. "zebra cake" → a cake staple), never A's private food or A's alias target.
    for food in ["zebra cake", "zc", "my cake"] {
        assert_ne!(
            matched(&state.db, idb, food).await,
            Some(food_id),
            "B: {food}"
        );
    }
    assert_ne!(
        matched(&state.db, idb, "dinner").await,
        Some(rice),
        "B: dinner"
    );
    // B can't alias, or save a portion of, A's private food.
    let (status, _) = b
        .post("/api/aliases", json!({"alias": "mine", "food_id": food_id}))
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = b
        .post(
            "/api/portions",
            json!({"phrase": "mine", "food_id": food_id, "grams": 50}),
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    // B's own food and alias with A's names don't change what A matches.
    let (_, bfood) = b
        .post(
            "/api/foods",
            json!({"name": "Zebra cake", "kcal": 100, "protein_g": 1, "carbs_g": 20, "fat_g": 1}),
        )
        .await;
    b.post("/api/aliases", json!({"alias": "zc", "food_id": rice}))
        .await;
    assert_eq!(matched(&state.db, ida, "zc").await, Some(food_id));
    assert_eq!(matched(&state.db, ida, "zebra cake").await, Some(food_id));
    assert_eq!(
        matched(&state.db, idb, "zebra cake").await,
        bfood["id"].as_i64()
    );
}

#[tokio::test]
async fn b_cannot_list_or_delete_a_aliases() {
    let (app, state) = app().await;
    let mut a = admin(&app).await;
    let mut b = user(&app, &state, "bob").await;
    let rice: i64 = sqlx::query_scalar("SELECT id FROM foods WHERE name = 'Rice, white, cooked'")
        .fetch_one(&state.db)
        .await
        .unwrap();
    let (_, al) = a
        .post("/api/aliases", json!({"alias": "dinner", "food_id": rice}))
        .await;
    let (status, list) = b.get("/api/aliases").await;
    assert_eq!((status, &list["aliases"]), (StatusCode::OK, &json!([])));
    let (status, _) = b
        .req("DELETE", &format!("/api/aliases/{}", al["id"]), None)
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (_, list) = a.get("/api/aliases").await;
    assert_eq!(list["aliases"].as_array().unwrap().len(), 1);
    assert_eq!(list["aliases"][0]["food_name"], "Rice, white, cooked");
    assert_eq!(list["aliases"][0]["source"], "staple");
    assert!(list["aliases"][0]["brand"].is_null());
    let (status, _) = a
        .req("DELETE", &format!("/api/aliases/{}", al["id"]), None)
        .await;
    assert_eq!(status, StatusCode::OK);
    let (_, list) = a.get("/api/aliases").await;
    assert_eq!(list["aliases"], json!([]));
}

#[tokio::test]
async fn b_cannot_find_or_log_a_private_food() {
    let (app, state) = app().await;
    let mut a = admin(&app).await;
    let mut b = user(&app, &state, "bob").await;
    let food_id: i64 = sqlx::query_scalar(
        "INSERT INTO foods (source, source_ref, user_id, name, name_norm, barcode, kcal, protein_g,
             carbs_g, fat_g)
         SELECT 'user', id, id, 'secret', 'secret', '96385074', 100, 1, 1, 1
         FROM users WHERE username = 'admin' RETURNING id",
    )
    .fetch_one(&state.db)
    .await
    .unwrap();
    let (status, _) = b.get("/api/products/96385074").await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let d = today("Asia/Kolkata").to_string();
    let mut it = item(&uuid(), 100.0);
    it["food_id"] = json!(food_id);
    let o = || {
        op(
            "meal.upsert",
            &at(0),
            meal(&uuid(), &d, json!([it.clone()])),
        )
    };
    assert_eq!(b.sync(vec![o()]).await, ["rejected"]);
    let (_, list) = b.get("/api/products").await;
    assert_eq!(list["products"], json!([]));
    assert_eq!(a.sync(vec![o()]).await, ["applied"]);
}
