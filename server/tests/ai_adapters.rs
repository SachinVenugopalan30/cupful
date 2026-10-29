// SPDX-License-Identifier: AGPL-3.0-or-later
//! The three adapters against wiremock (PLAN.md §9). Every scenario runs once per adapter.
use std::sync::Arc;
use std::time::{Duration, Instant};

use cupful::ai::{
    self, Image, JsonRequest, Kind, Mode, Provider, ProviderConfig, ProviderError, Timeouts,
};
use cupful::crypto::Secret;
use serde_json::{Value, json};
use wiremock::matchers::{body_partial_json, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const KEY: &str = "sk-test-0123456789abcdefghijklmnop";
const KINDS: [Kind; 3] = [Kind::OpenaiCompatible, Kind::Anthropic, Kind::Gemini];

fn timeouts() -> Timeouts {
    Timeouts {
        connect: Duration::from_secs(1),
        text: Duration::from_millis(300),
        vision: Duration::from_millis(300),
        backoff: Duration::from_millis(5),
    }
}

/// Where each adapter posts, relative to the mock server, and the header carrying the key.
fn route(kind: Kind) -> (&'static str, &'static str, &'static str, String) {
    match kind {
        Kind::OpenaiCompatible => (
            "/v1",
            "/v1/chat/completions",
            "authorization",
            format!("Bearer {KEY}"),
        ),
        Kind::Anthropic => ("/v1", "/v1/messages", "x-api-key", KEY.into()),
        Kind::Gemini => (
            "/v1beta",
            "/v1beta/models/m:generateContent",
            "x-goog-api-key",
            KEY.into(),
        ),
    }
}

/// A successful provider response whose reply text is `text`.
fn reply(kind: Kind, text: &str) -> ResponseTemplate {
    let body = match kind {
        Kind::OpenaiCompatible => json!({"choices": [{"message": {"content": text}}]}),
        Kind::Anthropic => json!({"content": [{"type": "text", "text": text}]}),
        Kind::Gemini => json!({"candidates": [{"content": {"parts": [{"text": text}]}}]}),
    };
    ResponseTemplate::new(200).set_body_json(body)
}

/// The structured-output field each adapter sends in native mode.
fn native_field(kind: Kind) -> Value {
    match kind {
        Kind::OpenaiCompatible => {
            json!({"temperature": 0, "max_tokens": 4096, "response_format": {"type": "json_schema"}})
        }
        Kind::Anthropic => {
            json!({"max_tokens": 4096, "output_config": {"format": {"type": "json_schema"}}})
        }
        Kind::Gemini => json!({"generationConfig": {
            "temperature": 0, "maxOutputTokens": 4096, "responseMimeType": "application/json",
            "responseJsonSchema": {"type": "object"}
        }}),
    }
}

async fn provider(server: &MockServer, kind: Kind, mode: Mode) -> Arc<dyn Provider> {
    let (base, ..) = route(kind);
    ai::build(
        ProviderConfig {
            kind,
            base_url: format!("{}{base}", server.uri()),
            model: "m".into(),
            api_key: Some(Secret::new(KEY.into())),
            extra_headers: vec![],
            vision: true,
            mode,
        },
        reqwest::Client::new(),
        timeouts(),
    )
}

fn request() -> JsonRequest {
    JsonRequest {
        instruction: "Estimate.".into(),
        user_text: "rice".into(),
        images: vec![],
        schema: json!({"type": "object", "properties": {"ok": {"type": "boolean"}}, "required": ["ok"]}),
    }
}

#[tokio::test]
async fn success_sends_key_and_structured_output_settings() {
    for kind in KINDS {
        let server = MockServer::start().await;
        let (_, p, h, v) = route(kind);
        Mock::given(method("POST"))
            .and(path(p))
            .and(header(h, v.as_str()))
            .and(body_partial_json(native_field(kind)))
            .respond_with(reply(kind, r#"{"ok": true}"#))
            .expect(1)
            .mount(&server)
            .await;
        let got = provider(&server, kind, Mode::Native)
            .await
            .complete_json(request())
            .await;
        assert_eq!(got, Ok(json!({"ok": true})), "{kind:?}");
    }
}

#[tokio::test]
async fn malformed_reply_is_retried_once_with_the_error() {
    for kind in KINDS {
        let server = MockServer::start().await;
        let (_, p, ..) = route(kind);
        Mock::given(path(p))
            .respond_with(reply(kind, "not json"))
            .up_to_n_times(1)
            .mount(&server)
            .await;
        Mock::given(path(p))
            .respond_with(reply(kind, r#"{"ok": false}"#))
            .mount(&server)
            .await;
        let got = provider(&server, kind, Mode::Native)
            .await
            .complete_json(request())
            .await;
        assert_eq!(got, Ok(json!({"ok": false})), "{kind:?}");
        let reqs = server.received_requests().await.unwrap();
        assert_eq!(reqs.len(), 2, "{kind:?}");
        let second = String::from_utf8_lossy(&reqs[1].body);
        assert!(second.contains("could not be used"), "{kind:?}: {second}");
    }
}

#[tokio::test]
async fn still_malformed_after_the_retry_is_unparseable() {
    for kind in KINDS {
        let server = MockServer::start().await;
        let (_, p, ..) = route(kind);
        Mock::given(path(p))
            .respond_with(reply(kind, r#"{"wrong": 1}"#))
            .expect(2)
            .mount(&server)
            .await;
        let got = provider(&server, kind, Mode::Native)
            .await
            .complete_json(request())
            .await;
        assert!(
            matches!(got, Err(ProviderError::Unparseable(_))),
            "{kind:?}: {got:?}"
        );
    }
}

#[tokio::test]
async fn auth_error_is_not_retried_and_hides_the_key() {
    for kind in KINDS {
        let server = MockServer::start().await;
        let (_, p, ..) = route(kind);
        Mock::given(path(p))
            .respond_with(
                ResponseTemplate::new(401)
                    .set_body_string(format!("invalid api key {KEY}, try again")),
            )
            .expect(1)
            .mount(&server)
            .await;
        let got = provider(&server, kind, Mode::Native)
            .await
            .complete_json(request())
            .await;
        let Err(ProviderError::Auth(msg)) = got else {
            panic!("{kind:?}: {got:?}")
        };
        assert!(
            !msg.contains(KEY) && msg.contains("invalid api key"),
            "{msg}"
        );
    }
}

#[tokio::test]
async fn rate_limit_waits_for_retry_after_then_succeeds() {
    for kind in KINDS {
        let server = MockServer::start().await;
        let (_, p, ..) = route(kind);
        Mock::given(path(p))
            .respond_with(ResponseTemplate::new(429).insert_header("retry-after", "1"))
            .up_to_n_times(1)
            .mount(&server)
            .await;
        Mock::given(path(p))
            .respond_with(reply(kind, r#"{"ok": true}"#))
            .mount(&server)
            .await;
        let t0 = Instant::now();
        let got = provider(&server, kind, Mode::Native)
            .await
            .complete_json(request())
            .await;
        assert_eq!(got, Ok(json!({"ok": true})), "{kind:?}");
        assert!(t0.elapsed() >= Duration::from_secs(1), "{kind:?}");
    }
}

#[tokio::test]
async fn persistent_rate_limit_gives_up_after_two_retries() {
    for kind in KINDS {
        let server = MockServer::start().await;
        let (_, p, ..) = route(kind);
        Mock::given(path(p))
            .respond_with(ResponseTemplate::new(429))
            .expect(3)
            .mount(&server)
            .await;
        let got = provider(&server, kind, Mode::Native)
            .await
            .complete_json(request())
            .await;
        assert_eq!(got, Err(ProviderError::RateLimited), "{kind:?}");
    }
}

#[tokio::test]
async fn slow_provider_times_out() {
    for kind in KINDS {
        let server = MockServer::start().await;
        let (_, p, ..) = route(kind);
        Mock::given(path(p))
            .respond_with(reply(kind, r#"{"ok": true}"#).set_delay(Duration::from_secs(2)))
            .mount(&server)
            .await;
        let got = provider(&server, kind, Mode::Native)
            .await
            .complete_json(request())
            .await;
        assert_eq!(got, Err(ProviderError::Timeout), "{kind:?}");
    }
}

#[tokio::test]
async fn server_errors_are_retried_and_scrubbed_of_keys() {
    for kind in KINDS {
        let server = MockServer::start().await;
        let (_, p, ..) = route(kind);
        let other = "AIzaSyD-abcdefghijklmnopqrstuvwxyz012345";
        Mock::given(path(p))
            .respond_with(
                ResponseTemplate::new(500)
                    .set_body_string(format!("boom key={KEY} other={other} sk-proj-abc123def")),
            )
            .expect(3)
            .mount(&server)
            .await;
        let got = provider(&server, kind, Mode::Native)
            .await
            .complete_json(request())
            .await;
        let Err(ProviderError::Other(msg)) = got else {
            panic!("{kind:?}: {got:?}")
        };
        assert!(msg.contains("boom"), "{msg}");
        for secret in [KEY, other, "sk-proj-abc123def"] {
            assert!(!msg.contains(secret), "{kind:?} leaked {secret}: {msg}");
        }
    }
}

#[tokio::test]
async fn prompt_mode_extracts_the_first_json_object() {
    for kind in KINDS {
        let server = MockServer::start().await;
        let (_, p, ..) = route(kind);
        Mock::given(path(p))
            .respond_with(reply(
                kind,
                "Sure! ```json\n{\"ok\": true} ``` and {\"more\": 1}",
            ))
            .mount(&server)
            .await;
        let prov = provider(&server, kind, Mode::Prompt).await;
        assert!(!prov.capabilities().native_json_schema);
        assert_eq!(
            prov.complete_json(request()).await,
            Ok(json!({"ok": true})),
            "{kind:?}"
        );
        let body =
            String::from_utf8_lossy(&server.received_requests().await.unwrap()[0].body).to_string();
        assert!(body.contains("only one JSON object"), "{kind:?}");
        assert!(
            !body.contains("response_format")
                && !body.contains("output_config")
                && !body.contains("responseJsonSchema"),
            "{kind:?}: {body}"
        );
    }
}

#[tokio::test]
async fn images_are_sent_and_oversized_ones_rejected_without_a_request() {
    for kind in KINDS {
        let server = MockServer::start().await;
        let (_, p, ..) = route(kind);
        Mock::given(path(p))
            .respond_with(reply(kind, r#"{"ok": true}"#))
            .expect(1)
            .mount(&server)
            .await;
        let prov = provider(&server, kind, Mode::Native).await;
        let mut req = request();
        req.images = vec![Image {
            bytes: b"jpegbytes".to_vec(),
            media_type: "image/jpeg".into(),
        }];
        assert_eq!(
            prov.complete_json(req.clone()).await,
            Ok(json!({"ok": true}))
        );
        let body =
            String::from_utf8_lossy(&server.received_requests().await.unwrap()[0].body).to_string();
        assert!(
            body.contains("anBlZ2J5dGVz"),
            "{kind:?}: base64 image missing"
        );
        req.images[0].bytes = vec![0; 5 * 1024 * 1024 + 1];
        assert!(matches!(
            prov.complete_json(req).await,
            Err(ProviderError::Other(_))
        ));
    }
}

#[tokio::test]
async fn unreachable_provider_is_reported() {
    let prov = ai::build(
        ProviderConfig {
            kind: Kind::OpenaiCompatible,
            base_url: "http://127.0.0.1:9/v1".into(),
            model: "m".into(),
            api_key: None,
            extra_headers: vec![],
            vision: false,
            mode: Mode::Native,
        },
        reqwest::Client::new(),
        timeouts(),
    );
    let got = prov.complete_json(request()).await;
    assert!(matches!(got, Err(ProviderError::Unreachable(_))), "{got:?}");
}

#[tokio::test]
async fn extra_headers_are_sent() {
    let server = MockServer::start().await;
    Mock::given(path("/openai/v1/chat/completions"))
        .and(header("api-key", "azure-secret"))
        .respond_with(reply(Kind::OpenaiCompatible, r#"{"ok": true}"#))
        .expect(1)
        .mount(&server)
        .await;
    let prov = ai::build(
        ProviderConfig {
            kind: Kind::OpenaiCompatible,
            base_url: format!("{}/openai/v1/", server.uri()),
            model: "m".into(),
            api_key: None,
            extra_headers: vec![("api-key".into(), Secret::new("azure-secret".into()))],
            vision: false,
            mode: Mode::Native,
        },
        reqwest::Client::new(),
        timeouts(),
    );
    assert_eq!(prov.complete_json(request()).await, Ok(json!({"ok": true})));
}
