// SPDX-License-Identifier: AGPL-3.0-or-later
mod common;
use axum::http::StatusCode;
use common::*;
use serde_json::json;

const TZ: &str = "Asia/Kolkata";

#[tokio::test]
async fn empty_day_reads_as_nulls() {
    let (app, _) = app().await;
    let mut a = admin(&app).await;
    let (status, day) = a.get("/api/days/2026-01-05").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(day["date"], "2026-01-05");
    assert_eq!(day["steps"], json!(null));
    assert_eq!(day["trained"], json!([]));
}

#[tokio::test]
async fn day_ranges_return_logged_days_and_are_capped_at_400() {
    let (app, _) = app().await;
    let mut a = admin(&app).await;
    a.sync(vec![
        op(
            "day.patch",
            &at(0),
            json!({"date": "2026-01-02", "steps": 100}),
        ),
        op(
            "day.patch",
            &at(0),
            json!({"date": "2026-01-04", "steps": 200}),
        ),
    ])
    .await;
    let (status, body) = a.get("/api/days?from=2026-01-01&to=2026-01-03").await;
    assert_eq!(status, StatusCode::OK);
    let days = body["days"].as_array().unwrap();
    assert_eq!(days.len(), 1);
    assert_eq!(days[0]["steps"], 100);

    let (status, _) = a.get("/api/days?from=2025-01-01&to=2026-02-05").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = a.get("/api/days?from=2026-01-05&to=2026-01-01").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn budget_needs_a_profile() {
    let (app, _) = app().await;
    let mut a = admin(&app).await;
    let (status, _) = a.get(&format!("/api/budget/{}", today(TZ))).await;
    assert_eq!(status, StatusCode::CONFLICT);
}

#[tokio::test]
async fn budget_uses_the_latest_weight_on_or_before_the_date_and_the_days_activity() {
    let (app, state) = app().await;
    let mut a = admin(&app).await;
    sqlx::query("INSERT INTO profile_versions (user_id, effective_from, sex, birth_year, height_cm, start_weight_kg, goal_kg, pace_kg_per_week, typical_steps) VALUES (1, '2026-01-01', 'male', 1990, 180, 85, 75, 0.5, 8000)")
        .execute(&state.db).await.unwrap();
    a.sync(vec![
        op("day.patch", &at(0), json!({"date": "2026-01-03", "weight_kg": 80.0})),
        op("day.patch", &at(0), json!({"date": "2026-01-10", "weight_kg": 70.0})),
        op("day.patch", &at(0), json!({"date": "2026-01-05", "steps": 10000, "gym_minutes": 60, "trained": ["Chest", "Arms"]})),
    ]).await;
    let (status, b) = a.get("/api/budget/2026-01-05").await;
    assert_eq!(status, StatusCode::OK, "{b}");
    assert_eq!(b["weight_kg"], 80.0);
    assert_eq!(b["steps"], 10000);
    assert_eq!(b["flags"], json!([]));
    // Same inputs as the first fixture case, except age (36 in 2026).
    assert_eq!(b["budget"], 2212);

    let (_, b) = a.get("/api/budget/2026-01-02").await;
    assert_eq!(b["weight_kg"], 85.0, "before any weigh-in: start weight");
    assert_eq!(b["flags"], json!(["provisional"]));

    let (status, _) = a.get("/api/budget/2025-12-31").await;
    assert_eq!(
        status,
        StatusCode::CONFLICT,
        "no profile version in effect yet"
    );
}

#[tokio::test]
async fn bad_dates_are_400() {
    let (app, _) = app().await;
    let mut a = admin(&app).await;
    for uri in [
        "/api/days/2026-13-01",
        "/api/budget/yesterday",
        "/api/meals?date=nope",
    ] {
        let (status, _) = a.get(uri).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{uri}");
    }
}

fn meal_on(id: &str, date: &str, meal_type: &str, items: serde_json::Value) -> serde_json::Value {
    let mut m = meal(id, date, items);
    m["meal_type"] = json!(meal_type);
    m
}

fn item_with(kcal: f64, protein: f64, carbs: f64, fat: f64) -> serde_json::Value {
    let mut i = item(&uuid(), kcal);
    i["protein_g"] = json!(protein);
    i["carbs_g"] = json!(carbs);
    i["fat_g"] = json!(fat);
    i
}

#[tokio::test]
async fn trends_average_macros_and_meal_mix_over_days_with_meals() {
    let (app, _) = app().await;
    let mut a = admin(&app).await;
    a.sync(vec![
        op(
            "meal.upsert",
            &at(0),
            meal_on(
                &uuid(),
                "2026-01-02",
                "breakfast",
                json!([item_with(300.0, 10.0, 20.0, 5.0)]),
            ),
        ),
        op(
            "meal.upsert",
            &at(0),
            meal_on(
                &uuid(),
                "2026-01-02",
                "lunch",
                json!([item_with(500.0, 20.0, 30.0, 10.0)]),
            ),
        ),
        op(
            "meal.upsert",
            &at(0),
            meal_on(
                &uuid(),
                "2026-01-03",
                "dinner",
                json!([item_with(700.0, 30.0, 40.0, 15.0)]),
            ),
        ),
    ])
    .await;

    let (status, body) = a.get("/api/trends?from=2026-01-01&to=2026-01-05").await;
    assert_eq!(status, StatusCode::OK, "{body}");

    // Two days have meals: Jan 2 and Jan 3. Jan 5 is a gap, not a zero.
    assert_eq!(body["averages"]["protein_g"], json!(30.0));
    assert_eq!(body["averages"]["carbs_g"], json!(45.0));
    assert_eq!(body["averages"]["fat_g"], json!(15.0));

    let mix = &body["meal_mix"];
    assert_eq!(mix["breakfast"], json!(150));
    assert_eq!(mix["lunch"], json!(250));
    assert_eq!(mix["dinner"], json!(350));
    assert_eq!(mix["snack"], json!(0));
    let sum: i64 = ["breakfast", "lunch", "dinner", "snack"]
        .iter()
        .map(|k| mix[k].as_i64().unwrap())
        .sum();
    let eaten: i64 = body["days"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|d| d["eaten_kcal"].as_i64())
        .sum();
    assert_eq!(sum, 750, "the mix adds up to the average eaten kcal");
    assert_eq!(eaten, 1500);
}

#[tokio::test]
async fn trends_meal_mix_rounds_to_whole_kcal_without_losing_the_sum() {
    let (app, _) = app().await;
    let mut a = admin(&app).await;
    a.sync(vec![
        op(
            "meal.upsert",
            &at(0),
            meal_on(
                &uuid(),
                "2026-02-01",
                "breakfast",
                json!([item_with(100.0, 1.0, 1.0, 1.0)]),
            ),
        ),
        op(
            "meal.upsert",
            &at(0),
            meal_on(
                &uuid(),
                "2026-02-02",
                "lunch",
                json!([item_with(101.0, 1.0, 1.0, 1.0)]),
            ),
        ),
    ])
    .await;

    let (status, body) = a.get("/api/trends?from=2026-02-01&to=2026-02-02").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let mix = &body["meal_mix"];
    // Exact averages are 50 and 50.5 kcal; the rounded total is 101.
    assert_eq!(mix["breakfast"], json!(50));
    assert_eq!(mix["lunch"], json!(51));
    assert_eq!(
        mix["breakfast"].as_i64().unwrap()
            + mix["lunch"].as_i64().unwrap()
            + mix["dinner"].as_i64().unwrap()
            + mix["snack"].as_i64().unwrap(),
        101
    );
}

#[tokio::test]
async fn trends_averages_are_null_with_no_meals() {
    let (app, _) = app().await;
    let mut a = admin(&app).await;
    let (status, body) = a.get("/api/trends?from=2026-03-01&to=2026-03-07").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["averages"], json!(null));
    assert_eq!(body["meal_mix"], json!(null));
    assert_eq!(body["days"].as_array().unwrap().len(), 7);
}
