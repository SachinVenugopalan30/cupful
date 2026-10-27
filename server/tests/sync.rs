// SPDX-License-Identifier: AGPL-3.0-or-later
mod common;
use axum::http::StatusCode;
use common::*;
use serde_json::json;

const TZ: &str = "Asia/Kolkata";

async fn meals(c: &mut Client, date: &str) -> serde_json::Value {
    let (status, body) = c.get(&format!("/api/meals?date={date}")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body["meals"].clone()
}

#[tokio::test]
async fn upsert_then_read_back_with_totals() {
    let (app, _) = app().await;
    let mut a = admin(&app).await;
    let d = today(TZ).to_string();
    let id = uuid();
    let res = a
        .sync(vec![op(
            "meal.upsert",
            &at(0),
            meal(&id, &d, json!([item(&uuid(), 200.0), item(&uuid(), 100.0)])),
        )])
        .await;
    assert_eq!(res, ["applied"]);
    let m = meals(&mut a, &d).await;
    assert_eq!(m[0]["id"], id.as_str());
    assert_eq!(m[0]["items"].as_array().unwrap().len(), 2);
    assert_eq!(m[0]["totals"]["kcal"], 300.0);
}

#[tokio::test]
async fn replaying_an_op_is_a_duplicate_and_changes_nothing() {
    let (app, _) = app().await;
    let mut a = admin(&app).await;
    let d = today(TZ).to_string();
    let o = op(
        "meal.upsert",
        &at(0),
        meal(&uuid(), &d, json!([item(&uuid(), 200.0)])),
    );
    assert_eq!(a.sync(vec![o.clone()]).await, ["applied"]);
    assert_eq!(a.sync(vec![o]).await, ["duplicate"]);
    assert_eq!(meals(&mut a, &d).await.as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn resending_an_upsert_with_a_new_op_id_updates_rather_than_duplicates() {
    let (app, _) = app().await;
    let mut a = admin(&app).await;
    let d = today(TZ).to_string();
    let (id, item_id) = (uuid(), uuid());
    a.sync(vec![op(
        "meal.upsert",
        &at(-10),
        meal(&id, &d, json!([item(&item_id, 200.0)])),
    )])
    .await;
    let res = a
        .sync(vec![op(
            "meal.upsert",
            &at(-5),
            meal(&id, &d, json!([item(&item_id, 250.0)])),
        )])
        .await;
    assert_eq!(res, ["applied"]);
    let m = meals(&mut a, &d).await;
    assert_eq!(m.as_array().unwrap().len(), 1);
    assert_eq!(m[0]["totals"]["kcal"], 250.0);
}

#[tokio::test]
async fn items_are_replaced_not_merged() {
    let (app, _) = app().await;
    let mut a = admin(&app).await;
    let d = today(TZ).to_string();
    let (id, keep) = (uuid(), uuid());
    a.sync(vec![op(
        "meal.upsert",
        &at(-10),
        meal(&id, &d, json!([item(&keep, 200.0), item(&uuid(), 150.0)])),
    )])
    .await;
    a.sync(vec![op(
        "meal.upsert",
        &at(-5),
        meal(&id, &d, json!([item(&keep, 200.0)])),
    )])
    .await;
    let m = meals(&mut a, &d).await;
    assert_eq!(m[0]["items"].as_array().unwrap().len(), 1);
    assert_eq!(m[0]["totals"]["kcal"], 200.0);
}

#[tokio::test]
async fn stored_portion_multiplier_is_never_reapplied() {
    let (app, _) = app().await;
    let mut a = admin(&app).await;
    let d = today(TZ).to_string();
    let mut m = meal(&uuid(), &d, json!([item(&uuid(), 300.0)]));
    m["scale"] = json!(1.5); // the client already multiplied the item: 200 kcal x 1.5
    a.sync(vec![op("meal.upsert", &at(0), m)]).await;
    let got = meals(&mut a, &d).await;
    assert_eq!(got[0]["scale"], 1.5);
    assert_eq!(got[0]["items"][0]["kcal"], 300.0);
    assert_eq!(got[0]["totals"]["kcal"], 300.0);
}

#[tokio::test]
async fn older_meal_write_is_stale() {
    let (app, _) = app().await;
    let mut a = admin(&app).await;
    let d = today(TZ).to_string();
    let id = uuid();
    a.sync(vec![op(
        "meal.upsert",
        &at(-10),
        meal(&id, &d, json!([item(&uuid(), 200.0)])),
    )])
    .await;
    let res = a
        .sync(vec![op(
            "meal.upsert",
            &at(-60),
            meal(&id, &d, json!([item(&uuid(), 999.0)])),
        )])
        .await;
    assert_eq!(res, ["stale"]);
    assert_eq!(meals(&mut a, &d).await[0]["totals"]["kcal"], 200.0);
}

#[tokio::test]
async fn timestamps_far_in_the_future_are_rejected_and_near_ones_clamped() {
    let (app, _) = app().await;
    let mut a = admin(&app).await;
    let d = today(TZ).to_string();
    let res = a
        .sync(vec![op(
            "meal.upsert",
            &at(6 * 60),
            meal(&uuid(), &d, json!([item(&uuid(), 1.0)])),
        )])
        .await;
    assert_eq!(res, ["rejected"]);

    // 4 minutes ahead is accepted, but its effective time is clamped to receipt,
    // so a write stamped a minute later (in real time) still wins.
    let id = uuid();
    assert_eq!(
        a.sync(vec![op(
            "meal.upsert",
            &at(4 * 60),
            meal(&id, &d, json!([item(&uuid(), 1.0)]))
        )])
        .await,
        ["applied"]
    );
    tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    assert_eq!(
        a.sync(vec![op(
            "meal.upsert",
            &at(0),
            meal(&id, &d, json!([item(&uuid(), 2.0)]))
        )])
        .await,
        ["applied"]
    );
}

#[tokio::test]
async fn timestamps_older_than_tombstone_retention_are_stale() {
    let (app, _) = app().await;
    let mut a = admin(&app).await;
    let d = today(TZ).to_string();
    let res = a
        .sync(vec![op(
            "meal.upsert",
            &at(-181 * 86400),
            meal(&uuid(), &d, json!([item(&uuid(), 1.0)])),
        )])
        .await;
    assert_eq!(res, ["stale"]);
}

#[tokio::test]
async fn delete_leaves_a_tombstone_that_beats_later_upserts() {
    let (app, _) = app().await;
    let mut a = admin(&app).await;
    let d = today(TZ).to_string();
    let id = uuid();
    a.sync(vec![op(
        "meal.upsert",
        &at(-30),
        meal(&id, &d, json!([item(&uuid(), 200.0)])),
    )])
    .await;
    assert_eq!(
        a.sync(vec![op("meal.delete", &at(-20), json!({"id": id}))])
            .await,
        ["applied"]
    );
    assert_eq!(meals(&mut a, &d).await.as_array().unwrap().len(), 0);
    let res = a
        .sync(vec![op(
            "meal.upsert",
            &at(-10),
            meal(&id, &d, json!([item(&uuid(), 200.0)])),
        )])
        .await;
    assert_eq!(res, ["stale"]);
    assert_eq!(meals(&mut a, &d).await.as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn delete_older_than_the_last_edit_is_stale() {
    let (app, _) = app().await;
    let mut a = admin(&app).await;
    let d = today(TZ).to_string();
    let id = uuid();
    a.sync(vec![op(
        "meal.upsert",
        &at(-10),
        meal(&id, &d, json!([item(&uuid(), 200.0)])),
    )])
    .await;
    assert_eq!(
        a.sync(vec![op("meal.delete", &at(-60), json!({"id": id}))])
            .await,
        ["stale"]
    );
    assert_eq!(meals(&mut a, &d).await.as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn day_fields_resolve_last_write_wins_independently() {
    let (app, _) = app().await;
    let mut a = admin(&app).await;
    let d = today(TZ).to_string();
    // Phone A sets weight at t-10; phone B's older steps (t-60) still apply: different field.
    assert_eq!(
        a.sync(vec![op(
            "day.patch",
            &at(-10),
            json!({"date": d, "weight_kg": 81.4})
        )])
        .await,
        ["applied"]
    );
    assert_eq!(
        a.sync(vec![op(
            "day.patch",
            &at(-60),
            json!({"date": d, "steps": 9000})
        )])
        .await,
        ["applied"]
    );
    // An older weight loses.
    assert_eq!(
        a.sync(vec![op(
            "day.patch",
            &at(-60),
            json!({"date": d, "weight_kg": 90.0})
        )])
        .await,
        ["stale"]
    );
    let (_, day) = a.get(&format!("/api/days/{d}")).await;
    assert_eq!(day["weight_kg"], 81.4);
    assert_eq!(day["steps"], 9000);
    assert_eq!(day["steps_source"], "manual");
}

#[tokio::test]
async fn day_patch_null_clears_a_field() {
    let (app, _) = app().await;
    let mut a = admin(&app).await;
    let d = today(TZ).to_string();
    a.sync(vec![op(
        "day.patch",
        &at(-10),
        json!({"date": d, "weight_kg": 81.4, "trained": ["Chest", "Arms"]}),
    )])
    .await;
    a.sync(vec![op(
        "day.patch",
        &at(-5),
        json!({"date": d, "weight_kg": null}),
    )])
    .await;
    let (_, day) = a.get(&format!("/api/days/{d}")).await;
    assert_eq!(day["weight_kg"], json!(null));
    assert_eq!(day["trained"], json!(["Chest", "Arms"]));
}

#[tokio::test]
async fn manual_values_beat_automated_sources() {
    let (app, state) = app().await;
    let mut a = admin(&app).await;
    let d = today(TZ);
    let ds = d.to_string();
    a.sync(vec![op(
        "day.patch",
        &at(-60),
        json!({"date": ds, "steps": 5000}),
    )])
    .await;
    let now = jiff::Timestamp::now().as_millisecond();
    let patch = |steps: i64| cupful::sync::DayPatch {
        steps: Some(Some(steps)),
        gym_minutes: Some(Some(30)),
        ..Default::default()
    };
    let applied = cupful::sync::patch_day(
        &state.db,
        1,
        d,
        &patch(12000),
        cupful::sync::Source::Shortcut,
        now,
    )
    .await
    .unwrap();
    assert!(applied, "gym minutes had no manual value, so they apply");
    let (_, day) = a.get(&format!("/api/days/{ds}")).await;
    assert_eq!(day["steps"], 5000);
    assert_eq!(day["steps_source"], "manual");
    assert_eq!(day["gym_minutes"], 30);
    assert_eq!(day["gym_source"], "shortcut");

    // Among automated sources, the latest wins.
    cupful::sync::patch_day(
        &state.db,
        1,
        d,
        &cupful::sync::DayPatch {
            gym_minutes: Some(Some(45)),
            ..Default::default()
        },
        cupful::sync::Source::Import,
        now + 1,
    )
    .await
    .unwrap();
    let (_, day) = a.get(&format!("/api/days/{ds}")).await;
    assert_eq!(day["gym_minutes"], 45);
    assert_eq!(day["gym_source"], "import");
}

#[tokio::test]
async fn dates_after_tomorrow_are_rejected() {
    let (app, _) = app().await;
    let mut a = admin(&app).await;
    let t = today(TZ);
    let tomorrow = t.tomorrow().unwrap().to_string();
    let later = t.tomorrow().unwrap().tomorrow().unwrap().to_string();
    let res = a
        .sync(vec![
            op("day.patch", &at(0), json!({"date": tomorrow, "steps": 1})),
            op("day.patch", &at(0), json!({"date": later, "steps": 1})),
            op(
                "meal.upsert",
                &at(0),
                meal(&uuid(), &later, json!([item(&uuid(), 1.0)])),
            ),
        ])
        .await;
    assert_eq!(res, ["applied", "rejected", "rejected"]);
}

#[tokio::test]
async fn invalid_ops_are_rejected_individually() {
    let (app, _) = app().await;
    let mut a = admin(&app).await;
    let d = today(TZ).to_string();
    let mut neg = meal(&uuid(), &d, json!([item(&uuid(), -5.0)]));
    neg["meal_type"] = json!("lunch");
    let mut bad_type = meal(&uuid(), &d, json!([]));
    bad_type["meal_type"] = json!("brunch");
    let (status, body) = a
        .post(
            "/api/sync",
            json!({"ops": [
                op("meal.upsert", &at(0), neg),
                op("meal.upsert", &at(0), bad_type),
                op("meal.upsert", &at(0), meal("not-a-uuid", &d, json!([]))),
                op("day.patch", &at(0), json!({"date": d, "steps": 200000})),
                op("day.patch", &at(0), json!({"date": d, "trained": ["Toes"]})),
                op("meal.explode", &at(0), json!({})),
                op("day.patch", &at(0), json!({"date": d, "steps": 1234})),
            ]}),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let r = body["results"].as_array().unwrap();
    for x in &r[..6] {
        assert_eq!(x["status"], "rejected", "{x}");
        assert!(x["error"].as_str().is_some_and(|e| !e.is_empty()));
    }
    assert_eq!(r[6]["status"], "applied");
}

#[tokio::test]
async fn at_most_100_ops_per_request() {
    let (app, _) = app().await;
    let mut a = admin(&app).await;
    let d = today(TZ).to_string();
    let ops: Vec<_> = (0..101)
        .map(|_| op("day.patch", &at(0), json!({"date": d, "steps": 1})))
        .collect();
    let (status, _) = a.post("/api/sync", json!({ "ops": ops })).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}
