// SPDX-License-Identifier: AGPL-3.0-or-later
//! JSON backup and restore (PLAN.md §14 M5).
mod common;
use axum::http::StatusCode;
use common::*;
use cupful::AppState;
use serde_json::{Value, json};

const TZ: &str = "Asia/Kolkata";
const LABEL_BARCODE: &str = "4006381333931";
const OTHER_BARCODE: &str = "96385074";

async fn export(c: &mut Client) -> Value {
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

async fn restore(c: &mut Client, doc: &Value) -> (StatusCode, Value) {
    c.post("/api/restore", json!({"confirm": true, "backup": doc}))
        .await
}

struct Fixture {
    private_food: i64,
    label_food: i64,
    staple: i64,
    off_food: i64,
    meal_id: String,
    item_ids: Vec<String>,
    day: String,
}

fn find_food(doc: &Value, id: i64) -> &Value {
    doc["foods"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["id"].as_i64() == Some(id))
        .unwrap_or_else(|| panic!("food {id} missing from backup"))
}

/// Profile (with an older direct version), a day, a meal touching private, label, staple and off
/// foods, an alias, a portion and a saved product.
async fn rich_data(_app: &axum::Router, state: &AppState, c: &mut Client) -> Fixture {
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

#[tokio::test]
async fn export_shape_headers_and_ordering() {
    let (app, state) = app().await;
    let mut a = admin(&app).await;
    let f = rich_data(&app, &state, &mut a).await;
    let doc = export(&mut a).await;

    assert_eq!(doc["format"], "cupful-backup");
    assert_eq!(doc["version"], 1);
    assert!(doc["exported_at"].as_str().unwrap().contains('T'));
    assert_eq!(doc["username"], "admin");
    assert_eq!(doc["profile"]["timezone"], TZ);
    assert_eq!(doc["profile"]["weight_unit"], "kg");
    for key in [
        "profile_versions",
        "days",
        "meals",
        "foods",
        "aliases",
        "portions",
        "products",
    ] {
        assert!(doc[key].is_array(), "{key} must be an array");
    }
    assert_eq!(doc["profile_versions"].as_array().unwrap().len(), 2);
    assert_eq!(doc["meals"].as_array().unwrap().len(), 1);
    assert_eq!(doc["meals"][0]["id"], f.meal_id.as_str());
    assert_eq!(doc["meals"][0]["items"].as_array().unwrap().len(), 5);
    assert_eq!(doc["meals"][0]["scale"], 1.25);
    assert_eq!(doc["meals"][0]["source"], "ai");
    assert_eq!(doc["meals"][0]["assumptions"], "assumed cooked rice");
    assert!(doc["meals"][0]["provider_id"].is_null());
    assert!(doc["meals"][0]["received_at"].is_string());
    assert!(doc["meals"][0]["updated_at"].is_string());

    let foods = doc["foods"].as_array().unwrap();
    let ids: Vec<i64> = foods.iter().map(|f| f["id"].as_i64().unwrap()).collect();
    let mut sorted = ids.clone();
    sorted.sort_unstable();
    assert_eq!(ids, sorted, "foods ordered by id");
    for id in [f.private_food, f.label_food, f.staple, f.off_food] {
        let food = find_food(&doc, id);
        assert!(food.get("portions").is_some(), "food {id} carries portions");
    }
    assert!(
        find_food(&doc, f.label_food)["portions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p["unit"] == "cup")
    );
    // Items keep their food and a resolved `food` object.
    let item0 = &doc["meals"][0]["items"][0];
    assert_eq!(item0["id"], f.item_ids[0].as_str());
    assert_eq!(item0["food"]["source"], "user");
    assert_eq!(item0["food_id"], f.private_food);
    // Days expose values, sources and per-field update times.
    assert_eq!(doc["days"][0]["steps"], 9123);
    assert_eq!(doc["days"][0]["steps_source"], "manual");
    assert!(doc["days"][0]["steps_updated_at"].is_string());
    assert_eq!(doc["days"][0]["trained"], json!(["Chest", "Arms"]));
    assert!(doc["days"][0]["weight_updated_at"].is_string());
    // Natural-key collections.
    assert_eq!(doc["aliases"][0]["alias"], "my basmati");
    assert_eq!(doc["portions"][0]["phrase_norm"], "my usual bar");
    assert!(
        doc["products"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p["food_id"] == f.label_food)
    );

    // The filename uses the user's local today.
    let (_, headers, _) = a.get_bytes("/api/backup").await;
    assert_eq!(
        headers
            .get("content-disposition")
            .unwrap()
            .to_str()
            .unwrap(),
        format!("attachment; filename=\"cupful-backup-{}.json\"", today(TZ))
    );
}

#[tokio::test]
async fn round_trip_into_a_fresh_instance() {
    let (app, state) = app().await;
    let mut a = admin(&app).await;
    let f = rich_data(&app, &state, &mut a).await;
    let doc = export(&mut a).await;

    let (app2, state2) = common::app().await;
    let mut b = admin(&app2).await;
    // Start the target with different logs so the restore really replaces them.
    assert_eq!(
        b.sync(vec![op(
            "day.patch",
            &at(-5),
            json!({"date": today(TZ).to_string(), "steps": 1})
        )])
        .await,
        ["applied"]
    );
    let (status, counts) = restore(&mut b, &doc).await;
    assert_eq!(status, StatusCode::OK, "{counts}");
    for key in [
        "profile_versions",
        "days",
        "meals",
        "items",
        "foods",
        "aliases",
        "portions",
        "products",
    ] {
        assert!(counts[key].as_u64().unwrap() > 0, "{key} count");
    }
    assert_eq!(counts["meals"], 1);
    assert_eq!(counts["items"], 5);

    let (status, profile) = b.get("/api/profile").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(profile["timezone"], TZ);
    assert_eq!(profile["weight_unit"], "kg");
    assert_eq!(profile["version"]["birth_year"], 1990);

    let (status, meals) = b.get(&format!("/api/meals?date={}", f.day)).await;
    assert_eq!(status, StatusCode::OK, "{meals}");
    assert_eq!(meals["meals"].as_array().unwrap().len(), 1);
    let m = &meals["meals"][0];
    assert_eq!(m["id"], f.meal_id.as_str());
    assert_eq!(m["scale"], 1.25);
    assert_eq!(m["source"], "ai");
    assert_eq!(m["confidence"], "high");
    assert_eq!(m["assumptions"], "assumed cooked rice");
    assert_eq!(m["items"].as_array().unwrap().len(), 5);
    assert_eq!(m["totals"]["kcal"], 754.0);
    assert!(m["provider_id"].is_null());

    // Timestamps and sources were kept exactly; received_at and provider are server-owned.
    let day_row: (
        String,
        Option<i64>,
        Option<String>,
        Option<String>,
        Option<i64>,
    ) = sqlx::query_as(
        "SELECT date, steps, steps_source, trained, weight_updated_at FROM days
             WHERE user_id = (SELECT id FROM users ORDER BY id LIMIT 1)",
    )
    .fetch_one(&state2.db)
    .await
    .unwrap();
    let original = &doc["days"][0];
    assert_eq!(day_row.0, f.day);
    assert_eq!(day_row.1, Some(9123));
    assert_eq!(day_row.2.as_deref(), Some("manual"));
    assert_eq!(day_row.3.as_deref(), Some("[\"Chest\",\"Arms\"]"));
    assert_eq!(
        day_row.4,
        Some(
            original["weight_updated_at"]
                .as_str()
                .unwrap()
                .parse::<jiff::Timestamp>()
                .unwrap()
                .as_millisecond()
        )
    );
    let meal_row: (i64, i64, Option<i64>) = sqlx::query_as(
        "SELECT updated_at, received_at, provider_id FROM meals
         WHERE id = ?",
    )
    .bind(&f.meal_id)
    .fetch_one(&state2.db)
    .await
    .unwrap();
    assert_eq!(
        meal_row.0,
        doc["meals"][0]["updated_at"]
            .as_str()
            .unwrap()
            .parse::<jiff::Timestamp>()
            .unwrap()
            .as_millisecond()
    );
    assert!(meal_row.1 > 0);
    assert!(meal_row.2.is_none());

    // Private food came back, with its per-food portions and for its owner only.
    let private: (String, Option<i64>, f64) =
        sqlx::query_as("SELECT name, user_id, kcal FROM foods WHERE name = 'My protein scoop'")
            .fetch_one(&state2.db)
            .await
            .unwrap();
    assert_eq!(private.0, "My protein scoop");
    assert!(private.1.is_some());
    assert_eq!(private.2, 400.0);
    let portions: Vec<String> = sqlx::query_scalar(
        "SELECT fp.unit FROM food_portions fp JOIN foods f ON f.id = fp.food_id
         WHERE f.name = 'My protein scoop' ORDER BY fp.unit",
    )
    .fetch_all(&state2.db)
    .await
    .unwrap();
    assert!(portions.is_empty(), "a food with no portions keeps none");

    // Label and off foods existed only in the first instance, so they were restored as private
    // copies (never written into the shared database).
    let label_owner: Option<i64> =
        sqlx::query_scalar("SELECT user_id FROM foods WHERE barcode = ? AND name = 'Test yogurt'")
            .bind(LABEL_BARCODE)
            .fetch_one(&state2.db)
            .await
            .unwrap();
    assert!(label_owner.is_some(), "missing label became private");
    let label_portions: Vec<String> = sqlx::query_scalar(
        "SELECT fp.unit FROM food_portions fp JOIN foods f ON f.id = fp.food_id
         WHERE f.barcode = ? ORDER BY fp.unit",
    )
    .bind(LABEL_BARCODE)
    .fetch_all(&state2.db)
    .await
    .unwrap();
    assert_eq!(label_portions, ["cup", "serving"]);
    let off: (String, Option<i64>) =
        sqlx::query_as("SELECT name, user_id FROM foods WHERE barcode = '99999991'")
            .fetch_one(&state2.db)
            .await
            .unwrap();
    assert_eq!(off.0, "Off oat bar");
    assert!(off.1.is_some());

    // The staple existed in both instances and stayed shared.
    let staple_source: String = sqlx::query_scalar("SELECT source FROM foods WHERE id = ?")
        .bind(f.staple)
        .fetch_one(&state2.db)
        .await
        .unwrap();
    assert_eq!(staple_source, "staple");

    // Aliases, portions and products resolve to restored food rows.
    let (status, aliases) = b.get("/api/aliases").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(aliases["aliases"][0]["alias"], "my basmati");
    let portion: (String, f64) = sqlx::query_as(
        "SELECT phrase_norm, grams FROM user_portions
         WHERE user_id = (SELECT id FROM users ORDER BY id LIMIT 1)",
    )
    .fetch_one(&state2.db)
    .await
    .unwrap();
    assert_eq!(portion, ("my usual bar".into(), 55.0));
    let (status, products) = b.get("/api/products").await;
    assert_eq!(status, StatusCode::OK, "{products}");
    let yogurt = products["products"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["name"] == "Test yogurt")
        .expect("the saved product survived");
    assert_eq!(yogurt["last_servings"], 1.0);
}

/// Strip fields that legitimately change between exports (instance-local ids and times).
fn stable(doc: &Value) -> Value {
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

#[tokio::test]
async fn repeated_restore_is_idempotent() {
    let (app, state) = app().await;
    let mut a = admin(&app).await;
    rich_data(&app, &state, &mut a).await;
    let doc = export(&mut a).await;

    let (app2, _) = common::app().await;
    let mut b = admin(&app2).await;
    let (status, first) = restore(&mut b, &doc).await;
    assert_eq!(status, StatusCode::OK, "{first}");
    let once = export(&mut b).await;
    let (status, second) = restore(&mut b, &doc).await;
    assert_eq!(status, StatusCode::OK, "{second}");
    assert_eq!(first, second, "the same document yields the same counts");
    let twice = export(&mut b).await;
    assert_eq!(stable(&once), stable(&twice));
}

#[tokio::test]
async fn restore_replaces_prior_logs_and_profile_versions() {
    let (app, state) = app().await;
    let mut a = admin(&app).await;
    rich_data(&app, &state, &mut a).await;
    let doc = export(&mut a).await;

    let extra_id = uuid();
    assert_eq!(
        a.sync(vec![
            op(
                "meal.upsert",
                &at(-1),
                meal(
                    &extra_id,
                    &today(TZ).to_string(),
                    json!([item(&uuid(), 1.0)])
                )
            ),
            op(
                "day.patch",
                &at(-1),
                json!({"date": today(TZ).checked_sub(jiff::ToSpan::days(4)).unwrap().to_string(),
                       "steps": 5})
            ),
        ])
        .await,
        ["applied", "applied"]
    );
    let (status, _) = a
        .post(
            "/api/aliases",
            json!({"alias": "to vanish", "food_id": doc["foods"][0]["id"]}),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    sqlx::query(
        "INSERT INTO profile_versions (user_id, effective_from, sex, birth_year, height_cm,
             start_weight_kg, pace_kg_per_week, typical_steps)
         VALUES ((SELECT id FROM users ORDER BY id LIMIT 1), '2020-01-01', 'male', 1990,
             180.0, 85.0, 0.5, 8000)",
    )
    .execute(&state.db)
    .await
    .unwrap();

    let (status, body) = restore(&mut a, &doc).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let got = export(&mut a).await;
    assert_eq!(got["profile_versions"].as_array().unwrap().len(), 2);
    assert_eq!(got["meals"].as_array().unwrap().len(), 1);
    assert_eq!(got["meals"][0]["id"], doc["meals"][0]["id"]);
    assert_eq!(got["days"].as_array().unwrap().len(), 1);
    let aliases: Vec<String> = got["aliases"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["alias"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(aliases, ["my basmati"]);
}

#[tokio::test]
async fn shared_and_label_rows_are_not_modified() {
    let (app, state) = app().await;
    let mut a = admin(&app).await;
    let f = rich_data(&app, &state, &mut a).await;
    let doc = export(&mut a).await;

    let snapshot = |id: i64| {
        let db = state.db.clone();
        async move {
            sqlx::query_as::<_, (i64, String, Option<i64>, Option<String>, f64, String)>(
                "SELECT id, source, user_id, source_ref, kcal, name FROM foods WHERE id = ?",
            )
            .bind(id)
            .fetch_one(&db)
            .await
            .unwrap()
        }
    };
    let label_before = snapshot(f.label_food).await;
    let staple_before = snapshot(f.staple).await;
    let shared_count: i64 = sqlx::query_scalar("SELECT count(*) FROM foods WHERE user_id IS NULL")
        .fetch_one(&state.db)
        .await
        .unwrap();

    let (status, body) = restore(&mut a, &doc).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(snapshot(f.label_food).await, label_before);
    assert_eq!(snapshot(f.staple).await, staple_before);
    let shared_after: i64 = sqlx::query_scalar("SELECT count(*) FROM foods WHERE user_id IS NULL")
        .fetch_one(&state.db)
        .await
        .unwrap();
    assert_eq!(shared_after, shared_count, "no new shared rows");
}

#[tokio::test]
async fn another_users_label_food_survives_a_restore() {
    let (app, state) = app().await;
    let mut a = admin(&app).await;
    rich_data(&app, &state, &mut a).await;
    let doc = export(&mut a).await;

    let mut b = user(&app, &state, "bob").await;
    let (status, body) = b
        .post(
            "/api/products",
            json!({"name": "Bob's bar", "barcode": OTHER_BARCODE, "basis": "100g",
                   "kcal": 200.0, "protein_g": 5.0, "carbs_g": 30.0, "fat_g": 7.0}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let bob_food = body["food_id"].as_i64().unwrap();
    let bob_meal = uuid();
    assert_eq!(
        b.sync(vec![op(
            "meal.upsert",
            &at(-5),
            meal(
                &bob_meal,
                &today(TZ).to_string(),
                json!([
                    json!({"id": uuid(), "name": "bob bar", "portion_text": "1 bar",
                              "grams": 50.0, "kcal": 100.0, "protein_g": 2.5,
                              "carbs_g": 15.0, "fat_g": 3.5, "food_id": bob_food,
                              "origin": "food_db"})
                ])
            )
        )])
        .await,
        ["applied"]
    );

    let (status, body) = restore(&mut a, &doc).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let row: (Option<i64>, String) = sqlx::query_as("SELECT user_id, name FROM foods WHERE id = ?")
        .bind(bob_food)
        .fetch_one(&state.db)
        .await
        .unwrap();
    let bob_id: i64 = sqlx::query_scalar("SELECT id FROM users WHERE username = 'bob'")
        .fetch_one(&state.db)
        .await
        .unwrap();
    assert_eq!(row, (Some(bob_id), "Bob's bar".into()));
    let (status, meals) = b.get(&format!("/api/meals?date={}", today(TZ))).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(meals["meals"][0]["id"], bob_meal.as_str());
    assert_eq!(meals["meals"][0]["items"][0]["food_id"], bob_food);
}

#[tokio::test]
async fn invalid_documents_are_400_and_change_nothing() {
    let (app, state) = app().await;
    let mut a = admin(&app).await;
    rich_data(&app, &state, &mut a).await;
    let doc = export(&mut a).await;
    let baseline = stable(&export(&mut a).await);

    type Mutation = Box<dyn Fn(&mut Value)>;
    let cases: Vec<(&str, Mutation)> = vec![
        (
            "timezone",
            Box::new(|d: &mut Value| d["profile"]["timezone"] = json!("Not/AZone")),
        ),
        (
            "future date",
            Box::new(|d: &mut Value| {
                let far = today(TZ)
                    .checked_add(jiff::ToSpan::days(3))
                    .unwrap()
                    .to_string();
                d["days"][0]["date"] = json!(far);
            }),
        ),
        (
            "future timestamp",
            Box::new(|d: &mut Value| d["days"][0]["steps_updated_at"] = json!(at(600))),
        ),
        (
            "unknown food",
            Box::new(|d: &mut Value| {
                d["meals"][0]["items"][0]["food_id"] = json!(999_999);
            }),
        ),
        (
            "bad source",
            Box::new(|d: &mut Value| d["days"][0]["steps_source"] = json!("telepathy")),
        ),
        (
            "bad body part",
            Box::new(|d: &mut Value| d["days"][0]["trained"] = json!(["Tail"])),
        ),
        (
            "bad meal date",
            Box::new(|d: &mut Value| {
                let far = today(TZ)
                    .checked_add(jiff::ToSpan::days(2))
                    .unwrap()
                    .to_string();
                d["meals"][0]["date"] = json!(far);
            }),
        ),
        (
            "duplicate day",
            Box::new(|d: &mut Value| {
                let first = d["days"][0].clone();
                d["days"].as_array_mut().unwrap().push(first);
            }),
        ),
        (
            "wrong format",
            Box::new(|d: &mut Value| d["format"] = json!("something-else")),
        ),
        (
            "wrong version",
            Box::new(|d: &mut Value| d["version"] = json!(2)),
        ),
    ];

    for (name, mutate) in cases {
        let mut bad = doc.clone();
        mutate(&mut bad);
        let (status, body) = restore(&mut a, &bad).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{name}: {body}");
        assert_eq!(
            stable(&export(&mut a).await),
            baseline,
            "{name} changed data"
        );
    }
}

#[tokio::test]
async fn confirm_must_be_true() {
    let (app, state) = app().await;
    let mut a = admin(&app).await;
    rich_data(&app, &state, &mut a).await;
    let doc = export(&mut a).await;

    let (status, _) = a
        .post(
            "/api/restore",
            json!({"confirm": false, "backup": doc.clone()}),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = a.post("/api/restore", json!({"backup": doc})).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = a.post("/api/restore", json!({"confirm": true})).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn foreign_ids_are_409_and_roll_back() {
    let (app, state) = app().await;
    let mut a = admin(&app).await;
    rich_data(&app, &state, &mut a).await;
    let doc = export(&mut a).await;
    let original = export(&mut a).await;

    let mut b = user(&app, &state, "bob").await;
    let bob_meal = uuid();
    let bob_item = uuid();
    assert_eq!(
        b.sync(vec![op(
            "meal.upsert",
            &at(-5),
            meal(
                &bob_meal,
                &today(TZ).to_string(),
                json!([
                    json!({"id": bob_item, "name": "bobrice", "portion_text": "",
                              "grams": null, "kcal": 10.0, "protein_g": 0.0,
                              "carbs_g": 0.0, "fat_g": 0.0, "food_id": null,
                              "origin": "manual"})
                ])
            )
        )])
        .await,
        ["applied"]
    );

    // A meal id owned by B.
    let mut conflict = doc.clone();
    conflict["meals"][0]["id"] = json!(bob_meal);
    let (status, body) = restore(&mut a, &conflict).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(stable(&export(&mut a).await), stable(&original));

    // An item id owned by B, in an otherwise valid meal.
    let mut conflict = doc.clone();
    conflict["meals"][0]["items"][0]["id"] = json!(bob_item);
    let (status, body) = restore(&mut a, &conflict).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(stable(&export(&mut a).await), stable(&original));

    // B's data is untouched too.
    let (status, meals) = b.get(&format!("/api/meals?date={}", today(TZ))).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(meals["meals"][0]["id"], bob_meal.as_str());
}

#[tokio::test]
async fn credentials_tokens_providers_and_google_survive() {
    let (app, state) = app().await;
    let mut a = admin(&app).await;
    rich_data(&app, &state, &mut a).await;
    let doc = export(&mut a).await;

    let (token_id, _) = a.token("phone").await;
    let (status, provider) = a
        .post(
            "/api/ai/providers",
            json!({"label": "local", "kind": "openai_compatible",
                   "base_url": "http://127.0.0.1:1/v1", "model": "m"}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{provider}");
    let provider_id = provider["id"].as_i64().unwrap();
    sqlx::query(
        "INSERT INTO google_connections (user_id, refresh_token, connected_at)
         VALUES ((SELECT id FROM users ORDER BY id LIMIT 1), 'sealed', 1)",
    )
    .execute(&state.db)
    .await
    .unwrap();

    let (status, body) = restore(&mut a, &doc).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let tokens: Vec<i64> = sqlx::query_scalar("SELECT id FROM ingest_tokens ORDER BY id")
        .fetch_all(&state.db)
        .await
        .unwrap();
    assert_eq!(tokens, [token_id]);
    let providers: Vec<i64> = sqlx::query_scalar("SELECT id FROM ai_providers ORDER BY id")
        .fetch_all(&state.db)
        .await
        .unwrap();
    assert_eq!(providers, [provider_id]);
    let google: i64 = sqlx::query_scalar("SELECT count(*) FROM google_connections")
        .fetch_one(&state.db)
        .await
        .unwrap();
    assert_eq!(google, 1);
}

#[tokio::test]
async fn backup_excludes_credentials_providers_ops_and_tombstones() {
    let (app, state) = app().await;
    let mut a = admin(&app).await;
    rich_data(&app, &state, &mut a).await;
    let (_, token) = a.token("phone").await;
    let (status, _) = a
        .post(
            "/api/ai/providers",
            json!({"label": "local", "kind": "openai_compatible",
                   "base_url": "http://127.0.0.1:1/v1", "model": "m",
                   "api_key": "sk-super-secret"}),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let deleted: String =
        sqlx::query_scalar("SELECT id FROM meals WHERE deleted_at IS NOT NULL LIMIT 1")
            .fetch_one(&state.db)
            .await
            .unwrap();

    let doc = export(&mut a).await;
    let text = doc.to_string();
    assert!(!text.contains(&token), "ingest token must not be exported");
    assert!(!text.contains("sk-super-secret"));
    assert!(!text.contains("token_hash"));
    assert!(!text.contains("ai_providers"));
    assert!(!text.contains("sync_ops"));
    assert!(!text.contains("deleted_at"));
    assert!(
        !text.contains(&deleted),
        "tombstoned meal must not be exported"
    );
    assert!(!doc.as_object().unwrap().contains_key("ingest_tokens"));
    assert!(!doc.as_object().unwrap().contains_key("providers"));
}

#[tokio::test]
async fn backups_are_isolated_between_users() {
    let (app, state) = app().await;
    let mut a = admin(&app).await;
    let f = rich_data(&app, &state, &mut a).await;
    let a_doc = export(&mut a).await;

    let mut b = user(&app, &state, "bob").await;
    let b_doc = export(&mut b).await;
    assert!(b_doc["meals"].as_array().unwrap().is_empty());
    assert!(b_doc["days"].as_array().unwrap().is_empty());
    assert!(b_doc["profile_versions"].as_array().unwrap().is_empty());
    assert!(b_doc["aliases"].as_array().unwrap().is_empty());
    assert!(b_doc["portions"].as_array().unwrap().is_empty());
    assert!(b_doc["products"].as_array().unwrap().is_empty());
    let b_food_ids: Vec<i64> = b_doc["foods"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["id"].as_i64().unwrap())
        .collect();
    for id in [f.private_food, f.label_food, f.off_food] {
        assert!(!b_food_ids.contains(&id), "B sees none of A's foods");
    }
    assert!(
        !b_doc.to_string().contains("My protein scoop"),
        "A's private food must not leak into B's backup"
    );

    // B restoring A's document must not touch A. A's private food cannot be accepted for B.
    let (status, body) = restore(&mut b, &a_doc).await;
    assert!(
        status == StatusCode::BAD_REQUEST || status == StatusCode::CONFLICT,
        "{status}: {body}"
    );
    assert_eq!(stable(&export(&mut a).await), stable(&a_doc.clone()));
    let (status, meals) = a.get(&format!("/api/meals?date={}", f.day)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(meals["meals"][0]["id"], f.meal_id.as_str());
}

#[tokio::test]
async fn restore_keeps_tombstones_and_sync_ops() {
    let (app, state) = app().await;
    let mut a = admin(&app).await;
    rich_data(&app, &state, &mut a).await;
    let doc = export(&mut a).await;
    let tombstones = || async {
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM meals WHERE deleted_at IS NOT NULL")
            .fetch_one(&state.db)
            .await
            .unwrap()
    };
    assert_eq!(tombstones().await, 1);
    let ops_before: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sync_ops")
        .fetch_one(&state.db)
        .await
        .unwrap();
    assert!(ops_before > 0);

    let (status, body) = restore(&mut a, &doc).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(tombstones().await, 1, "tombstone survives restore");
    let ops_after: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sync_ops")
        .fetch_one(&state.db)
        .await
        .unwrap();
    assert_eq!(ops_before, ops_after, "sync_ops survive restore");

    // An offline device replaying its queue cannot resurrect the deleted meal, and an
    // already-applied op id is a duplicate.
    let gone: String = sqlx::query_scalar("SELECT id FROM meals WHERE deleted_at IS NOT NULL")
        .fetch_one(&state.db)
        .await
        .unwrap();
    let day = today(TZ).to_string();
    let replay = op(
        "meal.upsert",
        &at(-8),
        meal(&gone, &day, json!([item(&uuid(), 10.0)])),
    );
    assert_eq!(a.sync(vec![replay]).await, ["stale"]);
    let old_op: String = sqlx::query_scalar("SELECT op_id FROM sync_ops LIMIT 1")
        .fetch_one(&state.db)
        .await
        .unwrap();
    let mut dup = op("day.patch", &at(-1), json!({"date": day, "steps": 1}));
    dup["op_id"] = json!(old_op);
    assert_eq!(a.sync(vec![dup]).await, ["duplicate"]);
}

#[tokio::test]
async fn backup_meal_wins_over_a_target_tombstone() {
    let (app, state) = app().await;
    let mut a = admin(&app).await;
    let fx = rich_data(&app, &state, &mut a).await;
    let doc = export(&mut a).await;
    assert_eq!(
        a.sync(vec![op("meal.delete", &at(-1), json!({"id": fx.meal_id}))])
            .await,
        ["applied"]
    );
    assert!(export(&mut a).await["meals"].as_array().unwrap().is_empty());

    let (status, body) = restore(&mut a, &doc).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let got = export(&mut a).await;
    assert_eq!(got["meals"].as_array().unwrap().len(), 1);
    assert_eq!(got["meals"][0]["id"], json!(fx.meal_id));
    assert_eq!(got["meals"][0]["items"].as_array().unwrap().len(), 5);
}

#[tokio::test]
async fn same_named_private_foods_stay_distinct() {
    let (app, state) = app().await;
    let mut a = admin(&app).await;
    let fx = rich_data(&app, &state, &mut a).await;
    let mut doc = export(&mut a).await;
    let mut twin = find_food(&doc, fx.private_food).clone();
    twin["id"] = json!(900_001);
    twin["kcal"] = json!(123.0);
    twin["protein_g"] = json!(1.0);
    doc["foods"].as_array_mut().unwrap().push(twin);

    let (status, body) = restore(&mut a, &doc).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let kcals: Vec<f64> = sqlx::query_scalar(
        "SELECT kcal FROM foods WHERE name = 'My protein scoop' AND user_id IS NOT NULL ORDER BY kcal",
    )
    .fetch_all(&state.db)
    .await
    .unwrap();
    assert_eq!(kcals, [123.0, 400.0]);
}

#[tokio::test]
async fn oversized_restore_body_is_rejected() {
    let (app, _) = app().await;
    let mut a = admin(&app).await;
    let big = "x".repeat(11 << 20);
    let (status, _) = a
        .post(
            "/api/restore",
            json!({"confirm": true, "backup": {"pad": big}}),
        )
        .await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
}

#[tokio::test]
async fn old_backup_with_a_shared_label_food_restores_privately() {
    let (app, state) = app().await;
    let mut a = admin(&app).await;
    let fx = rich_data(&app, &state, &mut a).await;
    let mut doc = export(&mut a).await;
    // Older backups wrote label foods as shared rows: user_id null, source_ref = owner.
    let uid: i64 = sqlx::query_scalar("SELECT id FROM users ORDER BY id LIMIT 1")
        .fetch_one(&state.db)
        .await
        .unwrap();
    for f in doc["foods"].as_array_mut().unwrap() {
        if f["id"] == json!(fx.label_food) {
            f["user_id"] = json!(null);
            f["source_ref"] = json!(uid.to_string());
        }
    }
    let (status, body) = restore(&mut a, &doc).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let owner: Option<i64> =
        sqlx::query_scalar("SELECT user_id FROM foods WHERE barcode = ? AND name = 'Test yogurt'")
            .bind(LABEL_BARCODE)
            .fetch_one(&state.db)
            .await
            .unwrap();
    assert_eq!(owner, Some(uid));
}
