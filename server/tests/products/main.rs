// SPDX-License-Identifier: AGPL-3.0-or-later
//! Barcode lookups against a mocked Open Food Facts, label entry and saved products (PLAN.md §8).
/// Shared helpers live at tests/common/mod.rs, outside this target directory.
#[path = "../common/mod.rs"]
mod common;

mod label;
mod off;
mod saved;

use std::time::Duration;

use axum::Router;
use axum::http::StatusCode;
use common::*;
use cupful::{AppState, Config};
use serde_json::{Value, json};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

pub(crate) const BC: &str = "4006381333931";
pub(crate) const PATH: &str = "/api/v2/product/4006381333931";

/// A mocked Open Food Facts plus a logged-in admin. `lookup` turns the real client on.
pub(crate) async fn setup(lookup: bool) -> (MockServer, Router, AppState, Client) {
    let off = MockServer::start().await;
    let (app, state) = app_with(Config {
        product_lookup: lookup,
        off_base_url: off.uri(),
        off_timeout: Duration::from_millis(300),
        ..Config::default()
    })
    .await;
    let a = admin(&app).await;
    (off, app, state, a)
}

pub(crate) fn nutriments() -> Value {
    json!({"energy-kcal_100g": 450, "energy-kj_100g": 1883, "proteins_100g": 7,
           "carbohydrates_100g": 76, "fat_100g": 13.5})
}

pub(crate) fn found(product: Value) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_json(
        json!({"code": BC, "status": 1, "status_verbose": "product found", "product": product}),
    )
}

pub(crate) fn parle(nutriments: Value) -> Value {
    json!({"product_name": "Parle-G", "brands": "Parle, Parle Products",
           "serving_size": "1 biscuit (12 g)", "serving_quantity": 12,
           "serving_quantity_unit": "g", "nutriments": nutriments})
}

pub(crate) fn off_returns(product: Value) -> Mock {
    Mock::given(method("GET"))
        .and(path(PATH))
        .respond_with(found(product))
}

pub(crate) async fn lookup(c: &mut Client, code: &str) -> (StatusCode, Value) {
    c.get(&format!("/api/products/{code}")).await
}

pub(crate) fn label(barcode: &str, name: &str) -> Value {
    json!({"name": name, "brand": "Parle", "barcode": barcode, "basis": "100g",
           "kcal": 460.0, "protein_g": 6.5, "carbs_g": 77.0, "fat_g": 14.0,
           "serving_g": 12.0, "serving_text": "1 biscuit"})
}

pub(crate) async fn portions(state: &AppState, food_id: i64) -> Vec<(String, f64)> {
    sqlx::query_as("SELECT unit, grams FROM food_portions WHERE food_id = ? ORDER BY unit")
        .bind(food_id)
        .fetch_all(&state.db)
        .await
        .unwrap()
}
