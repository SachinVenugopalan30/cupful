// SPDX-License-Identifier: AGPL-3.0-or-later
use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

/// API error. Renders as `{"error": "<message>"}` with a matching status.
#[derive(Debug)]
pub enum Error {
    Status(StatusCode, String),
    /// Logged; the client only sees "internal error".
    Internal(String),
}

impl Error {
    pub fn bad_request(msg: impl Into<String>) -> Self {
        Error::Status(StatusCode::BAD_REQUEST, msg.into())
    }
    pub fn unauthorized() -> Self {
        Error::Status(StatusCode::UNAUTHORIZED, "not logged in".into())
    }
    pub fn conflict(msg: impl Into<String>) -> Self {
        Error::Status(StatusCode::CONFLICT, msg.into())
    }
    pub fn not_found() -> Self {
        Error::Status(StatusCode::NOT_FOUND, "not found".into())
    }
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        let (status, msg) = match self {
            Error::Status(s, m) => (s, m),
            Error::Internal(e) => {
                tracing::error!("{e}");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal error".to_string(),
                )
            }
        };
        (status, Json(json!({ "error": msg }))).into_response()
    }
}

impl From<sqlx::Error> for Error {
    fn from(e: sqlx::Error) -> Self {
        Error::Internal(e.to_string())
    }
}

impl From<tower_sessions::session::Error> for Error {
    fn from(e: tower_sessions::session::Error) -> Self {
        Error::Internal(e.to_string())
    }
}

pub type Result<T> = std::result::Result<T, Error>;
