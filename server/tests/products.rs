// SPDX-License-Identifier: AGPL-3.0-or-later
//! Barcode lookups against a mocked Open Food Facts, label entry and saved products (PLAN.md §8).
mod common;
use std::time::Duration;

use axum::Router;
use axum::http::StatusCode;
use common::*;
use cupful::{AppState, Config};
use serde_json::{Value, json};
use wiremock::matchers::{header_regex, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

const BC: &str = "4006381333931";
const PATH: &str = "/api/v2/product/4006381333931";
const DAY_MS: i64 = 24 * 60 * 60 * 1000;

async fn setup(lookup: bool) -> (MockServer, Router, AppState, Client) {
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

fn nutriments() -> Value {
    json!({"energy-kcal_100g": 450, "energy-kj_100g": 1883, "proteins_100g": 7,
           "carbohydrates_100g": 76, "fat_100g": 13.5})
}

fn found(product: Value) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_json(
        json!({"code": BC, "status": 1, "status_verbose": "product found", "product": product}),
    )
}

fn parle(nutriments: Value) -> Value {
    json!({"product_name": "Parle-G", "brands": "Parle, Parle Products",
           "serving_size": "1 biscuit (12 g)", "serving_quantity": 12,
           "serving_quantity_unit": "g", "nutriments": nutriments})
}

fn off_returns(product: Value) -> Mock {
    Mock::given(method("GET"))
        .and(path(PATH))
        .respond_with(found(product))
}

async fn lookup(c: &mut Client, code: &str) -> (StatusCode, Value) {
    c.get(&format!("/api/products/{code}")).await
}

#[tokio::test]
async fn hit_is_mapped_stored_and_saved() {
    let (off, _, _, mut a) = setup(true).await;
    Mock::given(method("GET"))
        .and(path(PATH))
        .and(query_param(
            "fields",
            "product_name,brands,nutriments,serving_size,serving_quantity,serving_quantity_unit",
        ))
        .and(header_regex(
            "user-agent",
            r"^Cupful/\d+\.\d+\.\d+ \(\+https://",
        ))
        .respond_with(found(parle(nutriments())))
        .expect(1)
        .mount(&off)
        .await;
    let (status, p) = lookup(&mut a, BC).await;
    assert_eq!(status, StatusCode::OK, "{p}");
    assert_eq!(p["name"], "Parle-G");
    assert_eq!(p["brand"], "Parle");
    assert_eq!(p["barcode"], BC);
    assert_eq!(p["source"], "off");
    assert_eq!(p["kcal"], 450.0);
    assert_eq!(p["protein_g"], 7.0);
    assert_eq!(p["carbs_g"], 76.0);
    assert_eq!(p["fat_g"], 13.5);
    assert_eq!(p["serving_g"], 12.0);
    assert_eq!(p["serving_text"], "1 biscuit (12 g)");
    assert_eq!(
        p["attribution"]["text"],
        "Product data from Open Food Facts (ODbL)"
    );
    assert!(p["food_id"].is_i64());

    // Second lookup is served from the database: `expect(1)` fails on drop otherwise.
    let (status, again) = lookup(&mut a, BC).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(again["food_id"], p["food_id"]);

    let (_, list) = a.get("/api/products").await;
    assert_eq!(list["products"][0]["food_id"], p["food_id"]);
    assert_eq!(list["products"][0]["name"], "Parle-G");
}

#[tokio::test]
async fn miss_is_a_coded_404() {
    let (off, _, _, mut a) = setup(true).await;
    Mock::given(method("GET"))
        .and(path(PATH))
        .respond_with(
            ResponseTemplate::new(404).set_body_json(
                json!({"code": BC, "status": 0, "status_verbose": "product not found"}),
            ),
        )
        .expect(1)
        .mount(&off)
        .await;
    let (status, body) = lookup(&mut a, BC).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["code"], "product_not_found");
}

#[tokio::test]
async fn kj_only_is_converted() {
    let (off, _, _, mut a) = setup(true).await;
    let mut n = nutriments();
    n.as_object_mut().unwrap().remove("energy-kcal_100g");
    off_returns(parle(n)).mount(&off).await;
    let (status, p) = lookup(&mut a, BC).await;
    assert_eq!(status, StatusCode::OK, "{p}");
    assert_eq!(p["kcal"], 450.0); // 1883 / 4.184 = 450.04
}

