// SPDX-License-Identifier: AGPL-3.0-or-later
//! JSON backup and restore (PLAN.md §14 M5).
//!
//! `export` covers the backup document itself, `restore` what a valid document does to an
//! instance, and `validate` what is refused before anything changes.
/// Shared helpers live at tests/common/mod.rs, outside this target directory.
#[path = "../common/mod.rs"]
mod common;

mod export;
mod restore;
mod survives;
mod validate;

use axum::http::StatusCode;
use common::*;
use cupful::AppState;
use serde_json::{Value, json};

pub(crate) const TZ: &str = "Asia/Kolkata";
pub(crate) const LABEL_BARCODE: &str = "4006381333931";
pub(crate) const OTHER_BARCODE: &str = "96385074";

pub(crate) async fn export(c: &mut Client) -> Value {
    let (status, headers, bytes) = c.get_bytes("/api/backup").await;
    assert_eq!(status, StatusCode::OK, "{bytes:?}");
    assert_eq!(
        headers.get("content-type").unwrap().to_str().unwrap(),
        "application/json"
    );
    assert_eq!(
        headers.get("cache-control").unwrap().to_str().unwrap(),
        "no-store"
    );
    json(&bytes)
}

pub(crate) async fn restore(c: &mut Client, doc: &Value) -> (StatusCode, Value) {
    c.post("/api/restore", json!({"confirm": true, "backup": doc}))
        .await
}

pub(crate) struct Fixture {
    pub(crate) private_food: i64,
    pub(crate) label_food: i64,
    pub(crate) staple: i64,
    pub(crate) off_food: i64,
    pub(crate) meal_id: String,
    pub(crate) item_ids: Vec<String>,
    pub(crate) day: String,
}

pub(crate) fn find_food(doc: &Value, id: i64) -> &Value {
    doc["foods"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["id"].as_i64() == Some(id))
        .unwrap_or_else(|| panic!("food {id} missing from backup"))
}

/// Strip fields that legitimately change between exports (instance-local ids and times).
pub(crate) fn stable(doc: &Value) -> Value {
    let mut d = doc.clone();
    d.as_object_mut().unwrap().remove("exported_at");
    if let Some(meals) = d["meals"].as_array_mut() {
        for meal in meals {
            // The server stamps receipt time on restore; it is not part of the backup.
            meal.as_object_mut().unwrap().remove("received_at");
        }
    }
    d
}

