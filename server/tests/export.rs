// SPDX-License-Identifier: AGPL-3.0-or-later
mod common;
use axum::http::StatusCode;
use common::*;
use serde_json::json;

const TZ: &str = "Asia/Kolkata";

async fn export(c: &mut Client) -> (StatusCode, axum::http::HeaderMap, String) {
    let (status, headers, bytes) = c.get_bytes("/api/export.csv").await;
    (status, headers, String::from_utf8(bytes.to_vec()).unwrap())
}

fn records(csv: &str) -> Vec<Vec<String>> {
    let mut r = csv::ReaderBuilder::new()
        .has_headers(true)
        .from_reader(csv.as_bytes());
    r.records()
        .map(|rec| rec.unwrap().iter().map(str::to_string).collect::<Vec<_>>())
        .collect()
}

fn header(csv: &str) -> Vec<String> {
    let mut r = csv::ReaderBuilder::new()
        .has_headers(true)
        .from_reader(csv.as_bytes());
    r.headers().unwrap().iter().map(str::to_string).collect()
}

const HEADER: [&str; 15] = [
    "date",
    "meal_id",
    "item_index",
    "meal",
    "description",
    "food",
    "portion",
    "kcal",
    "protein_g",
    "carbs_g",
    "fat_g",
    "steps",
    "gym_minutes",
    "trained",
    "weight_kg",
];

#[tokio::test]
async fn header_is_exact_and_rows_are_ordered() {
    let (app, _) = app().await;
    let mut a = admin(&app).await;
    let d = today(TZ).to_string();
    let (m1, m2) = (
        "00000000-0000-4000-8000-000000000001",
        "00000000-0000-4000-8000-000000000002",
    );
    a.sync(vec![
        op(
            "meal.upsert",
            &at(0),
            meal(m2, &d, json!([item(&uuid(), 10.0)])),
        ),
        op(
            "meal.upsert",
            &at(0),
            meal(m1, &d, json!([item(&uuid(), 20.0), item(&uuid(), 30.0)])),
        ),
    ])
    .await;
    let (status, _, body) = export(&mut a).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(header(&body), HEADER);
    let rows = records(&body);
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0][1], m1);
    assert_eq!(rows[0][2], "0");
    assert_eq!(rows[1][1], m1);
    assert_eq!(rows[1][2], "1");
    assert_eq!(rows[2][1], m2);
    assert!(rows[0].iter().all(|c| c != "\r"));
}

#[tokio::test]
async fn empty_database_is_only_a_header() {
    let (app, _) = app().await;
    let mut a = admin(&app).await;
    let (status, _, body) = export(&mut a).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, format!("{}\n", HEADER.join(",")));
    assert!(records(&body).is_empty());
}

#[tokio::test]
async fn response_headers_mark_it_as_a_no_store_attachment() {
    let (app, _) = app().await;
    let mut a = admin(&app).await;
    let (status, headers, _) = a.get_bytes("/api/export.csv").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers["content-type"], "text/csv; charset=utf-8");
    assert_eq!(
        headers["content-disposition"],
        format!("attachment; filename=\"cupful-{}.csv\"", today(TZ))
    );
    assert_eq!(headers["cache-control"], "no-store");
}

#[tokio::test]
async fn a_day_without_meals_is_one_row_with_blank_meal_columns() {
    let (app, _) = app().await;
    let mut a = admin(&app).await;
    let d = today(TZ).to_string();
    a.sync(vec![op(
        "day.patch",
        &at(0),
        json!({"date": d, "steps": 8000, "gym_minutes": 30, "trained": ["Chest"], "weight_kg": 80.5}),
    )])
    .await;
    let (_, _, body) = export(&mut a).await;
    let rows = records(&body);
    assert_eq!(rows.len(), 1);
    let r = &rows[0];
    assert_eq!(r[0], d);
    for (i, cell) in r.iter().enumerate().take(11).skip(1) {
        assert_eq!(cell, "", "meal column {i} is blank");
    }
    assert_eq!(r[11], "8000");
    assert_eq!(r[12], "30");
    assert_eq!(r[13], "Chest");
    assert_eq!(r[14], "80.5");
}

#[tokio::test]
async fn a_meal_without_items_is_one_row_with_blank_item_columns() {
    let (app, _) = app().await;
    let mut a = admin(&app).await;
    let d = today(TZ).to_string();
    let id = uuid();
    a.sync(vec![op("meal.upsert", &at(0), meal(&id, &d, json!([])))])
        .await;
    let (_, _, body) = export(&mut a).await;
    let rows = records(&body);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0][1], id);
    assert_eq!(rows[0][2], "");
    for cell in &rows[0][5..=10] {
        assert_eq!(cell, "");
    }
}

