// SPDX-License-Identifier: AGPL-3.0-or-later
//! The shared HTTP driver behind every adapter: retries, timeouts, scrubbing, reply parsing.

use std::time::Duration;

use async_trait::async_trait;
use serde_json::Value;

use super::{
    Capabilities, JsonRequest, Kind, MAX_IMAGE_BYTES, Mode, Provider, ProviderConfig,
    ProviderError, Timeouts, anthropic, gemini, openai_compatible,
};

// Reasoning models (gpt-oss, o-series) spend part of this thinking before the JSON.
pub(super) const MAX_TOKENS: u32 = 4096;
const RETRIES: u32 = 2;
/// A longer `Retry-After` than this fails fast as rate limited instead of holding the request.
const MAX_RETRY_AFTER: Duration = Duration::from_secs(30);

pub(super) struct HttpProvider {
    pub(super) cfg: ProviderConfig,
    pub(super) http: reqwest::Client,
    pub(super) timeouts: Timeouts,
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
