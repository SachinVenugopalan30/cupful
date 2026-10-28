// SPDX-License-Identifier: AGPL-3.0-or-later
//! Nutrition pipeline: golden cases on recorded replies, `POST /api/estimate`, and saving foods.
mod common;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::*;
use cupful::ai::{Capabilities, JsonRequest, Provider, ProviderError};
use cupful::nutrition::{self, golden};
use serde_json::{Value, json};

/// Replies with a fixed recording; no network.
struct Recorded(Value);

#[async_trait::async_trait]
impl Provider for Recorded {
    async fn complete_json(&self, _req: JsonRequest) -> Result<Value, ProviderError> {
        Ok(self.0.clone())
    }
    fn capabilities(&self) -> Capabilities {
        Capabilities::default()
    }
}

#[tokio::test]
async fn golden_cases_pass_on_recorded_replies() {
    let db = cupful::db::memory().await;
    let cases = golden::load(std::path::Path::new(golden::DIR)).unwrap();
    assert!(cases.len() >= 8);
    let mut failures = vec![];
    for (name, case) in &cases {
        for field in [
            &case.recorded.provider,
            &case.recorded.model,
            &case.recorded.recorded_at,
        ] {
            assert!(
                !field.is_empty(),
                "{name}: recording needs provider, model and date"
            );
        }
        let est = nutrition::estimate(
            &db,
            0,
            &Recorded(case.recorded.output.clone()),
            &case.description,
            vec![],
        )
        .await
        .unwrap();
        let (_, _, f) = golden::check(case, &est);
        failures.extend(f.into_iter().map(|f| format!("{name}: {f}")));
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

async fn run(db: &sqlx::SqlitePool, user_id: i64, reply: Value) -> nutrition::Estimate {
    nutrition::estimate(db, user_id, &Recorded(reply), "test", vec![])
        .await
        .unwrap()
}

fn reply(items: Value) -> Value {
    json!({"items": items, "assumptions": "a", "confidence": "high"})
}

fn raw(food: &str, grams: f64, est: Value) -> Value {
    json!({"food": food, "amount": 1, "unit": "", "grams": grams, "cooking": "none", "mentions_oil": false, "estimate": est})
}

#[tokio::test]
async fn matched_items_use_database_values_and_unmatched_use_the_estimate() {
    let db = cupful::db::memory().await;
    let est = run(
        &db,
        0,
        reply(json!([
            raw(
                "rice",
                200.0,
                json!({"kcal": 999, "protein": 1, "carbs": 1, "fat": 1})
            ),
            raw(
                "durian",
                100.0,
                json!({"kcal": 300, "protein": 12, "carbs": 33, "fat": 13})
            ),
            raw(
                "baklava",
                10.0,
                json!({"kcal": 250, "protein": 10, "carbs": 30, "fat": 10})
            ),
        ])),
    )
    .await;
    let v = serde_json::to_value(&est).unwrap();
    // Rice: 130 kcal / 100 g from the staple, not the model's number.
    assert_eq!(v["items"][0]["kcal"], 260.0);
    assert_eq!(v["items"][0]["origin"], "food_db");
    assert_eq!(v["items"][1]["origin"], "ai_estimate");
    assert_eq!(v["items"][1]["kcal"], 300.0);
    // 2500 kcal / 100 g is implausible: the estimate is dropped and the user must edit it.
    assert_eq!(v["items"][2]["kcal"], Value::Null);
    assert_eq!(v["items"][2]["match"], "unmatched");
    assert_eq!(v["confidence"], "high");
}

#[tokio::test]
async fn ambiguous_items_offer_up_to_three_candidates_with_values() {
    let db = cupful::db::memory().await;
    let est = run(&db, 0, reply(json!([raw("fish", 100.0, json!(null))]))).await;
    let item = &est.items[0];
    assert_eq!(item.status, "ambiguous");
    assert!((2..=3).contains(&item.candidates.len()));
    assert!(item.candidates.iter().all(|c| c.kcal > 0.0));
}

#[tokio::test]
async fn too_many_items_are_rejected() {
    let db = cupful::db::memory().await;
    let items: Vec<Value> = (0..16).map(|_| raw("rice", 100.0, json!(null))).collect();
    let err = nutrition::estimate(&db, 0, &Recorded(reply(json!(items))), "x", vec![]).await;
    assert!(err.is_err());
}

#[tokio::test]
async fn reply_that_does_not_fit_the_schema_is_unparseable() {
    let db = cupful::db::memory().await;
    let err = nutrition::estimate(&db, 0, &Recorded(json!({"foods": []})), "x", vec![])
        .await
        .unwrap_err();
    let res = axum::response::IntoResponse::into_response(err);
    assert_eq!(res.status(), StatusCode::BAD_GATEWAY);
}

// ---- cupful import-foods ----

fn nutrient(number: &str, unit: &str, amount: f64) -> Value {
    json!({"type": "FoodNutrient", "nutrient": {"number": number, "name": "x", "unitName": unit}, "amount": amount})
}

#[tokio::test]
async fn usda_import_is_idempotent_and_resolves_shipped_aliases() {
    let db = cupful::db::memory().await;
    let file = std::env::temp_dir().join(format!("cupful-fdc-{}.json", std::process::id()));
    // Shaped like the FDC Foundation download: Atwater energy, carbs by summation, a null entry.
    let data = json!({"FoundationFoods": [
        {"fdcId": 173630, "description": "Chicken, broilers or fryers, wing, meat and skin, cooked, roasted",
         "foodNutrients": [nutrient("958", "kcal", 254.0), nutrient("203", "g", 23.8),
                           nutrient("205.2", "g", 0.0), nutrient("204", "g", 16.9)]},
        {"fdcId": 2, "description": "Fish, cooked, fried", "foodNutrients": [
            nutrient("208", "kcal", 230.0), nutrient("203", "g", 20.0), nutrient("205", "g", 8.0), nutrient("204", "g", 13.0)]},
        {"fdcId": 3, "description": "No energy given", "foodNutrients": [nutrient("203", "g", 1.0)]},
        null
    ]});
    std::fs::write(&file, data.to_string()).unwrap();
    assert_eq!(
        cupful::nutrition::foods::import_usda(&db, &file).await,
        Ok((2, 1))
    );
    assert_eq!(
        cupful::nutrition::foods::import_usda(&db, &file).await,
        Ok((2, 1))
    );
    let rows: Vec<(String, i64)> = sqlx::query_as(
        "SELECT source_ref, is_prepared FROM foods WHERE source = 'usda' ORDER BY source_ref",
    )
    .fetch_all(&db)
    .await
    .unwrap();
    assert_eq!(rows, [("173630".into(), 0), ("2".into(), 1)]);
    let est = run(
        &db,
        0,
        reply(json!([raw("chicken wings", 100.0, json!(null))])),
    )
    .await;
    assert_eq!(
        est.items[0].food_name.as_deref(),
        Some("Chicken, broilers or fryers, wing, meat and skin, cooked, roasted")
    );
    assert_eq!(est.items[0].kcal, Some(254.0));
    std::fs::remove_file(file).unwrap();
}

#[test]
fn eval_cli_names_a_missing_provider() {
    let dir = std::env::temp_dir().join(format!("cupful-eval-{}", std::process::id()));
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_cupful"))
        .args(["eval", "--provider", "42"])
        .env("CUPFUL_DATA_DIR", &dir)
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert_eq!(
        String::from_utf8_lossy(&out.stderr).trim(),
        "no such provider: 42"
    );
    std::fs::remove_dir_all(dir).unwrap();
}

// ---- POST /api/estimate ----

const BOUNDARY: &str = "cupfulboundary";

fn form(description: &str, photo: Option<&[u8]>) -> Vec<u8> {
    let mut b = format!(
        "--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"description\"\r\n\r\n{description}\r\n"
    )
    .into_bytes();
    if let Some(p) = photo {
        b.extend(format!("--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"photo\"; filename=\"p.jpg\"\r\nContent-Type: image/jpeg\r\n\r\n").as_bytes());
        b.extend(p);
        b.extend(b"\r\n");
    }
    b.extend(format!("--{BOUNDARY}--\r\n").as_bytes());
    b
}

async fn post_estimate(c: &Client, body: Vec<u8>) -> (StatusCode, Value) {
    let req = Request::post("/api/estimate")
        .header("x-cupful", "1")
        .header("cookie", c.cookie.clone().unwrap())
        .header(
            "content-type",
            format!("multipart/form-data; boundary={BOUNDARY}"),
        )
        .body(Body::from(body))
        .unwrap();
    let (status, _, bytes) = send(&c.app, req).await;
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

const JPEG: &[u8] = &[0xFF, 0xD8, 0xFF, 0xE0, 0, 0x10, b'J', b'F', b'I', b'F'];

#[tokio::test]
async fn estimate_without_a_provider_is_409_no_provider() {
    let (app, _) = app().await;
    let c = admin(&app).await;
    let (status, body) = post_estimate(&c, form("rice and dal", None)).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["code"], "no_provider");
    let (status, body) = post_estimate(&c, form("rice", Some(JPEG))).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
}

#[tokio::test]
async fn estimate_rejects_bad_input_before_calling_a_provider() {
    let (app, _) = app().await;
    let c = admin(&app).await;
    let (status, _) = post_estimate(&c, form("", None)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = post_estimate(&c, form(&"a".repeat(1001), None)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = post_estimate(&c, form(&"é".repeat(1000), None)).await;
    assert_eq!(status, StatusCode::CONFLICT, "1000 characters is fine");
    let (status, _) = post_estimate(&c, form("rice", Some(b"GIF89a not a photo"))).await;
    assert_eq!(status, StatusCode::UNSUPPORTED_MEDIA_TYPE);
    let mut big = JPEG.to_vec();
    big.resize(5 * 1024 * 1024 + 1, 0);
    let (status, _) = post_estimate(&c, form("rice", Some(&big))).await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
    let mut huge = JPEG.to_vec();
    huge.resize(7 * 1024 * 1024, 0);
    let (status, _) = post_estimate(&c, form("rice", Some(&huge))).await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
}

// ---- own foods, aliases, portions ----

pub async fn user_id(state: &cupful::AppState, name: &str) -> i64 {
    sqlx::query_scalar("SELECT id FROM users WHERE username = ?")
        .bind(name)
        .fetch_one(&state.db)
        .await
        .unwrap()
}

#[tokio::test]
async fn own_food_alias_and_portion_drive_matching() {
    let (app, state) = app().await;
    let mut a = admin(&app).await;
    let (status, food) = a
        .post(
            "/api/foods",
            json!({"name": "Amma's sambar", "kcal": 70, "protein_g": 3, "carbs_g": 9, "fat_g": 2.5}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{food}");
    let id = food["id"].as_i64().unwrap();
    let (status, _) = a
        .post("/api/aliases", json!({"alias": "sambar", "food_id": id}))
        .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = a
        .post(
            "/api/portions",
            json!({"phrase": "my usual sambar", "food_id": id, "grams": 300}),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let uid = user_id(&state, "admin").await;
    // The user's own alias beats the shipped "sambar" alias.
    let est = run(
        &state.db,
        uid,
        reply(json!([raw("sambar", 200.0, json!(null))])),
    )
    .await;
    assert_eq!(est.items[0].food_id, Some(id));
    assert_eq!(est.items[0].kcal, Some(140.0));
    // The saved portion sets food and grams.
    let est = run(
        &state.db,
        uid,
        reply(json!([raw("my usual sambar", 100.0, json!(null))])),
    )
    .await;
    assert_eq!(est.items[0].food_id, Some(id));
    assert_eq!(est.items[0].grams, 300.0);
}

#[tokio::test]
async fn save_routes_validate_input() {
    let (app, _) = app().await;
    let mut a = admin(&app).await;
    for bad in [
        json!({"name": "", "kcal": 70, "protein_g": 3, "carbs_g": 9, "fat_g": 2.5}),
        json!({"name": "x", "kcal": 950, "protein_g": 3, "carbs_g": 9, "fat_g": 2.5}),
        json!({"name": "x", "kcal": 70, "protein_g": -1, "carbs_g": 9, "fat_g": 2.5}),
        json!({"name": "x", "kcal": 70, "protein_g": 60, "carbs_g": 60, "fat_g": 2.5}),
    ] {
        let (status, _) = a.post("/api/foods", bad.clone()).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{bad}");
    }
    let (status, _) = a
        .post("/api/aliases", json!({"alias": "x", "food_id": 999999}))
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = a
        .post(
            "/api/portions",
            json!({"phrase": "x", "food_id": 1, "grams": 0}),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}
