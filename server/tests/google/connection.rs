// SPDX-License-Identifier: AGPL-3.0-or-later
use super::*;

#[tokio::test]
async fn hidden_when_not_configured() {
    let (app, _) = common::app().await;
    let mut a = admin(&app).await;
    let (status, body) = a.get("/api/google").await;
    assert_eq!(
        (status, body),
        (StatusCode::OK, json!({"configured": false}))
    );
    for (m, p) in [
        ("GET", "/api/google/connect"),
        ("GET", "/api/google/callback?code=x&state=y"),
        ("POST", "/api/google/sync"),
        ("DELETE", "/api/google"),
    ] {
        let (status, _) = a.req(m, p, None).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{m} {p}");
    }
}

#[tokio::test]
async fn connect_redirects_with_pkce_state_scope_and_offline() {
    let (g, _, _, mut a) = setup().await;
    let (status, loc) = nav(&mut a, "/api/google/connect").await;
    assert!(status.is_redirection(), "{status}");
    let loc = loc.unwrap();
    assert!(loc.starts_with(&format!("{}/auth?", g.uri())));
    let p = params(&loc);
    assert_eq!(p["client_id"], "cid.apps.googleusercontent.com");
    assert_eq!(p["redirect_uri"], "https://cupful.test/api/google/callback");
    assert_eq!(p["response_type"], "code");
    assert_eq!(p["scope"], cupful::google::SCOPE);
    assert_eq!(p["code_challenge_method"], "S256");
    assert_eq!(p["code_challenge"].len(), 43);
    assert_eq!(p["access_type"], "offline");
    assert_eq!(p["prompt"], "consent");
    assert!(p["state"].len() >= 32);
    assert!(!loc.contains("shh-secret"));
}

#[tokio::test]
async fn callback_rejects_missing_or_wrong_state() {
    let (g, _, _, mut a) = setup().await;
    token_ok(&g).await;
    // No sign-in started.
    let (status, _) = a.get("/api/google/callback?code=c&state=s").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    // Started, but the state differs.
    nav(&mut a, "/api/google/connect").await;
    let (status, _) = a.get("/api/google/callback?code=c&state=forged").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    // A failed attempt used up the pending sign-in: even the right state is gone.
    let (status, _) = a.get("/api/google/callback?code=c").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (_, st) = a.get("/api/google").await;
    assert_eq!(st["connected"], false);
    assert!(g.received_requests().await.unwrap().is_empty());
}

#[tokio::test]
async fn callback_exchanges_code_with_verifier_and_stores_encrypted_tokens() {
    let (g, _, state, mut a) = setup().await;
    token_ok(&g).await;
    let (_, loc) = nav(&mut a, "/api/google/connect").await;
    let p = params(&loc.unwrap());
    let (status, loc) = nav(
        &mut a,
        &format!("/api/google/callback?code=the-code&state={}", p["state"]),
    )
    .await;
    assert!(status.is_redirection());
    assert_eq!(loc.unwrap(), "/settings?google=connected");

    let reqs = g.received_requests().await.unwrap();
    let body = String::from_utf8(reqs[0].body.clone()).unwrap();
    let form: std::collections::HashMap<String, String> =
        reqwest::Url::parse(&format!("http://x/?{body}"))
            .unwrap()
            .query_pairs()
            .map(|(k, v)| (k.into_owned(), v.into_owned()))
            .collect();
    assert_eq!(form["grant_type"], "authorization_code");
    assert_eq!(form["code"], "the-code");
    assert_eq!(form["client_secret"], "shh-secret");
    assert_eq!(
        form["redirect_uri"],
        "https://cupful.test/api/google/callback"
    );
    // The verifier hashes to the challenge sent in the first leg.
    assert_eq!(
        B64URL.encode(Sha256::digest(form["code_verifier"].as_bytes())),
        p["code_challenge"]
    );

    let (refresh, access): (String, String) =
        sqlx::query_as("SELECT refresh_token, access_token FROM google_connections")
            .fetch_one(&state.db)
            .await
            .unwrap();
    assert!(refresh.starts_with("v1:") && !refresh.contains("rt-new"));
    assert!(access.starts_with("v1:") && !access.contains("at-new"));
    assert_eq!(state.secrets.decrypt(&refresh).unwrap(), "rt-new");

    let (_, st) = a.get("/api/google").await;
    assert_eq!(st["configured"], true);
    assert_eq!(st["connected"], true);
    assert_eq!(st["needs_reconnect"], false);

    // The state is one-time.
    let (status, _) = a
        .get(&format!("/api/google/callback?code=x&state={}", p["state"]))
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn callback_uses_the_redirect_uri_from_connect_not_the_callback_host() {
    let g = MockServer::start().await;
    let (app, _) = app_with(Config {
        product_lookup: false,
        google_client_id: Some("cid".into()),
        google_client_secret: Some(Secret::new("shh-secret".into())),
        google_auth_url: format!("{}/auth", g.uri()),
        google_token_url: format!("{}/token", g.uri()),
        ..Config::default()
    })
    .await;
    let mut a = admin(&app).await;
    token_ok(&g).await;
    let get = |uri: String, host: &'static str, ck: Option<String>| {
        let mut b = Request::get(uri).header("host", host);
        if let Some(ck) = ck {
            b = b.header("cookie", ck);
        }
        b.body(Body::empty()).unwrap()
    };
    let (_, h, _) = send(
        &app,
        get("/api/google/connect".into(), "a.test", a.cookie.clone()),
    )
    .await;
    let p = params(h.get("location").unwrap().to_str().unwrap());
    assert!(p["redirect_uri"].contains("a.test"));
    if let Some(sc) = h.get("set-cookie") {
        a.cookie = Some(sc.to_str().unwrap().split(';').next().unwrap().to_string());
    }
    let uri = format!("/api/google/callback?code=c&state={}", p["state"]);
    send(&app, get(uri, "b.test", a.cookie.clone())).await;
    let body = String::from_utf8(g.received_requests().await.unwrap()[0].body.clone()).unwrap();
    let sent = &params(&format!("http://x/?{body}"))["redirect_uri"];
    assert_eq!(sent, &p["redirect_uri"]);
}

