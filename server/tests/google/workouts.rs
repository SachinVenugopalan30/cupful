// SPDX-License-Identifier: AGPL-3.0-or-later
use super::*;

#[tokio::test]
async fn workouts_follow_pagination_and_send_the_civil_filter() {
    use wiremock::matchers::{query_param, query_param_is_missing};
    let (g, _, state, mut a) = setup().await;
    link(&state, user_id(&state, "admin").await, 3600).await;
    steps_empty(&g).await;
    let pt = |i: usize| session(&format!("w{i}"), "RUNNING", day(-(i as i32 % 10)), 7, 30);
    for (token, from, next) in [("", 0, "p2"), ("p2", 25, "p3"), ("p3", 50, "")] {
        let n = if next.is_empty() { 3 } else { 25 };
        let m = Mock::given(method("GET")).and(path(EXERCISE));
        let m = if token.is_empty() {
            m.and(query_param_is_missing("pageToken"))
        } else {
            m.and(query_param("pageToken", token))
        };
        m.and(query_param("pageSize", "25"))
            .and(header("authorization", "Bearer at-old"))
            .respond_with(page((from..from + n).map(pt).collect(), next))
            .expect(1)
            .mount(&g)
            .await;
    }
    let body = synced(&mut a).await;
    assert_eq!(body["workouts"], 53);
    let reqs = g.received_requests().await.unwrap();
    let req = reqs.iter().find(|r| r.url.path() == EXERCISE).unwrap();
    assert_eq!(
        params(req.url.as_str())["filter"],
        format!(
            r#"exercise.interval.civil_start_time >= "{}" AND exercise.interval.civil_start_time < "{}""#,
            day(-13),
            day(1)
        )
    );
}

#[tokio::test]
async fn workouts_are_listed_with_labels_and_upserted_on_resync() {
    let (g, _, state, mut a) = setup().await;
    link(&state, user_id(&state, "admin").await, 3600).await;
    steps_empty(&g).await;
    let d = day(-1);
    exercise(
        &g,
        vec![
            session("run", "RUNNING", d, 7, 31),
            session("lift", "STRENGTH_TRAINING", d, 18, 45),
        ],
    )
    .await;
    synced(&mut a).await;
    let body = synced(&mut a).await;
    assert_eq!(body["workouts"], 2);
    let (_, w) = a.get(&range()).await;
    let w = w["workouts"].as_array().unwrap();
    assert_eq!(w.len(), 2, "second sync upserts, no duplicates");
    assert_eq!(w[0]["type"], "RUNNING");
    assert_eq!(w[0]["type_label"], "Running");
    assert_eq!(w[0]["date"], d.to_string());
    assert_eq!(w[0]["active_minutes"], 31.0);
    assert_eq!(w[0]["kcal"], 412.0);
    assert_eq!(w[0]["distance_m"], 5200.0);
    assert_eq!(w[0]["avg_hr"], 148);
    assert_eq!(w[0]["counts_as_gym"], false);
    assert_eq!(w[1]["counts_as_gym"], true);
    assert_eq!(w[1]["name"], "STRENGTH_TRAINING session");
    assert!(w[0]["start"].as_str().unwrap().ends_with('Z'));
    assert!(w[0]["id"].is_i64());
}

#[tokio::test]
async fn gym_minutes_sum_strength_sessions_and_ignore_runs() {
    let (g, _, state, mut a) = setup().await;
    link(&state, user_id(&state, "admin").await, 3600).await;
    steps_empty(&g).await;
    let (d, run_only) = (day(-1), day(-2));
    exercise(
        &g,
        vec![
            session("a", "STRENGTH_TRAINING", d, 7, 30),
            session("b", "WEIGHTLIFTING", d, 18, 20),
            session("c", "RUNNING", d, 12, 60),
            session("d", "RUNNING", run_only, 12, 60),
        ],
    )
    .await;
    synced(&mut a).await;
    let (_, day1) = a.get(&format!("/api/days/{d}")).await;
    assert_eq!(
        (day1["gym_minutes"].clone(), day1["gym_source"].clone()),
        (json!(50), json!("google"))
    );
    let (_, day2) = a.get(&format!("/api/days/{run_only}")).await;
    assert_eq!(day2["gym_minutes"], json!(null));
    // Idempotent.
    synced(&mut a).await;
    let (_, again) = a.get(&format!("/api/days/{d}")).await;
    assert_eq!(again["gym_minutes"], 50);
}

#[tokio::test]
async fn manual_gym_minutes_win() {
    let (g, _, state, mut a) = setup().await;
    link(&state, user_id(&state, "admin").await, 3600).await;
    steps_empty(&g).await;
    let d = day(-1);
    a.sync(vec![op(
        "day.patch",
        &at(-60),
        json!({"date": d.to_string(), "gym_minutes": 25}),
    )])
    .await;
    exercise(&g, vec![session("a", "STRENGTH_TRAINING", d, 7, 30)]).await;
    synced(&mut a).await;
    let (_, day1) = a.get(&format!("/api/days/{d}")).await;
    assert_eq!(
        (day1["gym_minutes"].clone(), day1["gym_source"].clone()),
        (json!(25), json!("manual"))
    );
}

