// SPDX-License-Identifier: AGPL-3.0-or-later
mod common;
use common::*;
use jiff::civil::date;
use serde_json::json;
use sqlx::SqlitePool;

const TODAY: jiff::civil::Date = date(2026, 6, 30);

async fn dump(db: &SqlitePool) -> Vec<String> {
    let mut out = Vec::new();
    for q in [
        "SELECT json_group_array(json_object('d', date, 's', steps, 'ss', steps_source, 'g', gym_minutes,
             't', trained, 'w', weight_kg)) FROM (SELECT * FROM days ORDER BY date)",
        "SELECT json_group_array(json_object('id', id, 'd', date, 't', meal_type, 'x', description,
             'u', updated_at)) FROM (SELECT * FROM meals ORDER BY id)",
        "SELECT json_group_array(json_object('id', id, 'm', meal_id, 'n', name, 'g', grams, 'k', kcal))
             FROM (SELECT * FROM meal_items ORDER BY id)",
        "SELECT json_group_array(json_object('e', effective_from, 'w', start_weight_kg))
             FROM profile_versions",
    ] {
        out.push(sqlx::query_scalar(q).fetch_one(db).await.unwrap());
    }
    out
}

#[tokio::test]
async fn same_seed_gives_identical_rows() {
    let (_, a) = app().await;
    let (_, b) = app().await;
    let (_, c) = app().await;
    cupful::demo::seed(&a.db, TODAY, 90, 42, false)
        .await
        .unwrap();
    cupful::demo::seed(&b.db, TODAY, 90, 42, false)
        .await
        .unwrap();
    cupful::demo::seed(&c.db, TODAY, 90, 7, false)
        .await
        .unwrap();
    let (da, db, dc) = (dump(&a.db).await, dump(&b.db).await, dump(&c.db).await);
    assert_eq!(da, db);
    assert_ne!(da, dc);
    assert!(da[1].len() > 1000, "meals were created");
}

#[tokio::test]
async fn demo_data_is_realistic_and_readable() {
    let (app, state) = app().await;
    let password = cupful::demo::seed(&state.db, TODAY, 90, 42, false)
        .await
        .unwrap();
    let mut c = Client::new(&app);
    let (status, body) = c
        .post(
            "/api/auth/login",
            json!({"username": "demo", "password": password}),
        )
        .await;
    assert_eq!(status, axum::http::StatusCode::OK, "{body}");
    let (_, t) = c.get("/api/trends?from=2026-04-02&to=2026-06-30").await;
    let days = t["days"].as_array().unwrap();
    assert_eq!(days.len(), 90);
    assert!(days.iter().all(|d| d["budget_kcal"].is_i64()));
    let eaten: Vec<i64> = days
        .iter()
        .filter_map(|d| d["eaten_kcal"].as_i64())
        .collect();
    assert!(
        eaten.len() > 80 && eaten.len() < 90,
        "a few days without meals"
    );
    assert!(eaten.iter().all(|k| (800..4000).contains(k)), "{eaten:?}");
    let steps = days.iter().filter(|d| d["steps"].is_i64()).count();
    assert!((60..90).contains(&steps), "some steps missing: {steps}");
    let gym = days.iter().filter(|d| d["gym_minutes"].is_i64()).count();
    assert!((20..50).contains(&gym), "about 3 a week: {gym}");
    let first = days.iter().find_map(|d| d["weight_kg"].as_f64()).unwrap();
    let last = days
        .iter()
        .rev()
        .find_map(|d| d["weight_kg"].as_f64())
        .unwrap();
    assert!(last < first, "trending down: {first} -> {last}");
}

#[tokio::test]
async fn existing_demo_is_refused_unless_forced_and_others_are_untouched() {
    let (app, state) = app().await;
    let mut a = admin(&app).await;
    a.sync(vec![op(
        "day.patch",
        &at(-60),
        json!({"date": "2026-01-05", "steps": 7000}),
    )])
    .await;
    cupful::demo::seed(&state.db, TODAY, 10, 1, false)
        .await
        .unwrap();
    let err = cupful::demo::seed(&state.db, TODAY, 10, 1, false)
        .await
        .unwrap_err();
    assert!(err.contains("--force"), "{err}");
    cupful::demo::seed(&state.db, TODAY, 10, 1, true)
        .await
        .unwrap();
    let users: i64 = sqlx::query_scalar("SELECT count(*) FROM users WHERE username = 'demo'")
        .fetch_one(&state.db)
        .await
        .unwrap();
    assert_eq!(users, 1);
    let (_, day) = a.get("/api/days/2026-01-05").await;
    assert_eq!(day["steps"], 7000);
}
