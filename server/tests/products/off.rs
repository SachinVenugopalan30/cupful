// SPDX-License-Identifier: AGPL-3.0-or-later
//! Open Food Facts lookups: mapping, caching, retries, validation and rate limits.
use std::time::Duration;

use axum::http::StatusCode;
use wiremock::matchers::header_regex;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, ResponseTemplate};

use crate::common::*;
use crate::{BC, PATH, found, lookup, nutriments, off_returns, parle, portions, setup};

const DAY_MS: i64 = 24 * 60 * 60 * 1000;

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
    assert_eq!(p["serving_unit"], "g");
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
        .respond_with(ResponseTemplate::new(404).set_body_json(
            serde_json::json!({"code": BC, "status": 0, "status_verbose": "product not found"}),
        ))
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
async fn millilitre_servings_are_kept() {
    let (off, _, _, mut a) = setup(true).await;
    let mut p = parle(nutriments());
    p["product_name"] = serde_json::json!("Milk");
    p["serving_size"] = serde_json::json!("1 cup (240 ml)");
    p["serving_quantity"] = serde_json::json!(240);
    p["serving_quantity_unit"] = serde_json::json!("ml");
    off_returns(p).mount(&off).await;
    let (_, p) = lookup(&mut a, BC).await;
    assert_eq!(p["serving_g"], 240.0);
    assert_eq!(p["serving_unit"], "ml");
    assert_eq!(p["serving_text"], "1 cup (240 ml)");
}

#[tokio::test]
async fn old_liquid_rows_are_refetched_once() {
    let (off, _, state, mut a) = setup(true).await;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64;
    sqlx::query(
        "INSERT INTO foods (source, source_ref, name, name_norm, barcode, kcal, protein_g, carbs_g,
             fat_g, serving_text, fetched_at)
         VALUES ('off', ?, 'Milk', 'milk', ?, 42, 3.4, 5, 1, '1 cup (240 ml)', ?)",
    )
    .bind(BC)
    .bind(BC)
    .bind(now)
    .execute(&state.db)
    .await
    .unwrap();
    let mut p = parle(nutriments());
    p["product_name"] = serde_json::json!("Milk");
    p["serving_size"] = serde_json::json!("1 cup (240 ml)");
    p["serving_quantity"] = serde_json::json!(240);
    p["serving_quantity_unit"] = serde_json::json!("ml");
    off_returns(p).expect(1).mount(&off).await;

    let (status, p) = lookup(&mut a, BC).await;
    assert_eq!(status, StatusCode::OK, "{p}");
    assert_eq!(p["serving_g"], 240.0);
    assert_eq!(p["serving_unit"], "ml");
}

#[tokio::test]
async fn litre_servings_become_millilitres() {
    let (off, _, _, mut a) = setup(true).await;
    let mut p = parle(nutriments());
    p["product_name"] = serde_json::json!("Water");
    p["serving_size"] = serde_json::json!("1 L");
    p["serving_quantity"] = serde_json::json!(1);
    p["serving_quantity_unit"] = serde_json::json!("l");
    off_returns(p).mount(&off).await;
    let (_, p) = lookup(&mut a, BC).await;
    assert_eq!(p["serving_g"], 1000.0);
    assert_eq!(p["serving_unit"], "ml");
    assert_eq!(p["serving_text"], "1 L");
}

#[tokio::test]
async fn missing_serving_text_gets_the_volume() {
    let (off, _, _, mut a) = setup(true).await;
    let mut p = parle(nutriments());
    p.as_object_mut().unwrap().remove("serving_size");
    p["serving_quantity"] = serde_json::json!(330);
    p["serving_quantity_unit"] = serde_json::json!("ml");
    off_returns(p).mount(&off).await;
    let (_, p) = lookup(&mut a, BC).await;
    assert_eq!(p["serving_g"], 330.0);
    assert_eq!(p["serving_unit"], "ml");
    assert_eq!(p["serving_text"], "330 ml");
}

#[tokio::test]
async fn missing_serving() {
    let (off, _, _, mut a) = setup(true).await;
    off_returns(serde_json::json!({"product_name": "Parle-G", "nutriments": nutriments()}))
        .mount(&off)
        .await;
    let (status, p) = lookup(&mut a, BC).await;
    assert_eq!(status, StatusCode::OK, "{p}");
    assert_eq!(p["serving_g"], serde_json::json!(null));
    assert_eq!(p["serving_text"], serde_json::json!(null));
    assert_eq!(p["brand"], serde_json::json!(null));
}

#[tokio::test]
async fn implausible_or_incomplete_values_are_not_found() {
    let mut too_much = nutriments();
    too_much["energy-kcal_100g"] = serde_json::json!(1200);
    let mut no_fat = nutriments();
    no_fat.as_object_mut().unwrap().remove("fat_100g");
    let mut negative = nutriments();
    negative["proteins_100g"] = serde_json::json!(-1);
    for n in [too_much, no_fat, negative, serde_json::json!({})] {
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

async fn age_off_rows(state: &cupful::AppState, days: i64) {
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
    renamed["product_name"] = serde_json::json!("Parle-G Gold");
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

#[tokio::test]
async fn open_food_facts_servings_become_portions() {
    let (off, _, state, mut a) = setup(true).await;
    off_returns(parle(nutriments())).mount(&off).await;
    let (_, p) = lookup(&mut a, BC).await;
    assert_eq!(
        portions(&state, p["food_id"].as_i64().unwrap()).await,
        [("biscuit".into(), 12.0), ("serving".into(), 12.0)]
    );
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

    let (status, body) = a.post("/api/products", crate::label(BC, "Parle-G")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, p) = lookup(&mut a, BC).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(p["source"], "user");
    assert_eq!(p["attribution"], serde_json::json!(null));
}

#[tokio::test]
async fn invalid_barcodes_are_rejected() {
    let (_, _, _, mut a) = setup(false).await;
    for code in ["4006381333932", "1234", "abcdefgh"] {
        let (status, _) = lookup(&mut a, code).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{code}");
    }
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
    a.post("/api/products", crate::label(BC, "Parle-G")).await;
    assert_eq!(lookup(&mut a, BC).await.0, StatusCode::OK);
    let mut b = user(&app, &state, "bob").await;
    assert_eq!(lookup(&mut b, &ean8(n)).await.0, StatusCode::NOT_FOUND);
}
