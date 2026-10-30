// SPDX-License-Identifier: AGPL-3.0-or-later
//! CSRF header check, client IP, and the in-memory rate limiter.

use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use axum::extract::{ConnectInfo, FromRequestParts, Request};
use axum::http::request::Parts;
use axum::http::{Method, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use crate::AppState;
use crate::error::Error;

// ---- CSRF ----

/// Mutating API requests must carry `X-Cupful: 1`, on top of SameSite=Lax cookies. The ingest
/// route is exempt: it takes a bearer token and never reads a cookie, so there is nothing to forge.
pub async fn csrf(req: Request, next: Next) -> Response {
    let safe = matches!(*req.method(), Method::GET | Method::HEAD | Method::OPTIONS);
    if !safe
        && req.uri().path().starts_with("/api/")
        && req.uri().path() != crate::ingest::ACTIVITY_PATH
        && req.headers().get("x-cupful").is_none_or(|v| v != "1")
    {
        return Error::Status(StatusCode::FORBIDDEN, "missing X-Cupful header".into())
            .into_response();
    }
    next.run(req).await
}

// ---- client IP and rate limiting ----

/// The client's IP, when it is known. `X-Forwarded-For` is trusted only from the configured proxy,
/// and then only its rightmost entry (the one that proxy appended; the rest are client-controlled).
/// Loopback counts as unknown: behind the Tailscale sidecar every request comes from 127.0.0.1.
pub fn client_ip(
    peer: Option<IpAddr>,
    xff: Option<&str>,
    trusted: Option<IpAddr>,
) -> Option<IpAddr> {
    let peer = peer?;
    let ip = if Some(peer) == trusted {
        xff?.rsplit(',').next()?.trim().parse().ok()?
    } else {
        peer
    };
    (!ip.is_loopback()).then_some(ip)
}

pub struct ClientIp(pub Option<IpAddr>);

impl FromRequestParts<AppState> for ClientIp {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> std::result::Result<Self, Self::Rejection> {
        let peer = parts
            .extensions
            .get::<ConnectInfo<SocketAddr>>()
            .map(|c| c.0.ip());
        let xff = parts
            .headers
            .get("x-forwarded-for")
            .and_then(|v| v.to_str().ok());
        Ok(ClientIp(client_ip(peer, xff, state.config.trusted_proxy)))
    }
}

/// Sliding-window limiter: at most `limit` hits per key per `window`.
pub struct RateLimiter {
    hits: Mutex<HashMap<String, Vec<Instant>>>,
    limit: usize,
    window: Duration,
}

impl RateLimiter {
    pub fn new(limit: usize, window: Duration) -> Self {
        Self {
            hits: Mutex::default(),
            limit,
            window,
        }
    }

    /// Records a hit and says whether it's within the limit.
    pub fn allow(&self, key: &str, now: Instant) -> bool {
        let mut hits = self.hits.lock().unwrap();
        // ponytail: in-memory and per-process; fine for one binary, resets on restart.
        if hits.len() > 10_000 {
            hits.retain(|_, v| v.last().is_some_and(|t| now - *t < self.window));
        }
        let v = hits.entry(key.to_owned()).or_default();
        v.retain(|t| now - *t < self.window);
        if v.len() >= self.limit {
            return false;
        }
        v.push(now);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PROXY: &str = "10.0.0.1";

    fn ip(s: &str) -> Option<IpAddr> {
        Some(s.parse().unwrap())
    }

    #[test]
    fn no_trusted_proxy_uses_the_peer_and_ignores_xff() {
        assert_eq!(
            client_ip(ip("100.64.0.7"), Some("1.2.3.4"), None),
            ip("100.64.0.7")
        );
    }

    #[test]
    fn trusted_proxy_uses_the_rightmost_xff_entry() {
        assert_eq!(
            client_ip(ip(PROXY), Some("6.6.6.6, 203.0.113.9"), ip(PROXY)),
            ip("203.0.113.9")
        );
    }

    #[test]
    fn spoofed_leading_entry_is_ignored() {
        assert_ne!(
            client_ip(ip(PROXY), Some("6.6.6.6, 203.0.113.9"), ip(PROXY)),
            ip("6.6.6.6")
        );
    }

    #[test]
    fn untrusted_source_cannot_set_xff() {
        assert_eq!(
            client_ip(ip("198.51.100.4"), Some("203.0.113.9"), ip(PROXY)),
            ip("198.51.100.4")
        );
    }

    #[test]
    fn loopback_and_missing_peers_are_unknown() {
        assert_eq!(client_ip(ip("127.0.0.1"), None, None), None);
        assert_eq!(client_ip(None, Some("203.0.113.9"), ip(PROXY)), None);
        assert_eq!(client_ip(ip(PROXY), None, ip(PROXY)), None);
    }

    #[test]
    fn limiter_allows_five_per_window_then_recovers() {
        let l = RateLimiter::new(5, Duration::from_secs(60));
        let t0 = Instant::now();
        assert!((0..5).all(|_| l.allow("k", t0)));
        assert!(!l.allow("k", t0 + Duration::from_secs(30)));
        assert!(l.allow("other", t0));
        assert!(l.allow("k", t0 + Duration::from_secs(61)));
    }
}
