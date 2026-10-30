// SPDX-License-Identifier: AGPL-3.0-or-later
//! The steps, gym and weight columns.
use axum::http::StatusCode;
use serde_json::json;

use crate::common::*;
use crate::{TZ, csv, error, import};

#[tokio::test]
async fn manual_activity_survives_an_import_and_fill_only_fields_are_not_overwritten() {
    let (app, _) = app().await;
    let mut a = admin(&app).await;
    let d = today(TZ).to_string();
    a.sync(vec![op(
        "day.patch",
        &at(0),
        json!({"date": d, "steps": 5000, "gym_minutes": 20, "trained": ["Chest"], "weight_kg": 81.0}),
    )])
    .await;
    let row = format!("{d},,,,,,,,,,,12000,30,Legs,70.0");
    let (status, res) = import(&mut a, &csv(&[&row])).await;
    assert_eq!(status, StatusCode::OK, "{res}");
    let (_, day) = a.get(&format!("/api/days/{d}")).await;
    assert_eq!(day["steps"], 5000);
    assert_eq!(day["steps_source"], "manual");
    assert_eq!(day["gym_minutes"], 20);
    assert_eq!(day["gym_source"], "manual");
    assert_eq!(day["trained"], json!(["Chest"]));
    assert_eq!(day["weight_kg"], 81.0);
}

#[tokio::test]
async fn days_are_filled_with_import_source_when_nothing_is_stored() {
    let (app, _) = app().await;
    let mut a = admin(&app).await;
    let d = today(TZ).to_string();
    let row = format!("{d},,,,,,,,,,,9000,45,Legs;Core,79.5");
    let (status, res) = import(&mut a, &csv(&[&row])).await;
    assert_eq!(status, StatusCode::OK, "{res}");
    assert_eq!(res["days"], 1);
    let (_, day) = a.get(&format!("/api/days/{d}")).await;
    assert_eq!(day["steps"], 9000);
    assert_eq!(day["steps_source"], "import");
    assert_eq!(day["trained"], json!(["Legs", "Core"]));
    assert_eq!(day["weight_kg"], 79.5);
}

#[tokio::test]
async fn conflicting_activity_for_one_date_is_rejected() {
    let (app, _) = app().await;
    let mut a = admin(&app).await;
    let d = today(TZ).to_string();
    let body = csv(&[
        &format!("{d},,,,,,,,,,,5000,,,"),
        &format!("{d},,,,,,,,,,,6000,,,"),
    ]);
    let (status, res) = import(&mut a, &body).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(error(&res).contains("line 3"), "{res}");
    assert!(error(&res).contains("conflicting steps"), "{res}");
}
