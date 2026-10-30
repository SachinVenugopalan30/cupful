// SPDX-License-Identifier: AGPL-3.0-or-later
//! Whole sample and legacy files, imported once and again.
use axum::http::StatusCode;
use serde_json::json;

use crate::common::*;
use crate::{HEADER, TZ, fixture, import, meals};

#[tokio::test]
async fn importing_the_sample_fixture_twice_changes_nothing_the_second_time() {
    let (app, _) = app().await;
    let mut a = admin(&app).await;
    let body = fixture("fuel-ledger-sample.csv");
    let (status, first) = import(&mut a, &body).await;
    assert_eq!(status, StatusCode::OK, "{first}");
    assert_eq!(first["meals_created"], 3);
    assert_eq!(first["meals_updated"], 0);
    assert_eq!(first["items"], 5);
    assert_eq!(first["days"], 4);

    let (status, second) = import(&mut a, &body).await;
    assert_eq!(status, StatusCode::OK, "{second}");
    assert_eq!(second["meals_created"], 0);
    assert_eq!(second["meals_updated"], 0);
    assert_eq!(second["meals_unchanged"], 3);
    assert_eq!(second["days"], 0);

    let m = meals(&mut a, "2026-01-05").await;
    assert_eq!(m.as_array().unwrap().len(), 2);
    let (_, day) = a.get("/api/days/2026-01-05").await;
    assert_eq!(day["steps"], 8000);
    assert_eq!(day["steps_source"], "import");
    assert_eq!(day["gym_minutes"], 30);
    assert_eq!(day["trained"], json!(["Chest", "Arms"]));
    assert_eq!(day["weight_kg"], 80.5);
    // A zero calorie is a value, not a blank.
    let all = meals(&mut a, "2026-01-05").await;
    let coffee = all
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|m| m["items"].as_array().unwrap())
        .find(|i| i["name"] == "Coffee")
        .unwrap();
    assert_eq!(coffee["kcal"], 0.0);
}

#[tokio::test]
async fn a_second_import_leaves_day_sources_and_timestamps_alone() {
    let (app, state) = app().await;
    let mut a = admin(&app).await;
    let body = fixture("fuel-ledger-sample.csv");
    assert_eq!(import(&mut a, &body).await.0, StatusCode::OK);
    let q = "SELECT steps_source, steps_updated_at, gym_source, gym_updated_at,
                    trained_updated_at, weight_updated_at
             FROM days WHERE user_id = 1 AND date = '2026-01-05'";
    let read = || async {
        sqlx::query_as::<
            _,
            (
                Option<String>,
                Option<i64>,
                Option<String>,
                Option<i64>,
                Option<i64>,
                Option<i64>,
            ),
        >(q)
        .fetch_one(&state.db)
        .await
        .unwrap()
    };
    let before = read().await;
    assert_eq!(import(&mut a, &body).await.0, StatusCode::OK);
    assert_eq!(read().await, before);
}

#[tokio::test]
async fn legacy_identical_consecutive_meals_import_separately_and_reimport_stably() {
    let (app, _) = app().await;
    let mut a = admin(&app).await;
    let body = fixture("fuel-ledger-legacy.csv");
    let (status, first) = import(&mut a, &body).await;
    assert_eq!(status, StatusCode::OK, "{first}");
    assert_eq!(first["meals_created"], 3);
    let ids1: Vec<String> = meals(&mut a, "2026-02-01")
        .await
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["id"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(ids1.len(), 2);
    assert_ne!(ids1[0], ids1[1]);

    let (status, second) = import(&mut a, &body).await;
    assert_eq!(status, StatusCode::OK, "{second}");
    assert_eq!(second["meals_unchanged"], 3);
    assert_eq!(second["meals_created"], 0);
    let ids2: Vec<String> = meals(&mut a, "2026-02-01")
        .await
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["id"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(ids1, ids2);
}

#[tokio::test]
async fn bom_and_crlf_are_accepted() {
    let (app, _) = app().await;
    let mut a = admin(&app).await;
    let d = today(TZ).to_string();
    let row = format!("{d},,0,lunch,soup,Soup,1 bowl,100,1,2,3,,,,");
    let body = format!("\u{feff}{HEADER}\r\n{row}\r\n");
    let (status, res) = import(&mut a, &body).await;
    assert_eq!(status, StatusCode::OK, "{res}");
    assert_eq!(res["meals_created"], 1);
}
