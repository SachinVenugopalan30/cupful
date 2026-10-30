// SPDX-License-Identifier: AGPL-3.0-or-later
//! What a valid backup document does when restored.
use axum::http::StatusCode;
use serde_json::json;

use crate::common::*;
use crate::{LABEL_BARCODE, TZ, export, find_food, restore, rich_data, stable};

#[tokio::test]
async fn round_trip_into_a_fresh_instance() {
    let (app, state) = app().await;
    let mut a = admin(&app).await;
    let f = rich_data(&state, &mut a).await;
    let doc = export(&mut a).await;

    let (app2, state2) = crate::common::app().await;
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

#[tokio::test]
async fn repeated_restore_is_idempotent() {
    let (app, state) = app().await;
    let mut a = admin(&app).await;
    rich_data(&state, &mut a).await;
    let doc = export(&mut a).await;

    let (app2, _) = crate::common::app().await;
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
    rich_data(&state, &mut a).await;
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
async fn another_users_label_food_survives_a_restore() {
    let (app, state) = app().await;
    let mut a = admin(&app).await;
    rich_data(&state, &mut a).await;
    let doc = export(&mut a).await;

    let mut b = user(&app, &state, "bob").await;
    let (status, body) = b
        .post(
            "/api/products",
            json!({"name": "Bob's bar", "barcode": crate::OTHER_BARCODE, "basis": "100g",
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
async fn restore_keeps_tombstones_and_sync_ops() {
    let (app, state) = app().await;
    let mut a = admin(&app).await;
    rich_data(&state, &mut a).await;
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
    let fx = rich_data(&state, &mut a).await;
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
async fn unseen_delete_tombstones_are_ignored_by_export_and_kept_by_restore() {
    let (app, state) = app().await;
    let mut a = admin(&app).await;
    rich_data(&state, &mut a).await;
    let ghost = uuid();
    assert_eq!(
        a.sync(vec![op("meal.delete", &at(-1), json!({"id": ghost}))])
            .await,
        ["applied"]
    );
    let doc = export(&mut a).await;
    assert!(!doc.to_string().contains(&ghost));
    let (status, body) = restore(&mut a, &doc).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let n: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM meals WHERE id = ? AND deleted_at IS NOT NULL")
            .bind(&ghost)
            .fetch_one(&state.db)
            .await
            .unwrap();
    assert_eq!(n, 1);
}

#[tokio::test]
async fn old_backup_with_a_shared_label_food_restores_privately() {
    let (app, state) = app().await;
    let mut a = admin(&app).await;
    let fx = rich_data(&state, &mut a).await;
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

#[tokio::test]
async fn same_named_private_foods_stay_distinct() {
    let (app, state) = app().await;
    let mut a = admin(&app).await;
    let fx = rich_data(&state, &mut a).await;
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
