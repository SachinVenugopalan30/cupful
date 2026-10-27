// SPDX-License-Identifier: AGPL-3.0-or-later
//! AI provider interface (PLAN.md §9). Adapters, settings, secrets and the daily cap live behind
//! `for_user` / `by_id`; the nutrition pipeline only sees `Provider`.

use std::sync::Arc;

use async_trait::async_trait;
use axum::http::StatusCode;

use crate::AppState;
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
    _state: &AppState,
    _user_id: i64,
    _purpose: Purpose,
) -> Result<(i64, Arc<dyn Provider>), ProviderError> {
    Err(ProviderError::NotConfigured) // M2 track A implements this.
}

/// A configured provider by id, uncapped (for `cupful eval` and Test connection).
pub async fn by_id(
    _state: &AppState,
    _provider_id: i64,
) -> Result<Arc<dyn Provider>, ProviderError> {
    Err(ProviderError::NotConfigured) // M2 track A implements this.
}
