// SPDX-License-Identifier: AGPL-3.0-or-later
use axum::http::{HeaderName, HeaderValue, StatusCode, Uri, header};
use axum::response::{IntoResponse, Response};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use sha2::{Digest, Sha256};

/// The SvelteKit build. Release builds embed it; debug builds read it from disk on each request.
#[derive(rust_embed::Embed)]
#[folder = "../web/build"]
#[allow_missing = true]
struct Web;

/// The SPA fallback has inline scripts (theme bootstrap, SvelteKit start) whose text changes per
/// build, so their hashes go into `script-src` instead of allowing `'unsafe-inline'`.
fn script_hashes(html: &str) -> String {
    let mut out = String::new();
    let mut rest = html;
    while let Some(start) = rest.find("<script>") {
        let body = &rest[start + "<script>".len()..];
        let Some(end) = body.find("</script>") else {
            break;
        };
        let digest = Sha256::digest(&body.as_bytes()[..end]);
        out.push_str(&format!(" 'sha256-{}'", STANDARD.encode(digest)));
        rest = &body[end..];
    }
    out
}

fn csp(html: &str) -> String {
    format!(
        "default-src 'self'; img-src 'self' data: blob:; style-src 'self' 'unsafe-inline'; \
         script-src 'self' 'wasm-unsafe-eval'{}; connect-src 'self'; media-src 'self' blob:; \
         frame-ancestors 'none'; base-uri 'self'; form-action 'self' https://accounts.google.com",
        script_hashes(html)
    )
}

pub async fn serve(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    if path == "api" || path.starts_with("api/") {
        return crate::error::Error::not_found().into_response();
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
    let mime = file.metadata.mimetype().to_string();
    let html = mime.starts_with("text/html");
    let csp = html.then(|| csp(&String::from_utf8_lossy(&file.data)));
    let mut res = (
        [
            (header::CONTENT_TYPE, mime),
            (header::CACHE_CONTROL, cache.to_string()),
        ],
        file.data,
    )
        .into_response();
    let h = res.headers_mut();
    let mut set = |name: HeaderName, v: &str| {
        h.insert(name, HeaderValue::from_str(v).expect("ascii header"));
    };
    set(header::X_CONTENT_TYPE_OPTIONS, "nosniff");
    set(header::REFERRER_POLICY, "same-origin");
    set(header::X_FRAME_OPTIONS, "DENY");
    if let Some(csp) = csp {
        set(header::CONTENT_SECURITY_POLICY, &csp);
    }
    res
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csp_hashes_each_inline_script_and_ignores_external_ones() {
        let html = "<script>a()</script><script src=\"x.js\"></script><script>\nb()</script>";
        let c = csp(html);
        assert_eq!(c.matches("'sha256-").count(), 2);
        assert!(c.contains("frame-ancestors 'none'"));
        assert!(!c.contains("'unsafe-inline'; connect"));
    }
}
