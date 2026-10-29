// SPDX-License-Identifier: AGPL-3.0-or-later
use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;
use std::time::Duration;

#[derive(Clone, Debug)]
pub struct Config {
    pub bind: SocketAddr,
    pub data_dir: PathBuf,
    /// `Secure` flag on the session cookie. Off only for plain-HTTP local development.
    pub secure_cookies: bool,
    /// Only requests from this address may set the client IP through `X-Forwarded-For`.
    pub trusted_proxy: Option<IpAddr>,
    /// AGPL §13: where users of this instance can get its source.
    pub source_url: String,
    /// Barcode lookups at Open Food Facts. Off: stored products and label entry only.
    pub product_lookup: bool,
    pub off_base_url: String,
    /// Total time for one Open Food Facts request; connecting is capped at 5 s.
    pub off_timeout: Duration,
    /// Hosted AI calls per user per local day (`CUPFUL_AI_DAILY_CAP`).
    pub ai_daily_cap: i64,
    /// Google Health (PLAN.md §15): the feature exists only when both are set.
    pub google_client_id: Option<String>,
    pub google_client_secret: Option<crate::crypto::Secret>,
    /// `https://<name>.<tailnet>.ts.net`; the OAuth redirect URI is this plus `/api/google/callback`.
    /// Unset: derived from the request's Host header.
    pub public_url: Option<String>,
    /// Google endpoints; not env-configurable, tests point them at wiremock.
    pub google_auth_url: String,
    pub google_token_url: String,
    pub google_revoke_url: String,
    pub google_health_url: String,
    /// Provider HTTP timeouts and retry backoff; not env-configurable, tests shorten them.
    pub ai_timeouts: crate::ai::Timeouts,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            bind: ([127, 0, 0, 1], 8080).into(),
            data_dir: ".cupful".into(),
            secure_cookies: true,
            trusted_proxy: None,
            source_url: "https://github.com/SachinVenugopalan30/cupful".into(),
            product_lookup: true,
            off_base_url: "https://world.openfoodfacts.org".into(),
            off_timeout: Duration::from_secs(10),
            ai_daily_cap: 200,
            ai_timeouts: Default::default(),
            google_client_id: None,
            google_client_secret: None,
            public_url: None,
            google_auth_url: "https://accounts.google.com/o/oauth2/v2/auth".into(),
            google_token_url: "https://oauth2.googleapis.com/token".into(),
            google_revoke_url: "https://oauth2.googleapis.com/revoke".into(),
            google_health_url: "https://health.googleapis.com/v4".into(),
        }
    }
}

impl Config {
    pub fn from_env() -> Result<Self, String> {
        let mut c = Self::default();
        let var = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());
        if let Some(v) = var("CUPFUL_BIND") {
            c.bind = v.parse().map_err(|e| format!("CUPFUL_BIND={v}: {e}"))?;
        }
        if let Some(v) = var("CUPFUL_DATA_DIR") {
            c.data_dir = v.into();
        }
        if let Some(v) = var("CUPFUL_SECURE_COOKIES") {
            c.secure_cookies = v != "false" && v != "0";
        }
        if let Some(v) = var("CUPFUL_TRUSTED_PROXY") {
            c.trusted_proxy = Some(
                v.parse()
                    .map_err(|e| format!("CUPFUL_TRUSTED_PROXY={v}: {e}"))?,
            );
        }
        if let Some(v) = var("CUPFUL_SOURCE_URL") {
            c.source_url = v;
        }
        if let Some(v) = var("CUPFUL_PRODUCT_LOOKUP") {
            c.product_lookup = v != "off" && v != "false" && v != "0";
        }
        if let Some(v) = var("CUPFUL_AI_DAILY_CAP") {
            c.ai_daily_cap = v
                .parse()
                .map_err(|e| format!("CUPFUL_AI_DAILY_CAP={v}: {e}"))?;
        }
        c.google_client_id = var("CUPFUL_GOOGLE_CLIENT_ID");
        c.google_client_secret = var("CUPFUL_GOOGLE_CLIENT_SECRET").map(crate::crypto::Secret::new);
        c.public_url = var("CUPFUL_PUBLIC_URL").map(|u| u.trim_end_matches('/').to_string());
        Ok(c)
    }
}
