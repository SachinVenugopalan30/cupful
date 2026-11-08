// SPDX-License-Identifier: AGPL-3.0-or-later
//! The public GitHub link metadata for the nav. Stars are cached for an hour, failures too.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use axum::Json;
use axum::extract::State;
use serde_json::{Value, json};

use crate::AppState;

const REPO: &str = "SachinVenugopalan30/cupful";
const URL: &str = "https://github.com/SachinVenugopalan30/cupful";
const CACHE_SECONDS: Duration = Duration::from_secs(3600);

pub struct Stars {
    cache: Mutex<Option<(Instant, Option<i64>)>>,
    client: reqwest::Client,
}

impl Default for Stars {
    fn default() -> Self {
        Self::new()
    }
}

impl Stars {
    pub fn new() -> Self {
        Self {
            cache: Mutex::new(None),
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(5))
                .user_agent(concat!("cupful/", env!("CARGO_PKG_VERSION")))
                .build()
                .expect("build HTTP client"),
        }
    }

    async fn count(&self, api: &str) -> Option<i64> {
        if let Some((at, stars)) = self.cache.lock().ok().and_then(|c| *c)
            && at.elapsed() < CACHE_SECONDS
        {
            return stars;
        }
        let stars = self.fetch(api).await;
        if let Ok(mut cache) = self.cache.lock() {
            *cache = Some((Instant::now(), stars));
        }
        stars
    }

    async fn fetch(&self, api: &str) -> Option<i64> {
        let url = format!("{}/repos/{REPO}", api.trim_end_matches('/'));
        let response = self
            .client
            .get(url)
            .header("Accept", "application/vnd.github+json")
            .send()
            .await
            .ok()?;
        if !response.status().is_success() {
            return None;
        }
        let body: Value = serde_json::from_str(&response.text().await.ok()?).ok()?;
        body.get("stargazers_count")?.as_i64()
    }
}

pub async fn info(State(s): State<AppState>) -> Json<Value> {
    let stars = s.github.count(&s.config.github_api_url).await;
    Json(json!({"url": URL, "stars": stars}))
}
