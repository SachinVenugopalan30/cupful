// SPDX-License-Identifier: AGPL-3.0-or-later
//! AI provider interface (PLAN.md §9). Adapters, settings, secrets and the daily cap live behind
//! `for_user` / `by_id`; the nutrition pipeline only sees `Provider`.

mod anthropic;
mod gemini;
mod openai_compatible;
pub mod settings;

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::AppState;
use crate::crypto::Secret;
use crate::error::Error;

/// One structured-output request: instruction + user text (+ images), reply must match `schema`.
#[derive(Clone, Debug)]
pub struct JsonRequest {
    pub instruction: String,
    pub user_text: String,
    pub images: Vec<Image>,
    /// JSON Schema of the expected reply.
    pub schema: serde_json::Value,
}

#[derive(Clone, Debug)]
pub struct Image {
    pub bytes: Vec<u8>,
    /// `image/jpeg`, `image/png` or `image/webp`.
    pub media_type: String,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Capabilities {
    pub vision: bool,
    pub native_json_schema: bool,
}

#[async_trait]
pub trait Provider: Send + Sync {
    /// Returns the reply as JSON. Implementations validate it parses; callers validate the shape.
    async fn complete_json(&self, req: JsonRequest) -> Result<serde_json::Value, ProviderError>;
    fn capabilities(&self) -> Capabilities;
}

/// The failures the UI must tell apart (PLAN.md §9 "Errors the UI must handle").
#[derive(Debug, PartialEq)]
pub enum ProviderError {
    NotConfigured,
    Timeout,
    Unreachable(String),
    Unparseable(String),
    RateLimited,
    DailyCapReached,
    Auth(String),
    Other(String),
}

impl ProviderError {
    pub fn code(&self) -> &'static str {
        match self {
            ProviderError::NotConfigured => "no_provider",
            ProviderError::Timeout => "timeout",
            ProviderError::Unreachable(_) => "unreachable",
            ProviderError::Unparseable(_) => "unparseable",
            ProviderError::RateLimited => "rate_limited",
            ProviderError::DailyCapReached => "daily_cap",
            ProviderError::Auth(_) => "provider_auth",
            ProviderError::Other(_) => "provider_error",
        }
    }
}

