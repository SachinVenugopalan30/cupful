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
        Ok(c)
    }
}
