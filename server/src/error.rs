// SPDX-License-Identifier: AGPL-3.0-or-later
use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

/// API error. Renders as `{"error": "<message>"}` with a matching status.
#[derive(Debug)]
pub enum Error {
    NotFound,
    Internal(String),
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        let (status, msg) = match self {
            Error::NotFound => (StatusCode::NOT_FOUND, "not found".to_string()),
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

pub type Result<T> = std::result::Result<T, Error>;
