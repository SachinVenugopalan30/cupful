// SPDX-License-Identifier: AGPL-3.0-or-later
//! Documents and requests a restore must refuse.
use axum::http::StatusCode;
use serde_json::{Value, json};

use crate::common::*;
use crate::{TZ, export, restore, rich_data, stable};

#[tokio::test]
async fn invalid_documents_are_400_and_change_nothing() {
    let (app, state) = app().await;
    let mut a = admin(&app).await;
    rich_data(&state, &mut a).await;
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
    rich_data(&state, &mut a).await;
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
async fn foreign_ids_are_409_and_roll_back() {
    let (app, state) = app().await;
    let mut a = admin(&app).await;
    rich_data(&state, &mut a).await;
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
