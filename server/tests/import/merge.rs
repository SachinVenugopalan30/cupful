// SPDX-License-Identifier: AGPL-3.0-or-later
//! Identity and merge rules: ids, content matching, ownership, deletes and edits.
use axum::http::StatusCode;
use serde_json::json;
use uuid::Uuid;

use crate::common::*;
use crate::{TZ, csv, error, exported_csv, import, meals};

#[tokio::test]
async fn reimporting_an_own_export_preserves_ids_grams_food_scale_and_source() {
    let (app, state) = app().await;
    let mut a = admin(&app).await;
    let d = today(TZ).to_string();
    let food_id: i64 = sqlx::query_scalar(
        "INSERT INTO foods (source, name, name_norm, kcal, protein_g, carbs_g, fat_g)
         VALUES ('user', 'test rice', 'test rice', 100, 2, 20, 1) RETURNING id",
    )
    .fetch_one(&state.db)
    .await
    .unwrap();
    let meal_id = uuid();
    let item_id = uuid();
    let mut m = meal(
        &meal_id,
        &d,
        json!([{
            "id": item_id, "name": "rice", "portion_text": "1 cup", "grams": 123.0,
            "kcal": 200.0, "protein_g": 4.0, "carbs_g": 40.0, "fat_g": 0.5,
            "food_id": food_id, "origin": "food_db"
        }]),
    );
    m["source"] = json!("ai");
    m["scale"] = json!(1.7);
    m["assumptions"] = json!("assumed");
    m["confidence"] = json!("high");
    assert_eq!(
        a.sync(vec![op("meal.upsert", &at(0), m)]).await,
        ["applied"]
    );

    let exported = exported_csv(&mut a).await;
    let (status, res) = import(&mut a, &exported).await;
    assert_eq!(status, StatusCode::OK, "{res}");
    assert_eq!(res["meals_unchanged"], 1);
    assert_eq!(res["meals_created"], 0);
    assert_eq!(res["meals_updated"], 0);

    let m = meals(&mut a, &d).await;
    let m = &m[0];
    assert_eq!(m["id"], meal_id);
    assert_eq!(m["scale"], 1.7);
    assert_eq!(m["source"], "ai");
    assert_eq!(m["assumptions"], "assumed");
    assert_eq!(m["confidence"], "high");
    assert_eq!(m["items"][0]["id"], item_id);
    assert_eq!(m["items"][0]["grams"], 123.0);
    assert_eq!(m["items"][0]["food_id"], food_id);
    assert_eq!(m["items"][0]["origin"], "food_db");
}

#[tokio::test]
async fn an_edited_description_updates_the_same_meal_and_keeps_item_ids() {
    let (app, _) = app().await;
    let mut a = admin(&app).await;
    let d = today(TZ).to_string();
    let id = uuid();
    let item_id = uuid();
    a.sync(vec![op(
        "meal.upsert",
        &at(0),
        meal(&id, &d, json!([item(&item_id, 200.0)])),
    )])
    .await;
    let exported = exported_csv(&mut a).await;
    let edited = exported.replace("rice and chicken", "rice only");
    let (status, res) = import(&mut a, &edited).await;
    assert_eq!(status, StatusCode::OK, "{res}");
    assert_eq!(res["meals_updated"], 1);
    assert_eq!(res["meals_created"], 0);
    let m = meals(&mut a, &d).await;
    assert_eq!(m[0]["description"], "rice only");
    assert_eq!(m[0]["items"][0]["id"], item_id);
}

#[tokio::test]
async fn terms_that_look_like_formulas_survive_an_export_and_reimport() {
    let (app, _) = app().await;
    let mut a = admin(&app).await;
    let d = today(TZ).to_string();
    let mut m = meal(&uuid(), &d, json!([item(&uuid(), 100.0)]));
    m["description"] = json!("=A1+1");
    m["items"][0]["name"] = json!("@apple");
    m["items"][0]["portion_text"] = json!("-1 cup");
    a.sync(vec![op("meal.upsert", &at(0), m)]).await;
    let exported = exported_csv(&mut a).await;
    assert!(exported.contains("'=A1+1"));
    let (status, res) = import(&mut a, &exported).await;
    assert_eq!(status, StatusCode::OK, "{res}");
    assert_eq!(res["meals_unchanged"], 1);
    let m = meals(&mut a, &d).await;
    assert_eq!(m[0]["description"], "=A1+1");
    assert_eq!(m[0]["items"][0]["name"], "@apple");
    assert_eq!(m[0]["items"][0]["portion_text"], "-1 cup");
}

