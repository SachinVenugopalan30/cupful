// SPDX-License-Identifier: AGPL-3.0-or-later
mod common;
use axum::http::StatusCode;
use common::*;
use jiff::ToSpan;
use serde_json::json;

const TZ: &str = "Asia/Kolkata";

fn date(days_ago: i64) -> String {
    (today(TZ) - days_ago.days()).to_string()
}

fn meal_with(id: &str, date: &str, description: &str, kcal: f64) -> serde_json::Value {
    let mut m = meal(id, date, json!([item(&uuid(), kcal)]));
    m["description"] = json!(description);
    m
}

#[tokio::test]
async fn groups_by_description_and_returns_the_latest() {
    let (app, _) = app().await;
    let mut a = admin(&app).await;
    let (d1, d2) = (date(2), date(1));
    a.sync(vec![
        op(
            "meal.upsert",
            &at(-30),
            meal_with(&uuid(), &d1, "Oats with banana", 400.0),
        ),
        op(
            "meal.upsert",
            &at(-20),
            meal_with(&uuid(), &d2, "oats with banana", 500.0),
        ),
        op(
            "meal.upsert",
            &at(-10),
            meal_with(&uuid(), &d2, "Chicken rice", 600.0),
        ),
    ])
    .await;

    let (status, body) = a.get("/api/meals/history?limit=50").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let meals = body["meals"].as_array().unwrap();
    assert_eq!(meals.len(), 2);

    let oats = meals
        .iter()
        .find(|m| m["label"] == "Oats with banana")
        .unwrap();
    assert_eq!(oats["times"], 2);
    assert_eq!(oats["date"], d2);
    assert_eq!(oats["items"][0]["kcal"], 500.0);
}

#[tokio::test]
async fn empty_description_falls_back_to_item_names_and_itemless_meals_are_skipped() {
    let (app, _) = app().await;
    let mut a = admin(&app).await;
    a.sync(vec![
        op(
            "meal.upsert",
            &at(0),
            meal_with(&uuid(), &date(1), "", 300.0),
        ),
        op("meal.upsert", &at(0), meal(&uuid(), &date(1), json!([]))),
    ])
    .await;

    let (_, body) = a.get("/api/meals/history?limit=50").await;
    let meals = body["meals"].as_array().unwrap();
    assert_eq!(meals.len(), 1);
    assert_eq!(meals[0]["label"], "rice");
}

#[tokio::test]
async fn groups_before_applying_the_limit_and_excludes_old_or_deleted_meals() {
    let (app, _) = app().await;
    let mut a = admin(&app).await;
    let d = date(1);
    let id = uuid();
    a.sync(vec![
        op(
            "meal.upsert",
            &at(-30),
            meal_with(&uuid(), &d, "Rice", 100.0),
        ),
        op(
            "meal.upsert",
            &at(-20),
            meal_with(&uuid(), &d, "rice", 200.0),
        ),
        op(
            "meal.upsert",
            &at(-10),
            meal_with(&uuid(), &d, "Rice", 300.0),
        ),
        op(
            "meal.upsert",
            &at(-10),
            meal_with(&id, &d, "Old meal", 400.0),
        ),
        op(
            "meal.upsert",
            &at(-10),
            meal_with(&uuid(), &date(400), "Too old", 500.0),
        ),
    ])
    .await;
    a.sync(vec![op("meal.delete", &at(0), json!({"id": id}))])
        .await;

    let (_, body) = a.get("/api/meals/history?limit=1").await;
    let meals = body["meals"].as_array().unwrap();
    assert_eq!(meals.len(), 1);
    assert_eq!(meals[0]["label"], "Rice");
    assert_eq!(meals[0]["times"], 3);
}

#[tokio::test]
async fn limit_is_validated() {
    let (app, _) = app().await;
    let mut a = admin(&app).await;
    for q in ["limit=0", "limit=51", "limit=nope"] {
        let (status, _) = a.get(&format!("/api/meals/history?{q}")).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{q}");
    }
}

#[tokio::test]
async fn history_orders_ties_by_label() {
    let (app, _) = app().await;
    let mut a = admin(&app).await;
    let d = date(1);
    a.sync(vec![
        op(
            "meal.upsert",
            &at(0),
            meal_with(&uuid(), &d, "Zebra bowl", 200.0),
        ),
        op(
            "meal.upsert",
            &at(0),
            meal_with(&uuid(), &d, "Apple bowl", 200.0),
        ),
    ])
    .await;

    let (_, body) = a.get("/api/meals/history?limit=50").await;
    let labels: Vec<_> = body["meals"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["label"].as_str().unwrap())
        .collect();
    assert_eq!(labels, ["Apple bowl", "Zebra bowl"]);
}
