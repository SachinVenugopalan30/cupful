// SPDX-License-Identifier: AGPL-3.0-or-later
//! AI provider interface (PLAN.md §9). Adapters, settings, secrets and the daily cap live behind
//! `for_user` / `by_id`; the nutrition pipeline only sees `Provider`.

mod anthropic;
mod gemini;
mod http;
mod openai_compatible;
pub mod settings;

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};

use http::MAX_TOKENS;

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

impl ProviderError {
    /// Full message with upstream detail. Admin-only (Test connection); users get `Error::from`.
    pub fn detail(&self) -> String {
        match self {
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
            // Upstream detail (internal hosts, response bodies) stays out of user-facing errors.
            ProviderError::Unreachable(_) => "the AI provider is unreachable".to_string(),
            ProviderError::Auth(_) => "the AI provider rejected its credentials".to_string(),
            _ => e.detail(),
        };
        if matches!(
            e,
            ProviderError::Unparseable(_)
                | ProviderError::Other(_)
                | ProviderError::Unreachable(_)
                | ProviderError::Auth(_)
        ) {
            // The detail can echo the user's meal or internal URLs: debug only, like raw replies.
            tracing::warn!(code = e.code(), "AI provider call failed");
            tracing::debug!("{}", e.detail());
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
    Arc::new(http::HttpProvider {
        cfg,
        http,
        timeouts,
    })
}

pub const MAX_IMAGE_BYTES: usize = 5 * 1024 * 1024;

fn data_base64(img: &Image) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(&img.bytes)
}

fn join_url(base: &str, path: &str) -> String {
    format!("{}/{path}", base.trim_end_matches('/'))
}
