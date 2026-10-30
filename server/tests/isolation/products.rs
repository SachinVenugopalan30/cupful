// SPDX-License-Identifier: AGPL-3.0-or-later
//! Saved products and label entries.
use axum::http::StatusCode;
use serde_json::json;

use crate::common::*;
use crate::label;

#[tokio::test]
async fn b_cannot_see_change_or_delete_a_saved_products() {
    let (app, state) = app().await;
    let mut a = admin(&app).await;
    let mut b = user(&app, &state, "bob").await;
    let (_, p) = a.post("/api/products", label("A's biscuit")).await;
    let (_, list) = b.get("/api/products").await;
    assert_eq!(list["products"], json!([]));
    let (status, _) = b
        .req("DELETE", &format!("/api/products/{}", p["food_id"]), None)
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    // B's own label entry for the same barcode is a new row; A's entry and list are unchanged.
    let (_, bobs) = b.post("/api/products", label("B's biscuit")).await;
    assert_ne!(bobs["food_id"], p["food_id"]);
    let (_, list) = a.get("/api/products").await;
    assert_eq!(list["products"].as_array().unwrap().len(), 1);
    assert_eq!(list["products"][0]["name"], "A's biscuit");
    let (_, found) = a.get("/api/products/96385074").await;
    assert_eq!(found["name"], "A's biscuit");
}