#[tokio::test]
async fn tombstoned_meals_are_excluded() {
    let (app, _) = app().await;
    let mut a = admin(&app).await;
    let d = today(TZ).to_string();
    let id = uuid();
    a.sync(vec![op(
        "meal.upsert",
        &at(-10),
        meal(&id, &d, json!([item(&uuid(), 100.0)])),
    )])
    .await;
    a.sync(vec![op("meal.delete", &at(-5), json!({"id": id}))])
        .await;
    let (_, _, body) = export(&mut a).await;
    assert!(records(&body).is_empty());
}

#[tokio::test]
async fn activity_repeats_on_every_row_of_its_date() {
    let (app, _) = app().await;
    let mut a = admin(&app).await;
    let d = today(TZ).to_string();
    let id = uuid();
    a.sync(vec![
        op(
            "meal.upsert",
            &at(0),
            meal(&id, &d, json!([item(&uuid(), 10.0), item(&uuid(), 20.0)])),
        ),
        op(
            "day.patch",
            &at(0),
            json!({"date": d, "steps": 5000, "weight_kg": 70.0}),
        ),
    ])
    .await;
    let rows = records(&export(&mut a).await.2);
    assert_eq!(rows.len(), 2);
    for r in &rows {
        assert_eq!(r[11], "5000");
        assert_eq!(r[14], "70");
    }
}

#[tokio::test]
async fn commas_quotes_newlines_and_unicode_round_trip() {
    let (app, _) = app().await;
    let mut a = admin(&app).await;
    let d = today(TZ).to_string();
    let id = uuid();
    let mut m = meal(&id, &d, json!([item(&uuid(), 100.0)]));
    m["description"] = json!("rice, \"special\"\ntwo lines 米饭");
    m["items"][0]["name"] = json!("chicken\n thigh");
    m["items"][0]["portion_text"] = json!("1 cup, \"heaped\"");
    a.sync(vec![op("meal.upsert", &at(0), m)]).await;
    let body = export(&mut a).await.2;
    let rows = records(&body);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0][4], "rice, \"special\"\ntwo lines 米饭");
    assert_eq!(rows[0][5], "chicken\n thigh");
    assert_eq!(rows[0][6], "1 cup, \"heaped\"");
}

#[tokio::test]
async fn text_that_looks_like_a_formula_gets_one_quote() {
    let (app, _) = app().await;
    let mut a = admin(&app).await;
    let d = today(TZ).to_string();
    let mut m = meal(&uuid(), &d, json!([item(&uuid(), 5.0)]));
    m["description"] = json!("=1+1");
    m["items"][0]["name"] = json!("@apple");
    m["items"][0]["portion_text"] = json!("-1 cup");
    a.sync(vec![op("meal.upsert", &at(0), m)]).await;
    let rows = records(&export(&mut a).await.2);
    assert_eq!(rows[0][4], "'=1+1");
    assert_eq!(rows[0][5], "'@apple");
    assert_eq!(rows[0][6], "'-1 cup");
}

#[tokio::test]
async fn numbers_use_shortest_round_trip_format() {
    let (app, _) = app().await;
    let mut a = admin(&app).await;
    let d = today(TZ).to_string();
    let mut m = meal(&uuid(), &d, json!([item(&uuid(), 100.0)]));
    m["items"][0]["kcal"] = json!(200.0);
    m["items"][0]["protein_g"] = json!(4.25);
    m["items"][0]["carbs_g"] = json!(0.1);
    m["items"][0]["fat_g"] = json!(0.0);
    a.sync(vec![op("meal.upsert", &at(0), m)]).await;
    let rows = records(&export(&mut a).await.2);
    assert_eq!(rows[0][7], "200");
    assert_eq!(rows[0][8], "4.25");
    assert_eq!(rows[0][9], "0.1");
    assert_eq!(rows[0][10], "0");
}

#[tokio::test]
async fn another_users_data_is_not_exported() {
    let (app, state) = app().await;
    let mut a = admin(&app).await;
    let mut b = user(&app, &state, "bob").await;
    let d = today(TZ).to_string();
    b.sync(vec![op(
        "meal.upsert",
        &at(0),
        meal(&uuid(), &d, json!([item(&uuid(), 100.0)])),
    )])
    .await;
    let (_, _, body) = export(&mut a).await;
    assert!(records(&body).is_empty());
}
