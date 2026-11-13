// SPDX-License-Identifier: AGPL-3.0-or-later
use super::*;

#[tokio::test]
async fn sync_maps_rollups_to_dates_and_writes_source_google() {
    let (g, _, state, mut a) = setup().await;
    link(&state, user_id(&state, "admin").await, 3600).await;
    let (y, t) = (day(-1), day(0));
    Mock::given(method("POST"))
        .and(path(format!("/v4{ROLLUP}")))
        .and(header("authorization", "Bearer at-old"))
        .respond_with(rollups(&[(y, "8123"), (t, "4000")]))
        .expect(1)
        .mount(&g)
        .await;
    exercise(&g, vec![]).await;
    let (status, body) = a.post("/api/google/sync", json!({})).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["days"], 2);
    assert_eq!(body["changed"], 2);
    assert_eq!(body["workouts"], 0);

    let (_, d) = a.get(&format!("/api/days/{y}")).await;
    assert_eq!(
        (d["steps"].clone(), d["steps_source"].clone()),
        (json!(8123), json!("google"))
    );
    let (_, d) = a.get(&format!("/api/days/{t}")).await;
    assert_eq!(d["steps"], 4000);

    // 14 civil days ending today, closed-open.
    let req = &g.received_requests().await.unwrap()[0];
    let sent: Value = serde_json::from_slice(&req.body).unwrap();
    let (s, e) = (day(-13), day(1));
    assert_eq!(
        sent["range"],
        json!({
            "start": {"date": {"year": s.year(), "month": s.month(), "day": s.day()}},
            "end": {"date": {"year": e.year(), "month": e.month(), "day": e.day()}},
        })
    );
    assert_eq!(sent["windowSizeDays"], 1);

    let (_, st) = a.get("/api/google").await;
    assert!(st["last_sync_at"].is_string());
    assert_eq!(st["last_error"], json!(null));
}

#[tokio::test]
async fn sync_never_overwrites_manual_steps() {
    let (g, _, state, mut a) = setup().await;
    link(&state, user_id(&state, "admin").await, 3600).await;
    let y = day(-1);
    a.sync(vec![op(
        "day.patch",
        &at(-60),
        json!({"date": y.to_string(), "steps": 1234}),
    )])
    .await;
    Mock::given(method("POST"))
        .and(path(format!("/v4{ROLLUP}")))
        .respond_with(rollups(&[(y, "9999")]))
        .mount(&g)
        .await;
    let (_, body) = a.post("/api/google/sync", json!({})).await;
    assert_eq!(body["changed"], 0);
    let (_, d) = a.get(&format!("/api/days/{y}")).await;
    assert_eq!(
        (d["steps"].clone(), d["steps_source"].clone()),
        (json!(1234), json!("manual"))
    );
}

#[tokio::test]
async fn expired_access_token_is_refreshed() {
    let (g, _, state, mut a) = setup().await;
    link(&state, user_id(&state, "admin").await, -10).await;
    Mock::given(method("POST"))
        .and(path("/token"))
        .and(body_string_contains("grant_type=refresh_token"))
        .and(body_string_contains("refresh_token=rt-old"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"access_token": "at-fresh", "expires_in": 3599})),
        )
        .expect(1)
        .mount(&g)
        .await;
    Mock::given(method("POST"))
        .and(path(format!("/v4{ROLLUP}")))
        .and(header("authorization", "Bearer at-fresh"))
        .respond_with(rollups(&[(day(0), "10")]))
        .expect(2)
        .mount(&g)
        .await;
    let (status, body) = a.post("/api/google/sync", json!({})).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    // The new token is stored (encrypted) and reused without another refresh.
    let (status, _) = a.post("/api/google/sync", json!({})).await;
    assert_eq!(status, StatusCode::OK);
}
