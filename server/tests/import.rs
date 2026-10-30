// SPDX-License-Identifier: AGPL-3.0-or-later
mod common;
use axum::http::StatusCode;
use common::*;
use serde_json::{Value, json};
use uuid::Uuid;

const TZ: &str = "Asia/Kolkata";
const HEADER: &str = "date,meal_id,item_index,meal,description,food,portion,kcal,protein_g,carbs_g,fat_g,steps,gym_minutes,trained,weight_kg";

fn csv(rows: &[&str]) -> String {
    format!("{HEADER}\n{}\n", rows.join("\n"))
}

fn fixture(name: &str) -> String {
    std::fs::read_to_string(format!("{}/../fixtures/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap()
}

async fn import(c: &mut Client, body: &str) -> (StatusCode, Value) {
    c.post_text("/api/import/fuel-ledger", body).await
}

async fn meals(c: &mut Client, date: &str) -> Value {
    let (status, body) = c.get(&format!("/api/meals?date={date}")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body["meals"].clone()
}

fn error(body: &Value) -> String {
    body["error"].as_str().unwrap_or_default().to_string()
}

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

    let (_, _, bytes) = a.get_bytes("/api/export.csv").await;
    let exported = String::from_utf8(bytes.to_vec()).unwrap();
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
    let (_, _, bytes) = a.get_bytes("/api/export.csv").await;
    let exported = String::from_utf8(bytes.to_vec()).unwrap();
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
    let (_, _, bytes) = a.get_bytes("/api/export.csv").await;
    let exported = String::from_utf8(bytes.to_vec()).unwrap();
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
async fn manual_activity_survives_an_import_and_fill_only_fields_are_not_overwritten() {
    let (app, state) = app().await;
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
    let _ = state;
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
async fn a_blank_kcal_with_food_is_rejected_but_zero_is_a_value() {
    let (app, _) = app().await;
    let mut a = admin(&app).await;
    let d = today(TZ).to_string();
    let bad = format!("{d},,0,lunch,soup,Soup,1 bowl,,,,,,,,");
    let (status, res) = import(&mut a, &csv(&[&bad])).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(error(&res).contains("line 2"), "{res}");
    assert!(error(&res).contains("kcal"), "{res}");

    let zero = format!("{d},,0,lunch,soup,Soup,1 bowl,0,0,0,0,,,,");
    let (status, res) = import(&mut a, &csv(&[&zero])).await;
    assert_eq!(status, StatusCode::OK, "{res}");
    let m = meals(&mut a, &d).await;
    assert_eq!(m[0]["items"][0]["kcal"], 0.0);
}

#[tokio::test]
async fn food_without_kcal_and_kcal_without_food_are_both_rejected() {
    let (app, _) = app().await;
    let mut a = admin(&app).await;
    let d = today(TZ).to_string();
    let (status, res) = import(
        &mut a,
        &csv(&[&format!("{d},,0,lunch,soup,Soup,1 bowl,,,,,,,,")]),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(error(&res).contains("kcal is required"), "{res}");

    let (status, res) = import(
        &mut a,
        &csv(&[&format!("{d},,0,lunch,soup,,1 bowl,100,,,,,,,")]),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(error(&res).contains("food is required"), "{res}");
}

#[tokio::test]
async fn duplicate_and_missing_item_indexes_are_rejected_with_a_line() {
    let (app, _) = app().await;
    let mut a = admin(&app).await;
    let d = today(TZ).to_string();
    let id = "12121212-1212-4212-8212-121212121212";
    let dup = csv(&[
        &format!("{d},{id},0,lunch,soup,Soup,1 bowl,100,1,2,3,,,,"),
        &format!("{d},{id},0,lunch,soup,Tea,1 cup,5,0,1,0,,,,"),
    ]);
    let (status, res) = import(&mut a, &dup).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(error(&res).contains("line 3"), "{res}");
    assert!(error(&res).contains("duplicate item_index"), "{res}");

    let missing = csv(&[&format!("{d},{id},,lunch,soup,Soup,1 bowl,100,1,2,3,,,,")]);
    let (status, res) = import(&mut a, &missing).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(error(&res).contains("item_index"), "{res}");
}

#[tokio::test]
async fn an_inconsistent_meal_group_is_rejected() {
    let (app, _) = app().await;
    let mut a = admin(&app).await;
    let d = today(TZ).to_string();
    let id = "34343434-3434-4434-8434-343434343434";
    let body = csv(&[
        &format!("{d},{id},0,lunch,soup,Soup,1 bowl,100,1,2,3,,,,"),
        &format!("{d},{id},1,lunch,other soup,Tea,1 cup,5,0,1,0,,,,"),
    ]);
    let (status, res) = import(&mut a, &body).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(error(&res).contains("line 3"), "{res}");
    assert!(error(&res).contains("inconsistent"), "{res}");
}

#[tokio::test]
async fn files_that_mix_present_and_absent_meal_ids_are_rejected() {
    let (app, _) = app().await;
    let mut a = admin(&app).await;
    let d = today(TZ).to_string();
    let id = "56565656-5656-4656-8656-565656565656";
    let body = csv(&[
        &format!("{d},{id},0,lunch,soup,Soup,1 bowl,100,1,2,3,,,,"),
        &format!("{d},,1,lunch,soup,Tea,1 cup,5,0,1,0,,,,"),
    ]);
    let (status, res) = import(&mut a, &body).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(error(&res).contains("line 3"), "{res}");
    assert!(error(&res).contains("meal_id"), "{res}");
}

#[tokio::test]
async fn unknown_meal_types_and_future_dates_are_rejected_with_a_line() {
    let (app, _) = app().await;
    let mut a = admin(&app).await;
    let d = today(TZ).to_string();
    let (status, res) = import(
        &mut a,
        &csv(&[&format!("{d},,0,brunch,soup,Soup,1 bowl,100,1,2,3,,,,")]),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(error(&res).contains("line 2"), "{res}");

    let future = today(TZ).tomorrow().unwrap().tomorrow().unwrap();
    let (status, res) = import(
        &mut a,
        &csv(&[&format!("{future},,0,lunch,soup,Soup,1 bowl,100,1,2,3,,,,")]),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(error(&res).contains("line 2"), "{res}");
    assert!(error(&res).contains("future"), "{res}");
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

#[tokio::test]
async fn a_wrong_header_is_rejected() {
    let (app, _) = app().await;
    let mut a = admin(&app).await;
    let (status, res) = import(&mut a, "date,meal\ntoday,lunch\n").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(error(&res).contains("header"), "{res}");
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
