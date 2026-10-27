// SPDX-License-Identifier: AGPL-3.0-or-later
use axum::http::{StatusCode, Uri, header};
use axum::response::{IntoResponse, Response};

/// The SvelteKit build. Release builds embed it; debug builds read it from disk on each request.
#[derive(rust_embed::Embed)]
#[folder = "../web/build"]
#[allow_missing = true]
struct Web;

pub async fn serve(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    if path == "api" || path.starts_with("api/") {
        return crate::error::Error::NotFound.into_response();
    }
    let (file, path) = match Web::get(path) {
        Some(f) if !path.is_empty() => (f, path),
        _ => match Web::get("index.html") {
            Some(f) => (f, "index.html"),
            None => return (StatusCode::NOT_FOUND, "frontend not built").into_response(),
        },
    };
    let cache = if path.starts_with("_app/immutable/") {
        "public, max-age=31536000, immutable"
    } else {
        "no-cache"
    };
    (
        [
            (header::CONTENT_TYPE, file.metadata.mimetype().to_string()),
            (header::CACHE_CONTROL, cache.to_string()),
        ],
        file.data,
    )
        .into_response()
}
