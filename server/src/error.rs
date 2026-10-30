// SPDX-License-Identifier: AGPL-3.0-or-later
use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

/// API error. Renders as `{"error": "<message>"}` with a matching status.
#[derive(Debug)]
pub enum Error {
    Status(StatusCode, String),
    /// Like `Status`, plus a stable machine-readable `code` for the UI.
    Coded(StatusCode, &'static str, String),
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
            Error::Coded(s, code, m) => {
                return (s, Json(json!({ "error": m, "code": code }))).into_response();
            }
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

/// `axum::Json` and `axum::extract::Query`, but rejections render as our JSON errors.
pub struct JsonBody<T>(pub T);
pub struct Query<T>(pub T);
pub struct TextBody(pub String);
#[allow(dead_code)]
pub struct BytesBody(pub axum::body::Bytes);

impl<S: Send + Sync, T: serde::de::DeserializeOwned> axum::extract::FromRequest<S> for JsonBody<T> {
    type Rejection = Error;

    async fn from_request(req: axum::extract::Request, state: &S) -> Result<Self> {
        axum::Json::from_request(req, state)
            .await
            .map(|axum::Json(v)| JsonBody(v))
            .map_err(|e| Error::Status(e.status(), e.body_text()))
    }
}

impl<S: Send + Sync, T: serde::de::DeserializeOwned> axum::extract::FromRequestParts<S>
    for Query<T>
{
    type Rejection = Error;

    async fn from_request_parts(parts: &mut axum::http::request::Parts, state: &S) -> Result<Self> {
        axum::extract::Query::from_request_parts(parts, state)
            .await
            .map(|axum::extract::Query(v)| Query(v))
            .map_err(|e| Error::Status(e.status(), e.body_text()))
    }
}

impl<S: Send + Sync> axum::extract::FromRequest<S> for TextBody {
    type Rejection = Error;

    async fn from_request(req: axum::extract::Request, state: &S) -> Result<Self> {
        let bytes = axum::body::Bytes::from_request(req, state)
            .await
            .map_err(|e| Error::Status(e.status(), e.body_text()))?;
        String::from_utf8(bytes.to_vec())
            .map(TextBody)
            .map_err(|_| Error::bad_request("body must be UTF-8 text"))
    }
}

impl<S: Send + Sync> axum::extract::FromRequest<S> for BytesBody {
    type Rejection = Error;

    async fn from_request(req: axum::extract::Request, state: &S) -> Result<Self> {
        axum::body::Bytes::from_request(req, state)
            .await
            .map(BytesBody)
            .map_err(|e| Error::Status(e.status(), e.body_text()))
    }
}