#[tokio::test]
async fn serving_only_in_grams() {
    let (off, _, _, mut a) = setup(true).await;
    let mut p = parle(nutriments());
    p["serving_quantity_unit"] = json!("ml");
    off_returns(p).mount(&off).await;
    let (_, p) = lookup(&mut a, BC).await;
    assert_eq!(p["serving_g"], json!(null));
    assert_eq!(p["serving_text"], "1 biscuit (12 g)");
}

#[tokio::test]
async fn missing_serving() {
    let (off, _, _, mut a) = setup(true).await;
    off_returns(json!({"product_name": "Parle-G", "nutriments": nutriments()}))
        .mount(&off)
        .await;
    let (status, p) = lookup(&mut a, BC).await;
    assert_eq!(status, StatusCode::OK, "{p}");
    assert_eq!(p["serving_g"], json!(null));
    assert_eq!(p["serving_text"], json!(null));
    assert_eq!(p["brand"], json!(null));
}

#[tokio::test]
async fn implausible_or_incomplete_values_are_not_found() {
    let mut too_much = nutriments();
    too_much["energy-kcal_100g"] = json!(1200);
    let mut no_fat = nutriments();
    no_fat.as_object_mut().unwrap().remove("fat_100g");
    let mut negative = nutriments();
    negative["proteins_100g"] = json!(-1);
    for n in [too_much, no_fat, negative, json!({})] {
        let (off, _, _, mut a) = setup(true).await;
        off_returns(parle(n.clone())).mount(&off).await;
        let (status, body) = lookup(&mut a, BC).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{n}");
        assert_eq!(body["code"], "product_not_found");
    }
}

#[tokio::test]
async fn timeout_reads_as_lookup_failed() {
    let (off, _, _, mut a) = setup(true).await;
    Mock::given(method("GET"))
        .and(path(PATH))
        .respond_with(found(parle(nutriments())).set_delay(Duration::from_secs(2)))
        .mount(&off)
        .await;
    let (status, body) = lookup(&mut a, BC).await;
    assert_eq!(status, StatusCode::BAD_GATEWAY);
    assert_eq!(body["code"], "lookup_failed");
}

#[tokio::test]
async fn one_retry_on_5xx() {
    let (off, _, _, mut a) = setup(true).await;
    Mock::given(method("GET"))
        .and(path(PATH))
        .respond_with(ResponseTemplate::new(503))
        .up_to_n_times(1)
        .expect(1)
        .mount(&off)
        .await;
    off_returns(parle(nutriments())).mount(&off).await;
    let (status, p) = lookup(&mut a, BC).await;
    assert_eq!(status, StatusCode::OK, "{p}");
}

#[tokio::test]
async fn persistent_5xx_fails_after_two_requests() {
    let (off, _, _, mut a) = setup(true).await;
    Mock::given(method("GET"))
        .and(path(PATH))
        .respond_with(ResponseTemplate::new(500))
        .expect(2)
        .mount(&off)
        .await;
    let (status, body) = lookup(&mut a, BC).await;
    assert_eq!(status, StatusCode::BAD_GATEWAY);
    assert_eq!(body["code"], "lookup_failed");
}

#[tokio::test]
async fn no_retry_on_4xx() {
    let (off, _, _, mut a) = setup(true).await;
    Mock::given(method("GET"))
        .and(path(PATH))
        .respond_with(ResponseTemplate::new(429))
        .expect(1)
        .mount(&off)
        .await;
    let (_, body) = lookup(&mut a, BC).await;
    assert_eq!(body["code"], "lookup_failed");
}

async fn age_off_rows(state: &AppState, days: i64) {
    sqlx::query("UPDATE foods SET fetched_at = fetched_at - ? WHERE source = 'off'")
        .bind(days * DAY_MS)
        .execute(&state.db)
        .await
        .unwrap();
}

#[tokio::test]
async fn refetches_after_90_days() {
    let (off, _, state, mut a) = setup(true).await;
    off_returns(parle(nutriments()))
        .up_to_n_times(1)
        .mount(&off)
        .await;
    let mut renamed = parle(nutriments());
    renamed["product_name"] = json!("Parle-G Gold");
    off_returns(renamed).mount(&off).await;
    let (_, first) = lookup(&mut a, BC).await;

    age_off_rows(&state, 89).await;
    let (_, p) = lookup(&mut a, BC).await;
    assert_eq!(p["name"], "Parle-G", "still fresh");

    age_off_rows(&state, 2).await;
    let (_, p) = lookup(&mut a, BC).await;
    assert_eq!(p["name"], "Parle-G Gold");
    assert_eq!(p["food_id"], first["food_id"], "same row, updated");
    assert_eq!(off.received_requests().await.unwrap().len(), 2);
}

