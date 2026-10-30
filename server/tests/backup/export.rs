// SPDX-License-Identifier: AGPL-3.0-or-later
//! What the backup document contains, and what it must never contain.
use axum::http::StatusCode;
use serde_json::json;

use crate::common::*;
use crate::{TZ, export, find_food, restore, rich_data, stable};

#[tokio::test]
async fn export_shape_headers_and_ordering() {
    let (app, state) = app().await;
    let mut a = admin(&app).await;
    let f = rich_data(&state, &mut a).await;
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
async fn backup_excludes_credentials_providers_ops_and_tombstones() {
    let (app, state) = app().await;
    let mut a = admin(&app).await;
    rich_data(&state, &mut a).await;
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
    let f = rich_data(&state, &mut a).await;
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