/// Profile (with an older direct version), a day, a meal touching private, label, staple and off
/// foods, an alias, a portion and a saved product.
pub(crate) async fn rich_data(state: &AppState, c: &mut Client) -> Fixture {
    let (status, body) = c.put("/api/profile", profile_body()).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let uid: i64 = sqlx::query_scalar("SELECT id FROM users ORDER BY id LIMIT 1")
        .fetch_one(&state.db)
        .await
        .unwrap();
    let old = today(TZ).checked_sub(jiff::ToSpan::days(200)).unwrap();
    sqlx::query(
        "INSERT INTO profile_versions (user_id, effective_from, sex, birth_year, height_cm,
             start_weight_kg, goal_kg, pace_kg_per_week, typical_steps)
         VALUES (?, ?, 'female', 1992, 165.0, 72.0, 65.0, 0.25, 6000)",
    )
    .bind(uid)
    .bind(old.to_string())
    .execute(&state.db)
    .await
    .unwrap();

    let staple: i64 =
        sqlx::query_scalar("SELECT id FROM foods WHERE source = 'staple' ORDER BY id LIMIT 1")
            .fetch_one(&state.db)
            .await
            .unwrap();
    let off_food: i64 = sqlx::query_scalar(
        "INSERT INTO foods (source, source_ref, name, name_norm, barcode, kcal, protein_g,
             carbs_g, fat_g)
         VALUES ('off', '99999991', 'Off oat bar', 'off oat bar', '99999991', 380.0, 9.0, 62.0, 11.0)
         RETURNING id",
    )
    .fetch_one(&state.db)
    .await
    .unwrap();

    let (status, body) = c
        .post(
            "/api/foods",
            json!({"name": "My protein scoop", "is_prepared": false, "kcal": 400.0,
                   "protein_g": 80.0, "carbs_g": 10.0, "fat_g": 5.0,
                   "default_portion_g": 30.0}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let private_food = body["id"].as_i64().unwrap();

    let (status, body) = c
        .post(
            "/api/products",
            json!({"name": "Test yogurt", "brand": "Test", "barcode": LABEL_BARCODE,
                   "basis": "100g", "kcal": 70.0, "protein_g": 4.0, "carbs_g": 8.0,
                   "fat_g": 2.0, "serving_g": 150.0, "serving_text": "1 cup (150 g)"}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let label_food = body["food_id"].as_i64().unwrap();

    let (status, body) = c
        .post(
            "/api/aliases",
            json!({"alias": "my basmati", "food_id": staple}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body) = c
        .post(
            "/api/portions",
            json!({"phrase": "my usual bar", "food_id": off_food, "grams": 55.0}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let day = today(TZ).to_string();
    assert_eq!(
        c.sync(vec![op(
            "day.patch",
            &at(-30),
            json!({"date": day, "steps": 9123, "gym_minutes": 45,
                   "trained": ["Chest", "Arms"], "weight_kg": 82.4})
        )])
        .await,
        ["applied"]
    );

    let meal_id = uuid();
    let item_ids = vec![uuid(), uuid(), uuid(), uuid(), uuid()];
    let items = vec![
        json!({"id": item_ids[0], "name": "protein", "portion_text": "1 scoop", "grams": 30.0,
               "kcal": 120.0, "protein_g": 24.0, "carbs_g": 3.0, "fat_g": 1.5,
               "food_id": private_food, "origin": "food_db"}),
        json!({"id": item_ids[1], "name": "yogurt", "portion_text": "1 cup", "grams": 150.0,
               "kcal": 105.0, "protein_g": 6.0, "carbs_g": 12.0, "fat_g": 3.0,
               "food_id": label_food, "origin": "food_db"}),
        json!({"id": item_ids[2], "name": "rice", "portion_text": "1 cup", "grams": 150.0,
               "kcal": 200.0, "protein_g": 4.0, "carbs_g": 42.0, "fat_g": 1.0,
               "food_id": staple, "origin": "food_db"}),
        json!({"id": item_ids[3], "name": "bar", "portion_text": "1 bar", "grams": 55.0,
               "kcal": 209.0, "protein_g": 5.0, "carbs_g": 34.1, "fat_g": 6.0,
               "food_id": off_food, "origin": "food_db"}),
        json!({"id": item_ids[4], "name": "handful of nuts", "portion_text": "handful",
               "grams": 20.0, "kcal": 120.0, "protein_g": 3.0, "carbs_g": 4.0, "fat_g": 10.0,
               "food_id": null, "origin": "ai_estimate"}),
    ];
    // An item at position with a scale > 1 and assumptions exercises every meal field.
    let mut main_meal = meal(&meal_id, &day, json!(items));
    main_meal["scale"] = json!(1.25);
    main_meal["assumptions"] = json!("assumed cooked rice");
    main_meal["source"] = json!("ai");
    main_meal["confidence"] = json!("high");
    assert_eq!(
        c.sync(vec![op("meal.upsert", &at(-20), main_meal)]).await,
        ["applied"]
    );

    // A deleted meal must not appear in the backup.
    let gone = uuid();
    assert_eq!(
        c.sync(vec![
            op(
                "meal.upsert",
                &at(-10),
                meal(&gone, &day, json!([item(&uuid(), 10.0)]))
            ),
            op("meal.delete", &at(-5), json!({"id": gone})),
        ])
        .await,
        ["applied", "applied"]
    );
    Fixture {
        private_food,
        label_food,
        staple,
        off_food,
        meal_id,
        item_ids,
        day,
    }
}
