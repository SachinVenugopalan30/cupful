// SPDX-License-Identifier: AGPL-3.0-or-later
//! The saved-products list and what a synced product item does to it.
use axum::http::StatusCode;
use serde_json::{Value, json};

use crate::common::*;
use crate::{BC, label, lookup, setup};

fn product_item(food_id: &Value, grams: f64) -> Value {
    let mut i = item(&uuid(), 162.0);
    i["food_id"] = food_id.clone();
    i["grams"] = json!(grams);
    i["origin"] = json!("food_db");
    i
}

#[tokio::test]
async fn saved_products_most_recent_first_and_removable() {
    let (_, _, state, mut a) = setup(false).await;
    let (_, one) = a.post("/api/products", label(BC, "One")).await;
    let (_, two) = a.post("/api/products", label("96385074", "Two")).await;
    sqlx::query("UPDATE user_products SET last_used_at = last_used_at - 1000 WHERE food_id = ?")
        .bind(one["food_id"].as_i64())
        .execute(&state.db)
        .await
        .unwrap();
    let (_, list) = a.get("/api/products").await;
    let ids: Vec<_> = list["products"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["food_id"].clone())
        .collect();
    assert_eq!(ids, [two["food_id"].clone(), one["food_id"].clone()]);

    let uri = format!("/api/products/{}", two["food_id"]);
    let (status, _) = a.req("DELETE", &uri, None).await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = a.req("DELETE", &uri, None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (_, list) = a.get("/api/products").await;
    assert_eq!(list["products"].as_array().unwrap().len(), 1);
    // The shared food stays.
    let (status, p) = lookup(&mut a, "96385074").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(p["food_id"], two["food_id"]);
}

#[tokio::test]
async fn syncing_a_product_item_updates_saved_products() {
    let (_, _, state, mut a) = setup(false).await;
    let (_, p) = a.post("/api/products", label(BC, "Parle-G")).await;
    sqlx::query("DELETE FROM user_products")
        .execute(&state.db)
        .await
        .unwrap();
    let d = today("Asia/Kolkata").to_string();
    let res = a
        .sync(vec![op(
            "meal.upsert",
            &at(0),
            meal(&uuid(), &d, json!([product_item(&p["food_id"], 36.0)])),
        )])
        .await;
    assert_eq!(res, ["applied"]);
    let (_, list) = a.get("/api/products").await;
    assert_eq!(list["products"][0]["food_id"], p["food_id"]);
    assert_eq!(list["products"][0]["last_servings"], 3.0);
}

#[tokio::test]
async fn syncing_an_unknown_food_is_rejected() {
    let (_, _, _, mut a) = setup(false).await;
    let d = today("Asia/Kolkata").to_string();
    let res = a
        .sync(vec![op(
            "meal.upsert",
            &at(0),
            meal(&uuid(), &d, json!([product_item(&json!(999), 36.0)])),
        )])
        .await;
    assert_eq!(res, ["rejected"]);
}