#[tokio::test]
async fn changing_gym_types_recomputes_and_status_lists_choices() {
    let (g, _, state, mut a) = setup().await;
    link(&state, user_id(&state, "admin").await, 3600).await;
    steps_empty(&g).await;
    let d = day(-1);
    exercise(
        &g,
        vec![
            session("a", "STRENGTH_TRAINING", d, 7, 30),
            session("b", "RUNNING", d, 18, 20),
        ],
    )
    .await;
    synced(&mut a).await;
    let (_, st) = a.get("/api/google").await;
    assert!(
        st["gym_types"]
            .as_array()
            .unwrap()
            .contains(&json!("STRENGTH_TRAINING"))
    );
    let choices = st["exercise_types"].as_array().unwrap();
    assert!(choices.contains(&json!({"value": "STRENGTH_TRAINING", "label": "Strength training"})));
    for t in st["gym_types"].as_array().unwrap() {
        assert!(choices.iter().any(|c| &c["value"] == t));
    }

    let (status, body) = a
        .put(
            "/api/google/gym-types",
            json!({"types": ["STRENGTH_TRAINING", "RUNNING"]}),
        )
        .await;
    assert_eq!((status, body), (StatusCode::OK, json!({})));
    let (_, day1) = a.get(&format!("/api/days/{d}")).await;
    assert_eq!(day1["gym_minutes"], 50);
    let (_, w) = a.get(&range()).await;
    assert_eq!(w["workouts"][1]["counts_as_gym"], true);

    for bad in [
        json!({"types": ["NOPE"]}),
        json!({"types": vec!["YOGA"; 31]}),
    ] {
        let (status, _) = a.put("/api/google/gym-types", bad).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }
    let (status, _) = a.put("/api/google/gym-types", json!({"types": []})).await;
    assert_eq!(status, StatusCode::OK);
    let (_, st) = a.get("/api/google").await;
    assert_eq!(st["gym_types"], json!([]));
}

#[tokio::test]
async fn exercise_failure_keeps_steps_and_missing_scope_asks_to_reconnect() {
    let (g, _, state, mut a) = setup().await;
    link(&state, user_id(&state, "admin").await, 3600).await;
    Mock::given(method("POST"))
        .and(path(format!("/v4{ROLLUP}")))
        .respond_with(rollups(&[(day(-1), "500")]))
        .mount(&g)
        .await;
    Mock::given(method("GET"))
        .and(path(EXERCISE))
        .respond_with(ResponseTemplate::new(500))
        .up_to_n_times(1)
        .mount(&g)
        .await;
    let body = synced(&mut a).await;
    assert_eq!(body["changed"], 1);
    assert_eq!(body["workouts"], 0);
    assert!(body["workouts_error"].is_string());
    let (_, st) = a.get("/api/google").await;
    assert!(st["last_error"].is_string());
    assert_eq!(st["needs_reconnect"], false);

    Mock::given(method("GET"))
        .and(path(EXERCISE))
        .respond_with(ResponseTemplate::new(403))
        .mount(&g)
        .await;
    let (status, body) = a.post("/api/google/sync", json!({})).await;
    assert_eq!(
        (status, &body["code"]),
        (StatusCode::CONFLICT, &json!("reconnect"))
    );
    let (_, st) = a.get("/api/google").await;
    assert_eq!(st["needs_reconnect"], true);
    let (_, d) = a.get(&format!("/api/days/{}", day(-1))).await;
    assert_eq!(
        d["steps"], 500,
        "steps are stored despite the exercise error"
    );
}

