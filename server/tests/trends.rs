// SPDX-License-Identifier: AGPL-3.0-or-later
mod common;
use axum::http::StatusCode;
use common::*;
use serde_json::json;

const D1: &str = "2026-01-05";
const D2: &str = "2026-01-06";
const D3: &str = "2026-01-07";

async fn setup() -> Client {
    let (app, state) = app().await;
    let mut a = admin(&app).await;
    a.put("/api/profile", profile_body()).await;
    sqlx::query("UPDATE profile_versions SET effective_from = '2026-01-06'")
        .execute(&state.db)
        .await
        .unwrap();
    let items = json!([item(&uuid(), 500.0), item(&uuid(), 250.0)]);
    a.sync(vec![
        op("meal.upsert", &at(-60), meal(&uuid(), D2, items)),
        op(
            "day.patch",
            &at(-60),
            json!({"date": D1, "weight_kg": 84.0}),
        ),
        op(
            "day.patch",
            &at(-60),
            json!({"date": D2, "steps": 9000, "gym_minutes": 30}),
        ),
    ])
    .await;
    a
}

#[tokio::test]
async fn one_entry_per_date_with_null_rules() {
    let mut a = setup().await;
    let (status, t) = a.get(&format!("/api/trends?from={D1}&to={D3}")).await;
    assert_eq!(status, StatusCode::OK);
    let days = t["days"].as_array().unwrap();
    assert_eq!(days.len(), 3);
    assert_eq!(days[0]["date"], D1);
    // Before the first profile version: no budget; the weigh-in is that day's.
    assert_eq!(days[0]["budget_kcal"], json!(null));
    assert_eq!(days[0]["weight_kg"], 84.0);
    assert_eq!(days[0]["eaten_kcal"], json!(null));
    assert_eq!(days[0]["steps"], json!(null));
    assert_eq!(days[1]["eaten_kcal"], 750);
    assert_eq!(days[1]["steps"], 9000);
    assert_eq!(days[1]["gym_minutes"], 30);
    // No weigh-in that day: null, even though the budget carries the weight forward.
    assert_eq!(days[1]["weight_kg"], json!(null));
    assert!(days[1]["budget_kcal"].is_i64());
    assert_eq!(days[2]["eaten_kcal"], json!(null));
}

#[tokio::test]
async fn budget_matches_the_budget_endpoint() {
    let mut a = setup().await;
    let (_, t) = a.get(&format!("/api/trends?from={D2}&to={D3}")).await;
    for (i, d) in [D2, D3].into_iter().enumerate() {
        let (_, b) = a.get(&format!("/api/budget/{d}")).await;
        assert_eq!(t["days"][i]["budget_kcal"], b["budget"], "{d}");
    }
    // The carried-forward 84 kg (not the 85 kg start weight) fed D2's budget.
    let (_, b) = a.get(&format!("/api/budget/{D2}")).await;
    assert_eq!(b["weight_kg"], 84.0);
}

#[tokio::test]
async fn range_is_validated() {
    let mut a = setup().await;
    for q in [
        "from=2026-01-07&to=2026-01-05",
        "from=2025-01-01&to=2026-02-05",
        "from=nope&to=2026-01-05",
    ] {
        let (status, _) = a.get(&format!("/api/trends?{q}")).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{q}");
    }
    let (status, t) = a.get("/api/trends?from=2025-01-01&to=2026-01-05").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(t["days"].as_array().unwrap().len(), 370);
    let (_, t) = a.get(&format!("/api/trends?from={D1}&to={D1}")).await;
    assert_eq!(t["days"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn deleted_meals_do_not_count() {
    let mut a = setup().await;
    let id = uuid();
    let m = meal(&id, D3, json!([item(&uuid(), 100.0)]));
    a.sync(vec![op("meal.upsert", &at(-30), m)]).await;
    a.sync(vec![op("meal.delete", &at(-20), json!({"id": id}))])
        .await;
    let (_, t) = a.get(&format!("/api/trends?from={D3}&to={D3}")).await;
    assert_eq!(t["days"][0]["eaten_kcal"], json!(null));
}