impl From<ProviderError> for Error {
    fn from(e: ProviderError) -> Self {
        let status = match e {
            ProviderError::NotConfigured => StatusCode::CONFLICT,
            ProviderError::Timeout => StatusCode::GATEWAY_TIMEOUT,
            ProviderError::RateLimited | ProviderError::DailyCapReached => {
                StatusCode::TOO_MANY_REQUESTS
            }
            _ => StatusCode::BAD_GATEWAY,
        };
        let msg = match &e {
            ProviderError::NotConfigured => "no AI provider is configured".to_string(),
            ProviderError::Timeout => "the AI provider timed out".to_string(),
            ProviderError::Unreachable(m) => format!("the AI provider is unreachable: {m}"),
            ProviderError::Unparseable(m) => {
                format!("the AI provider's reply could not be read: {m}")
            }
            ProviderError::RateLimited => "the AI provider is rate limiting requests".to_string(),
            ProviderError::DailyCapReached => "today's AI call limit is reached".to_string(),
            ProviderError::Auth(m) => format!("the AI provider rejected its credentials: {m}"),
            ProviderError::Other(m) => m.clone(),
        };
        if matches!(e, ProviderError::Unparseable(_) | ProviderError::Other(_)) {
            tracing::warn!("{msg}");
        }
        Error::Coded(status, e.code(), msg)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Purpose {
    Text,
    Vision,
}

/// The user's default provider for `purpose`, with the daily cap enforced and usage counted on each
/// call. Returns the provider's id too, for `meals.provider_id`.
pub async fn for_user(
    state: &AppState,
    user_id: i64,
    purpose: Purpose,
) -> Result<(i64, Arc<dyn Provider>), ProviderError> {
    let vision = purpose == Purpose::Vision;
    let id = sqlx::query_scalar!(
        "SELECT id FROM ai_providers
         WHERE CASE WHEN ? THEN is_default_vision ELSE is_default_text END",
        vision
    )
    .fetch_optional(&state.db)
    .await
    .map_err(internal)?
    .ok_or(ProviderError::NotConfigured)?;
    let date = crate::time::user_today(&state.db, user_id)
        .await
        .map_err(|_| ProviderError::Other("could not read your timezone".into()))?
        .to_string();
    // One statement, so concurrent calls can't both take the last slot. Local providers skip the cap.
    let counted = sqlx::query!(
        "INSERT INTO ai_usage (user_id, date, provider_id, calls)
         SELECT ?1, ?2, ?3, 1
         WHERE (SELECT is_local FROM ai_providers WHERE id = ?3)
            OR (SELECT COALESCE(SUM(u.calls), 0) FROM ai_usage u
                JOIN ai_providers p ON p.id = u.provider_id
                WHERE u.user_id = ?1 AND u.date = ?2 AND p.is_local = 0) < ?4
         ON CONFLICT (user_id, date, provider_id) DO UPDATE SET calls = calls + 1",
        user_id,
        date,
        id,
        state.config.ai_daily_cap
    )
    .execute(&state.db)
    .await
    .map_err(internal)?
    .rows_affected();
    if counted == 0 {
        return Err(ProviderError::DailyCapReached);
    }
    Ok((id, by_id(state, id).await?))
}

/// A configured provider by id, uncapped (for `cupful eval` and Test connection).
pub async fn by_id(state: &AppState, provider_id: i64) -> Result<Arc<dyn Provider>, ProviderError> {
    let cfg = settings::load(state, provider_id)
        .await?
        .ok_or(ProviderError::NotConfigured)?;
    Ok(build(cfg, state.ai_http.clone(), state.config.ai_timeouts))
}

fn internal(e: sqlx::Error) -> ProviderError {
    tracing::error!("{e}");
    ProviderError::Other("internal error".into())
}

// ---- adapters ----

#[derive(Clone, Copy, Debug)]
pub struct Timeouts {
    pub connect: Duration,
    /// Total time for one request without images, and with them.
    pub text: Duration,
    pub vision: Duration,
    /// First retry delay without `Retry-After`; doubles each time, plus up to as much jitter.
    pub backoff: Duration,
}

impl Default for Timeouts {
    fn default() -> Self {
        Self {
            connect: Duration::from_secs(10),
            text: Duration::from_secs(60),
            vision: Duration::from_secs(120),
            backoff: Duration::from_millis(500),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    OpenaiCompatible,
    Anthropic,
    Gemini,
}

/// `Native`: the provider's schema-constrained output. `Prompt`: ask for only JSON and extract it.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Native,
    Prompt,
}

#[derive(Clone, Debug)]
pub struct ProviderConfig {
    pub kind: Kind,
    pub base_url: String,
    pub model: String,
    pub api_key: Option<Secret>,
    pub extra_headers: Vec<(String, Secret)>,
    pub vision: bool,
    pub mode: Mode,
}

pub fn build(cfg: ProviderConfig, http: reqwest::Client, timeouts: Timeouts) -> Arc<dyn Provider> {
    Arc::new(HttpProvider {
        cfg,
        http,
        timeouts,
    })
}

pub const MAX_IMAGE_BYTES: usize = 5 * 1024 * 1024;
// Reasoning models (gpt-oss, o-series) spend part of this thinking before the JSON.
const MAX_TOKENS: u32 = 4096;
const RETRIES: u32 = 2;
/// A longer `Retry-After` than this fails fast as rate limited instead of holding the request.
const MAX_RETRY_AFTER: Duration = Duration::from_secs(30);

struct HttpProvider {
    cfg: ProviderConfig,
    http: reqwest::Client,
    timeouts: Timeouts,
}

#[async_trait]
impl Provider for HttpProvider {
    async fn complete_json(&self, mut req: JsonRequest) -> Result<Value, ProviderError> {
        if req.images.iter().any(|i| i.bytes.len() > MAX_IMAGE_BYTES) {
            return Err(ProviderError::Other("the photo is larger than 5 MB".into()));
        }
        if self.cfg.mode == Mode::Prompt {
            req.instruction.push_str(&format!(
                "\n\nReply with only one JSON object that matches this JSON Schema, and no other text:\n{}",
                req.schema
            ));
        }
        let err = match parse(&self.call(&req).await?, &req.schema) {
            Ok(v) => return Ok(v),
            Err(e) => e,
        };
        req.user_text.push_str(&format!(
            "\n\nYour previous reply could not be used ({err}). Reply again with only the JSON object."
        ));
        parse(&self.call(&req).await?, &req.schema).map_err(ProviderError::Unparseable)
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            vision: self.cfg.vision,
            native_json_schema: self.cfg.mode == Mode::Native,
        }
    }
}

impl HttpProvider {
    /// One request, retried on 429 and 5xx; returns the reply text.
    async fn call(&self, req: &JsonRequest) -> Result<String, ProviderError> {
        let c = &self.cfg;
        let (url, body) = match c.kind {
            Kind::OpenaiCompatible => openai_compatible::request(c, req),
            Kind::Anthropic => anthropic::request(c, req),
            Kind::Gemini => gemini::request(c, req),
        };
        let timeout = if req.images.is_empty() {
            self.timeouts.text
        } else {
            self.timeouts.vision
        };
        let body = serde_json::to_vec(&body).expect("JSON values serialize");
        let mut attempt = 0;
        loop {
            let mut rb = self
                .http
                .post(&url)
                .timeout(timeout)
                .header("content-type", "application/json")
                .body(body.clone());
            rb = match (c.kind, &c.api_key) {
                (_, None) => rb,
                (Kind::OpenaiCompatible, Some(k)) => rb.bearer_auth(k.expose()),
                (Kind::Anthropic, Some(k)) => rb.header("x-api-key", k.expose()),
                (Kind::Gemini, Some(k)) => rb.header("x-goog-api-key", k.expose()),
            };
            if c.kind == Kind::Anthropic {
                rb = rb.header("anthropic-version", "2023-06-01");
            }
            for (k, v) in &c.extra_headers {
                rb = rb.header(k, v.expose());
            }
            let res = rb.send().await.map_err(|e| self.http_error(e))?;
            let status = res.status();
            if status.is_success() {
                let bytes = res.bytes().await.map_err(|e| self.http_error(e))?;
                let v: Value = serde_json::from_slice(&bytes).unwrap_or_default();
                let text = match c.kind {
                    Kind::OpenaiCompatible => openai_compatible::reply_text(&v),
                    Kind::Anthropic => anthropic::reply_text(&v),
                    Kind::Gemini => gemini::reply_text(&v),
                };
                return text.ok_or_else(|| {
                    let body = String::from_utf8_lossy(&bytes);
                    ProviderError::Unparseable(format!("unexpected reply: {}", self.scrub(&body)))
                });
            }
            let retry_after = res
                .headers()
                .get(reqwest::header::RETRY_AFTER)
                .and_then(|v| v.to_str().ok()?.trim().parse().ok())
                .map(Duration::from_secs);
            let text = self.scrub(&res.text().await.unwrap_or_default());
            let retryable = status.as_u16() == 429 || status.is_server_error();
            let wait = retry_after.unwrap_or_else(|| backoff(self.timeouts.backoff, attempt));
            if retryable && attempt < RETRIES && wait <= MAX_RETRY_AFTER {
                tracing::warn!("AI provider answered {status}; retrying in {wait:?}");
                tokio::time::sleep(wait).await;
                attempt += 1;
                continue;
            }
            return Err(match status.as_u16() {
                401 | 403 => ProviderError::Auth(text),
                429 => ProviderError::RateLimited,
                _ => ProviderError::Other(format!("the AI provider answered {status}: {text}")),
            });
        }
    }

    fn http_error(&self, e: reqwest::Error) -> ProviderError {
        if e.is_timeout() {
            ProviderError::Timeout
        } else if e.is_connect() {
            ProviderError::Unreachable(self.scrub(&e.to_string()))
        } else {
            ProviderError::Other(self.scrub(&e.to_string()))
        }
    }

    /// Removes this provider's secrets and anything key-shaped, and caps the length.
    fn scrub(&self, s: &str) -> String {
        let mut s = s.to_string();
        let secrets = self.cfg.api_key.iter();
        for secret in secrets.chain(self.cfg.extra_headers.iter().map(|(_, v)| v)) {
            if !secret.expose().is_empty() {
                s = s.replace(secret.expose(), "***");
            }
        }
        let s = scrub_tokens(&s);
        match s.char_indices().nth(500) {
            Some((i, _)) => format!("{}…", &s[..i]),
            None => s,
        }
    }
}

/// `base * 2^attempt` plus up to `base` of jitter.
fn backoff(base: Duration, attempt: u32) -> Duration {
    // ponytail: clock-derived jitter, enough to spread a few retries; use an RNG if it ever matters.
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    base * 2u32.pow(attempt) + base.mul_f64(f64::from(nanos % 1000) / 1000.0)
}

/// Replaces tokens that look like API keys: known prefixes, or 32+ characters mixing letters and digits.
fn scrub_tokens(s: &str) -> String {
    const PREFIXES: [&str; 6] = ["sk-", "sk_", "AIza", "xai-", "gsk_", "hf_"];
    let looks_like_key = |t: &str| {
        (t.len() >= 12 && PREFIXES.iter().any(|p| t.starts_with(p)))
            // A long unbroken random run; hyphenated model names ("gemma-4-12b-...") are short words.
            || t.split(['-', '_']).any(|seg| {
                seg.len() >= 24
                    && seg.chars().any(|c| c.is_ascii_digit())
                    && seg.chars().any(|c| c.is_ascii_alphabetic())
            })
    };
    let mut out = String::with_capacity(s.len());
    let mut token = String::new();
    for c in s.chars().map(Some).chain([None]) {
        if let Some(c) = c.filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_') {
            token.push(c);
            continue;
        }
        out.push_str(if looks_like_key(&token) {
            "***"
        } else {
            &token
        });
        token.clear();
        out.extend(c);
    }
    out
}

/// The first JSON object in `text`, with the schema's top-level `required` fields present.
fn parse(text: &str, schema: &Value) -> Result<Value, String> {
    let start = text.find('{').ok_or("the reply has no JSON object")?;
    let v = serde_json::Deserializer::from_str(&text[start..])
        .into_iter::<Value>()
        .next()
        .ok_or("the reply has no JSON object")?
        .map_err(|e| format!("invalid JSON: {e}"))?;
    let required = schema["required"].as_array().into_iter().flatten();
    if let Some(k) = required
        .filter_map(Value::as_str)
        .find(|k| v.get(k).is_none())
    {
        return Err(format!("missing field {k:?}"));
    }
    Ok(v)
}

fn data_base64(img: &Image) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(&img.bytes)
}

fn join_url(base: &str, path: &str) -> String {
    format!("{}/{path}", base.trim_end_matches('/'))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn scrubs_key_shaped_tokens_but_keeps_words() {
        let s = scrub_tokens(
            "bad key sk-abcdefghijkl1 for gpt-4.1-mini, id 0123456789abcdef0123456789abcdef.",
        );
        assert_eq!(s, "bad key *** for gpt-4.1-mini, id ***.");
    }

    #[test]
    fn keeps_long_hyphenated_model_names() {
        let name = "Model 'gemma-4-12b-coder-fable5-composer2.5-4bit' not found";
        assert_eq!(scrub_tokens(name), name);
        assert_eq!(
            scrub_tokens("token abc-0123456789abcdef0123456789abcdef-x"),
            "token ***"
        );
    }

    #[test]
    fn parse_takes_the_first_object_and_checks_required() {
        let schema = json!({"required": ["a"]});
        assert_eq!(
            parse("x {\"a\": 1} {\"b\": 2}", &schema),
            Ok(json!({"a": 1}))
        );
        assert!(parse("{\"b\": 1}", &schema).is_err());
        assert!(parse("nothing", &schema).is_err());
    }
}