#[tokio::test]
async fn two_identical_meals_with_different_ids_both_import() {
    let (app, _) = app().await;
    let mut a = admin(&app).await;
    let d = today(TZ).to_string();
    let body = csv(&[
        &format!("{d},aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa,0,lunch,soup,Soup,1 bowl,100,1,2,3,,,,"),
        &format!("{d},bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb,0,lunch,soup,Soup,1 bowl,100,1,2,3,,,,"),
    ]);
    let (status, res) = import(&mut a, &body).await;
    assert_eq!(status, StatusCode::OK, "{res}");
    assert_eq!(res["meals_created"], 2);
    assert_eq!(meals(&mut a, &d).await.as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn a_legacy_row_without_a_meal_id_is_inserted_by_content_and_an_edited_description_is_new() {
    let (app, _) = app().await;
    let mut a = admin(&app).await;
    let d = today(TZ).to_string();
    let original = format!("{d},,0,lunch,soup,Soup,1 bowl,100,1,2,3,,,,");
    assert_eq!(
        import(&mut a, &csv(&[&original])).await.1["meals_created"],
        1
    );
    assert_eq!(
        import(&mut a, &csv(&[&original])).await.1["meals_unchanged"],
        1
    );

    let edited = format!("{d},,0,lunch,new soup,Soup,1 bowl,100,1,2,3,,,,");
    let (status, res) = import(&mut a, &csv(&[&edited])).await;
    assert_eq!(status, StatusCode::OK, "{res}");
    assert_eq!(res["meals_created"], 1);
    assert_eq!(meals(&mut a, &d).await.as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn a_uuid_meal_id_owned_by_another_user_is_hashed_not_rejected() {
    let (app, state) = app().await;
    let mut a = admin(&app).await;
    let mut b = user(&app, &state, "bob").await;
    let d = today(TZ).to_string();
    let foreign = "90909090-9090-4090-8090-909090909090";
    assert_eq!(
        b.sync(vec![op(
            "meal.upsert",
            &at(0),
            meal(foreign, &d, json!([item(&uuid(), 50.0)])),
        )])
        .await,
        ["applied"]
    );
    let body = csv(&[&format!(
        "{d},{foreign},0,lunch,my soup,Soup,1 bowl,100,1,2,3,,,,",
    )]);
    let (status, res) = import(&mut a, &body).await;
    assert_eq!(status, StatusCode::OK, "{res}");
    assert_eq!(res["meals_created"], 1);
    let m = meals(&mut a, &d).await;
    assert_eq!(m.as_array().unwrap().len(), 1);
    assert_ne!(m[0]["id"], foreign);
    // Bob's meal is untouched.
    let bm = meals(&mut b, &d).await;
    assert_eq!(bm[0]["id"], foreign);
}

#[tokio::test]
async fn an_item_id_owned_by_an_unaffected_meal_is_a_conflict() {
    let (app, state) = app().await;
    let mut a = admin(&app).await;
    let mut b = user(&app, &state, "bob").await;
    let d = today(TZ).to_string();
    let a_id: i64 = sqlx::query_scalar("SELECT id FROM users WHERE username = 'admin'")
        .fetch_one(&state.db)
        .await
        .unwrap();
    let raw = "70707070-7070-4070-8070-707070707070";
    let ns = Uuid::new_v5(&Uuid::NAMESPACE_OID, b"cupful fuel-ledger import v1");
    let target = Uuid::new_v5(&ns, format!("{a_id}:{raw}").as_bytes());
    let stolen = Uuid::new_v5(&target, b"0");
    // Bob owns that item id on an unrelated meal.
    assert_eq!(
        b.sync(vec![op(
            "meal.upsert",
            &at(0),
            meal(&uuid(), &d, json!([item(&stolen.to_string(), 10.0)])),
        )])
        .await,
        ["applied"]
    );
    let body = csv(&[&format!("{d},{raw},0,lunch,soup,Soup,1 bowl,100,1,2,3,,,,",)]);
    let (status, res) = import(&mut a, &body).await;
    assert_eq!(status, StatusCode::CONFLICT, "{res}");
    assert!(error(&res).contains("another meal"), "{res}");
}

#[tokio::test]
async fn reimporting_does_not_revive_a_deleted_meal() {
    let (app, _) = app().await;
    let mut a = admin(&app).await;
    let d = today(TZ).to_string();
    let id = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
    let body = csv(&[&format!("{d},{id},0,lunch,soup,Soup,1 bowl,100,1,2,3,,,,")]);
    let (_, res) = import(&mut a, &body).await;
    assert_eq!(res["meals_created"], 1, "{res}");
    // A foreign-looking id is rehashed per user, so take the stored one.
    let stored = meals(&mut a, &d).await[0]["id"]
        .as_str()
        .unwrap()
        .to_string();
    a.sync(vec![op("meal.delete", &at(1), json!({ "id": stored }))])
        .await;
    let (status, res) = import(&mut a, &body).await;
    assert_eq!(status, StatusCode::OK, "{res}");
    assert_eq!(res["meals_skipped_deleted"], 1);
    assert_eq!(res["meals_created"], 0);
    assert!(meals(&mut a, &d).await.as_array().unwrap().is_empty());
}

#[tokio::test]
async fn reimporting_does_not_overwrite_a_meal_edited_in_the_app() {
    let (app, _) = app().await;
    let mut a = admin(&app).await;
    let d = today(TZ).to_string();
    let id = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
    let body = csv(&[&format!("{d},{id},0,lunch,soup,Soup,1 bowl,100,1,2,3,,,,")]);
    import(&mut a, &body).await;
    let m = meals(&mut a, &d).await;
    let stored = m[0]["id"].as_str().unwrap().to_string();
    let item_id = m[0]["items"][0]["id"].as_str().unwrap().to_string();
    let mut edited = meal(&stored, &d, json!([item(&item_id, 555.0)]));
    edited["source"] = json!("import");
    edited["description"] = json!("soup");
    a.sync(vec![op("meal.upsert", &at(1), edited)]).await;
    let (status, res) = import(&mut a, &body).await;
    assert_eq!(status, StatusCode::OK, "{res}");
    assert_eq!(res["meals_skipped_edited"], 1, "{res}");
    assert_eq!(res["meals_updated"], 0);
    assert_eq!(meals(&mut a, &d).await[0]["items"][0]["kcal"], 555.0);
}