#[tokio::test]
async fn workouts_survive_disconnect_and_list_without_google() {
    let (g, _, state, mut a) = setup().await;
    link(&state, user_id(&state, "admin").await, 3600).await;
    steps_empty(&g).await;
    exercise(&g, vec![session("a", "RUNNING", day(-1), 7, 30)]).await;
    synced(&mut a).await;
    Mock::given(method("POST"))
        .and(path("/revoke"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&g)
        .await;
    a.req("DELETE", "/api/google", None).await;
    let (_, w) = a.get(&range()).await;
    assert_eq!(w["workouts"].as_array().unwrap().len(), 1);

    let (app, _) = common::app().await;
    let mut c = admin(&app).await;
    let (status, w) = c.get(&range()).await;
    assert_eq!((status, w), (StatusCode::OK, json!({"workouts": []})));
    let (status, _) = c.put("/api/google/gym-types", json!({"types": []})).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn b_cannot_see_a_workouts_or_gym_types() {
    let (g, app, state, mut a) = setup().await;
    link(&state, user_id(&state, "admin").await, 3600).await;
    steps_empty(&g).await;
    exercise(&g, vec![session("a", "STRENGTH_TRAINING", day(-1), 7, 30)]).await;
    synced(&mut a).await;
    let mut b = user(&app, &state, "bob").await;
    b.put("/api/profile", profile_body()).await;
    let (_, w) = b.get(&range()).await;
    assert_eq!(w["workouts"], json!([]));
    b.put("/api/google/gym-types", json!({"types": []})).await;
    let (_, st) = a.get("/api/google").await;
    assert_ne!(st["gym_types"], json!([]), "B's setting is B's own");
}

#[tokio::test]
async fn workouts_without_a_google_name_are_kept_and_not_duplicated() {
    // The docs don't say exercise points carry `name`; losing every workout over it would be silent.
    let (g, _, state, mut a) = setup().await;
    link(&state, user_id(&state, "admin").await, 3600).await;
    steps_empty(&g).await;
    let d = day(-1);
    let mut nameless = session("x", "STRENGTH_TRAINING", d, 7, 40);
    nameless.as_object_mut().unwrap().remove("name");
    exercise(&g, vec![nameless]).await;
    assert_eq!(synced(&mut a).await["workouts"], 1);
    assert_eq!(synced(&mut a).await["workouts"], 1);
    let (_, w) = a.get(&range()).await;
    assert_eq!(
        w["workouts"].as_array().unwrap().len(),
        1,
        "a second sync upserts"
    );
    let (_, day) = a.get(&format!("/api/days/{d}")).await;
    assert_eq!(day["gym_minutes"], json!(40));
}

#[tokio::test]
async fn stored_types_become_pickable_and_treadmill_is_a_default() {
    let (g, _, state, mut a) = setup().await;
    link(&state, user_id(&state, "admin").await, 3600).await;
    steps_empty(&g).await;
    let d = day(-1);
    exercise(
        &g,
        vec![
            session("a", "TREADMILL", d, 7, 30),
            session("b", "CARPENTRY", d, 9, 20),
        ],
    )
    .await;
    synced(&mut a).await;
    let (_, st) = a.get("/api/google").await;
    assert!(
        st["gym_types"]
            .as_array()
            .unwrap()
            .contains(&json!("TREADMILL"))
    );
    let labels: Vec<&str> = st["exercise_types"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["label"].as_str().unwrap())
        .collect();
    assert!(labels.contains(&"Carpentry"), "stored type is offered");
    assert!(labels.windows(2).all(|w| w[0] <= w[1]), "sorted by label");
    let (_, day1) = a.get(&format!("/api/days/{d}")).await;
    assert_eq!(day1["gym_minutes"], 30);
    let (status, _) = a
        .put(
            "/api/google/gym-types",
            json!({"types": ["TREADMILL", "CARPENTRY"]}),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let (_, day1) = a.get(&format!("/api/days/{d}")).await;
    assert_eq!(day1["gym_minutes"], 50);
    let (status, _) = a
        .put("/api/google/gym-types", json!({"types": ["NOT_A_TYPE"]}))
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn unticking_a_type_clears_google_gym_minutes_but_never_manual() {
    let (g, _, state, mut a) = setup().await;
    link(&state, user_id(&state, "admin").await, 3600).await;
    steps_empty(&g).await;
    let (run_day, manual_day) = (day(0), day(-2)); // the profile starts today, so budget works
    a.sync(vec![op(
        "day.patch",
        &at(-60),
        json!({"date": manual_day.to_string(), "gym_minutes": 25}),
    )])
    .await;
    exercise(
        &g,
        vec![
            session("a", "RUNNING", run_day, 7, 60),
            session("b", "RUNNING", manual_day, 7, 40),
        ],
    )
    .await;
    synced(&mut a).await;
    let (_, d) = a.get(&format!("/api/days/{run_day}")).await;
    assert_eq!(d["gym_minutes"], json!(null));

    let tick = json!({"types": ["RUNNING"]});
    a.put("/api/google/gym-types", tick.clone()).await;
    let (_, d) = a.get(&format!("/api/days/{run_day}")).await;
    assert_eq!(
        (d["gym_minutes"].clone(), d["gym_source"].clone()),
        (json!(60), json!("google"))
    );
    let (_, b) = a.get(&format!("/api/budget/{run_day}")).await;
    assert!(b["gym_kcal"].as_f64().unwrap_or(-1.0) > 0.0, "{b}");
    a.put("/api/google/gym-types", tick).await;
    let (_, d) = a.get(&format!("/api/days/{run_day}")).await;
    assert_eq!(d["gym_minutes"], 60);

    a.put("/api/google/gym-types", json!({"types": []})).await;
    let (_, d) = a.get(&format!("/api/days/{run_day}")).await;
    assert_eq!(
        (d["gym_minutes"].clone(), d["gym_source"].clone()),
        (json!(null), json!(null))
    );
    let (_, b) = a.get(&format!("/api/budget/{run_day}")).await;
    assert_eq!(b["gym_kcal"].as_f64().unwrap_or(0.0), 0.0, "{b}");
    let (_, m) = a.get(&format!("/api/days/{manual_day}")).await;
    assert_eq!(
        (m["gym_minutes"].clone(), m["gym_source"].clone()),
        (json!(25), json!("manual"))
    );
    a.put("/api/google/gym-types", json!({"types": []})).await;
    let (_, d) = a.get(&format!("/api/days/{run_day}")).await;
    assert_eq!(d["gym_minutes"], json!(null));
}