#[tokio::test]
async fn failed_refetch_keeps_the_stored_copy() {
    let (off, _, state, mut a) = setup(true).await;
    off_returns(parle(nutriments()))
        .up_to_n_times(1)
        .mount(&off)
        .await;
    Mock::given(method("GET"))
        .and(path(PATH))
        .respond_with(ResponseTemplate::new(500))
        .mount(&off)
        .await;
    lookup(&mut a, BC).await;
    age_off_rows(&state, 91).await;
    let (status, p) = lookup(&mut a, BC).await;
    assert_eq!(status, StatusCode::OK, "{p}");
    assert_eq!(p["name"], "Parle-G");
}

fn label(barcode: &str, name: &str) -> Value {
    json!({"name": name, "brand": "Parle", "barcode": barcode, "basis": "100g",
           "kcal": 460.0, "protein_g": 6.5, "carbs_g": 77.0, "fat_g": 14.0,
           "serving_g": 12.0, "serving_text": "1 biscuit"})
}

#[tokio::test]
async fn lookup_off_makes_no_request_but_finds_stored_products() {
    let (off, _, _, mut a) = setup(false).await;
    Mock::given(method("GET"))
        .respond_with(found(parle(nutriments())))
        .expect(0)
        .mount(&off)
        .await;
    let (status, body) = lookup(&mut a, BC).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["code"], "product_not_found");

    let (status, body) = a.post("/api/products", label(BC, "Parle-G")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, p) = lookup(&mut a, BC).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(p["source"], "user");
    assert_eq!(p["attribution"], json!(null));
}

#[tokio::test]
async fn invalid_barcodes_are_rejected() {
    let (_, _, _, mut a) = setup(false).await;
    for code in ["4006381333932", "1234", "abcdefgh"] {
        let (status, _) = lookup(&mut a, code).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{code}");
    }
}

#[tokio::test]
async fn label_entry_per_serving_is_converted_and_shared() {
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

    // Shared on this instance: another user's scan finds it.
    let mut b = user(&app, &state, "bob").await;
    let (status, found) = lookup(&mut b, BC).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(found["food_id"], p["food_id"]);
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
async fn own_label_entry_beats_others_and_off() {
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
    // A third user has no entry of their own: a shared label entry still beats Open Food Facts.
    let mut c = user(&app, &state, "carol").await;
    let (_, p) = lookup(&mut c, BC).await;
    assert_eq!(p["source"], "user");
    assert_eq!(off.received_requests().await.unwrap().len(), 1);
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

fn product_item(food_id: &Value, grams: f64) -> Value {
    let mut i = item(&uuid(), 162.0);
    i["food_id"] = food_id.clone();
    i["grams"] = json!(grams);
    i["origin"] = json!("food_db");
    i
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

/// Smallest 8-digit code after `n` with a valid check digit.
fn ean8(n: u32) -> String {
    (n..)
        .map(|n| format!("{n:08}"))
        .find(|c| cupful::products::gtin_ok(c))
        .unwrap()
}

#[tokio::test]
async fn outbound_lookups_are_rate_limited_per_user() {
    let (off, app, state, mut a) = setup(true).await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&off)
        .await;
    let mut n = 10_000_000;
    for _ in 0..60 {
        let code = ean8(n);
        n = code.parse::<u32>().unwrap() + 1;
        assert_eq!(lookup(&mut a, &code).await.0, StatusCode::NOT_FOUND);
    }
    let (status, body) = lookup(&mut a, &ean8(n)).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(body["code"], "rate_limited");
    // Stored products don't count, and other users have their own budget.
    a.post("/api/products", label(BC, "Parle-G")).await;
    assert_eq!(lookup(&mut a, BC).await.0, StatusCode::OK);
    let mut b = user(&app, &state, "bob").await;
    assert_eq!(lookup(&mut b, &ean8(n)).await.0, StatusCode::NOT_FOUND);
}