#[tokio::test]
async fn invalid_grant_marks_reconnect_and_stops_retrying() {
    let (g, _, state, mut a) = setup().await;
    link(&state, user_id(&state, "admin").await, -10).await;
    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(ResponseTemplate::new(400).set_body_json(
            json!({"error": "invalid_grant", "error_description": "Token has been expired or revoked."}),
        ))
        .expect(1)
        .mount(&g)
        .await;
    let (status, body) = a.post("/api/google/sync", json!({})).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body["code"], "reconnect");
    let (_, st) = a.get("/api/google").await;
    assert_eq!(st["needs_reconnect"], true);
    assert!(st["last_error"].as_str().unwrap().contains("reconnect"));
    // No second call to Google.
    let (status, _) = a.post("/api/google/sync", json!({})).await;
    assert_eq!(status, StatusCode::CONFLICT);
}

#[tokio::test]
async fn missing_scope_asks_to_reconnect() {
    let (g, _, state, mut a) = setup().await;
    link(&state, user_id(&state, "admin").await, 3600).await;
    Mock::given(method("POST"))
        .and(path(format!("/v4{ROLLUP}")))
        .respond_with(ResponseTemplate::new(403).set_body_json(
            json!({"error": {"status": "PERMISSION_DENIED", "details": [{"reason": "MISSING_OAUTH_SCOPE"}]}}),
        ))
        .mount(&g)
        .await;
    let (status, body) = a.post("/api/google/sync", json!({})).await;
    assert_eq!(
        (status, &body["code"]),
        (StatusCode::CONFLICT, &json!("reconnect"))
    );
    let (_, st) = a.get("/api/google").await;
    assert_eq!(st["needs_reconnect"], true);
}

#[tokio::test]
async fn disconnect_revokes_and_deletes() {
    let (g, _, state, mut a) = setup().await;
    link(&state, user_id(&state, "admin").await, 3600).await;
    Mock::given(method("POST"))
        .and(path("/revoke"))
        .and(body_string_contains("token=rt-old"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&g)
        .await;
    let (status, _) = a.req("DELETE", "/api/google", None).await;
    assert_eq!(status, StatusCode::OK);
    let (_, st) = a.get("/api/google").await;
    assert_eq!(st["connected"], false);
    let left: i64 = sqlx::query_scalar("SELECT count(*) FROM google_connections")
        .fetch_one(&state.db)
        .await
        .unwrap();
    assert_eq!(left, 0);
}

// ---- isolation (PLAN.md §10) ----

#[tokio::test]
async fn b_cannot_see_touch_or_sync_a_connection() {
    let (g, app, state, mut a) = setup().await;
    link(&state, user_id(&state, "admin").await, 3600).await;
    let mut b = user(&app, &state, "bob").await;
    b.put("/api/profile", profile_body()).await;

    let (_, st) = b.get("/api/google").await;
    assert_eq!(
        (st["configured"].clone(), st["connected"].clone()),
        (json!(true), json!(false))
    );
    let (status, _) = b.post("/api/google/sync", json!({})).await;
    assert_eq!(status, StatusCode::CONFLICT, "bob has nothing to sync");
    let (status, _) = b.req("DELETE", "/api/google", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    // B's callback can't use A's pending sign-in: sessions are separate.
    let (_, loc) = nav(&mut a, "/api/google/connect").await;
    let state_a = params(&loc.unwrap())["state"].clone();
    let (status, _) = b
        .get(&format!("/api/google/callback?code=x&state={state_a}"))
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    // B's connect works and issues its own state.
    let (status, loc) = nav(&mut b, "/api/google/connect").await;
    assert!(status.is_redirection());
    assert_ne!(params(&loc.unwrap())["state"], state_a);

    let (_, st) = a.get("/api/google").await;
    assert_eq!(st["connected"], true);
    assert!(g.received_requests().await.unwrap().is_empty());
}
