// SPDX-License-Identifier: AGPL-3.0-or-later
//! Provider settings (PLAN.md §9): admin-only online REST, env seeding, loading with secrets.
//! Keys never leave the server; responses say `has_key`.

use std::collections::BTreeMap;

use axum::Json;
use axum::extract::{Path, State};
use axum::http::header::{HeaderName, HeaderValue};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{Value, json};

use super::{JsonRequest, Kind, Mode, ProviderConfig, ProviderError};
use crate::AppState;
use crate::auth::Admin;
use crate::crypto::Secret;
use crate::error::{Error, JsonBody, Result};

/// `snake_case` serde name of a unit enum, for the TEXT columns.
fn name<T: Serialize>(v: T) -> String {
    serde_json::to_value(v)
        .unwrap()
        .as_str()
        .unwrap()
        .to_owned()
}

fn from_name<T: for<'de> Deserialize<'de>>(s: &str) -> T {
    serde_json::from_value(json!(s)).expect("CHECK constraint keeps this valid")
}

/// The provider with its secrets decrypted, or `None` if there is no such provider.
pub async fn load(
    state: &AppState,
    id: i64,
) -> std::result::Result<Option<ProviderConfig>, ProviderError> {
    let Some(r) = sqlx::query!(
        "SELECT kind, base_url, model, api_key, extra_headers, supports_vision,
                structured_output_mode FROM ai_providers WHERE id = ?",
        id
    )
    .fetch_optional(&state.db)
    .await
    .map_err(super::internal)?
    else {
        return Ok(None);
    };
    let decrypt = |v: &str| {
        state.secrets.decrypt(v).map_err(|e| {
            ProviderError::Other(format!("could not decrypt this provider's secrets ({e})"))
        })
    };
    let api_key = r.api_key.as_deref().map(decrypt).transpose()?;
    let headers: BTreeMap<String, String> = match r.extra_headers.as_deref() {
        Some(v) => serde_json::from_str(&decrypt(v)?).unwrap_or_default(),
        None => BTreeMap::new(),
    };
    Ok(Some(ProviderConfig {
        kind: from_name(&r.kind),
        base_url: r.base_url,
        model: r.model,
        api_key: api_key.map(Secret::new),
        extra_headers: headers
            .into_iter()
            .map(|(k, v)| (k, Secret::new(v)))
            .collect(),
        vision: r.supports_vision != 0,
        mode: from_name(&r.structured_output_mode),
    }))
}

// ---- REST ----

/// Present-but-null vs absent: `None` keeps the stored value, `Some(None)` clears it.
fn tri<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    d: D,
) -> std::result::Result<Option<Option<T>>, D::Error> {
    Option::<T>::deserialize(d).map(Some)
}

#[derive(Deserialize)]
pub struct ProviderIn {
    label: String,
    kind: Kind,
    base_url: String,
    model: String,
    #[serde(default, deserialize_with = "tri")]
    api_key: Option<Option<String>>,
    #[serde(default, deserialize_with = "tri")]
    extra_headers: Option<Option<BTreeMap<String, String>>>,
    #[serde(default)]
    supports_vision: bool,
    #[serde(default = "native")]
    structured_output_mode: Mode,
    #[serde(default)]
    is_local: bool,
    #[serde(default)]
    is_default_text: bool,
    #[serde(default)]
    is_default_vision: bool,
}

fn native() -> Mode {
    Mode::Native
}

fn validate(p: &ProviderIn) -> Result<()> {
    let bad = |m: &str| Err(Error::bad_request(m));
    if p.label.trim().is_empty() || p.label.len() > 100 {
        return bad("label must be 1-100 characters");
    }
    if p.model.trim().is_empty() {
        return bad("model is required");
    }
    if !reqwest::Url::parse(&p.base_url).is_ok_and(|u| matches!(u.scheme(), "http" | "https")) {
        return bad("base_url must be an http:// or https:// URL");
    }
    if p.is_default_vision && !p.supports_vision {
        return bad("the default photo provider must support vision");
    }
    for (k, v) in p.extra_headers.iter().flatten().flatten() {
        if HeaderName::try_from(k.as_str()).is_err() || HeaderValue::try_from(v.as_str()).is_err() {
            return bad(&format!("invalid extra header {k:?}"));
        }
    }
    Ok(())
}

/// Writes `p` as provider `id` (or a new one), keeping stored secrets the body leaves out.
async fn save(state: &AppState, id: Option<i64>, p: &ProviderIn) -> Result<i64> {
    validate(p)?;
    let seal = async |v: Option<String>| -> Result<Option<String>> {
        match v.filter(|v| !v.is_empty()) {
            Some(v) => Ok(Some(state.secrets.seal(&state.db, &v).await?)),
            None => Ok(None),
        }
    };
    let key = match &p.api_key {
        Some(k) => Some(seal(k.clone()).await?),
        None => None,
    };
    let headers = match &p.extra_headers {
        Some(h) => Some(
            seal(
                h.as_ref()
                    .filter(|h| !h.is_empty())
                    .map(|h| json!(h).to_string()),
            )
            .await?,
        ),
        None => None,
    };
    let (kind, mode) = (name(p.kind), name(p.structured_output_mode));
    let now = crate::time::now_ms();
    let mut tx = state.db.begin().await?;
    if p.is_default_text {
        sqlx::query!("UPDATE ai_providers SET is_default_text = 0")
            .execute(&mut *tx)
            .await?;
    }
    if p.is_default_vision {
        sqlx::query!("UPDATE ai_providers SET is_default_vision = 0")
            .execute(&mut *tx)
            .await?;
    }
    let (set_key, key) = (key.is_some(), key.flatten());
    let (set_headers, headers) = (headers.is_some(), headers.flatten());
    let id = match id {
        None => {
            sqlx::query_scalar!(
                "INSERT INTO ai_providers (label, kind, base_url, model, api_key, extra_headers,
                 supports_vision, structured_output_mode, is_local, is_default_text,
                 is_default_vision, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) RETURNING id",
                p.label,
                kind,
                p.base_url,
                p.model,
                key,
                headers,
                p.supports_vision,
                mode,
                p.is_local,
                p.is_default_text,
                p.is_default_vision,
                now
            )
            .fetch_one(&mut *tx)
            .await?
        }
        Some(id) => sqlx::query_scalar!(
            "UPDATE ai_providers SET label = ?, kind = ?, base_url = ?, model = ?,
                 api_key = CASE WHEN ? THEN ? ELSE api_key END,
                 extra_headers = CASE WHEN ? THEN ? ELSE extra_headers END,
                 supports_vision = ?, structured_output_mode = ?, is_local = ?,
                 is_default_text = ?, is_default_vision = ?
             WHERE id = ? RETURNING id",
            p.label,
            kind,
            p.base_url,
            p.model,
            set_key,
            key,
            set_headers,
            headers,
            p.supports_vision,
            mode,
            p.is_local,
            p.is_default_text,
            p.is_default_vision,
            id
        )
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(Error::not_found)?,
    };
    tx.commit().await?;
    Ok(id)
}

pub async fn list(State(s): State<AppState>, _: Admin) -> Result<Json<Value>> {
    let rows = sqlx::query!(
        "SELECT id, label, kind, base_url, model,
                api_key IS NOT NULL AS \"has_key!: bool\",
                extra_headers IS NOT NULL AS \"has_extra_headers!: bool\",
                supports_vision AS \"supports_vision: bool\", structured_output_mode,
                is_local AS \"is_local: bool\", is_default_text AS \"is_default_text: bool\",
                is_default_vision AS \"is_default_vision: bool\"
         FROM ai_providers ORDER BY id"
    )
    .fetch_all(&s.db)
    .await?;
    let providers: Vec<Value> = rows
        .into_iter()
        .map(|r| {
            json!({
                "id": r.id, "label": r.label, "kind": r.kind, "base_url": r.base_url,
                "model": r.model, "has_key": r.has_key, "has_extra_headers": r.has_extra_headers,
                "supports_vision": r.supports_vision,
                "structured_output_mode": r.structured_output_mode, "is_local": r.is_local,
                "is_default_text": r.is_default_text, "is_default_vision": r.is_default_vision,
            })
        })
        .collect();
    Ok(Json(json!({ "providers": providers })))
}

pub async fn create(
    State(s): State<AppState>,
    _: Admin,
    JsonBody(p): JsonBody<ProviderIn>,
) -> Result<Json<Value>> {
    Ok(Json(json!({ "id": save(&s, None, &p).await? })))
}

pub async fn update(
    State(s): State<AppState>,
    _: Admin,
    Path(id): Path<i64>,
    JsonBody(p): JsonBody<ProviderIn>,
) -> Result<Json<Value>> {
    Ok(Json(json!({ "id": save(&s, Some(id), &p).await? })))
}

pub async fn delete(
    State(s): State<AppState>,
    _: Admin,
    Path(id): Path<i64>,
) -> Result<Json<Value>> {
    let n = sqlx::query!("DELETE FROM ai_providers WHERE id = ?", id)
        .execute(&s.db)
        .await?
        .rows_affected();
    if n == 0 {
        return Err(Error::not_found());
    }
    Ok(Json(json!({})))
}

/// Test connection: one tiny uncapped request, reported in plain language.
pub async fn test(State(s): State<AppState>, _: Admin, Path(id): Path<i64>) -> Result<Json<Value>> {
    if load(&s, id).await.map_err(Error::from)?.is_none() {
        return Err(Error::not_found());
    }
    let req = JsonRequest {
        instruction: "This is a connection test. Reply with {\"ok\": true}.".into(),
        user_text: "Are you there?".into(),
        images: vec![],
        schema: json!({
            "type": "object", "properties": {"ok": {"type": "boolean"}},
            "required": ["ok"], "additionalProperties": false
        }),
    };
    let result = match super::by_id(&s, id).await {
        Ok(p) => p.complete_json(req).await.map(|_| ()),
        Err(e) => Err(e),
    };
    Ok(Json(match result {
        Ok(()) => {
            json!({"ok": true, "message": "Connected. The provider answered with valid JSON."})
        }
        Err(e) => {
            // Admin-only: keeps the upstream detail the user-facing conversion drops.
            json!({"ok": false, "code": e.code(), "message": e.detail()})
        }
    }))
}

// ---- env seeding ----

fn parse_env<T: for<'de> Deserialize<'de>>(
    v: Option<String>,
    k: &str,
    default: &str,
) -> std::result::Result<T, String> {
    let v = v.unwrap_or(default.into());
    serde_json::from_value(json!(v)).map_err(|_| format!("CUPFUL_PROVIDER_{k}={v} is not valid"))
}

/// Seeds one provider from `CUPFUL_PROVIDER_*` when `ai_providers` is empty. Afterwards the
/// database wins; differing env values are ignored with a notice.
pub async fn seed_from_env(
    state: &AppState,
    var: impl Fn(&str) -> Option<String>,
) -> std::result::Result<(), String> {
    let var = |k: &str| var(&format!("CUPFUL_PROVIDER_{k}")).filter(|v| !v.is_empty());
    let Some(base_url) = var("BASE_URL") else {
        return Ok(());
    };
    let flag = |k: &str| var(k).is_some_and(|v| v == "true" || v == "1");
    let model =
        var("MODEL").ok_or("CUPFUL_PROVIDER_MODEL is required with CUPFUL_PROVIDER_BASE_URL")?;
    let extra_headers = var("EXTRA_HEADERS")
        .map(|v| serde_json::from_str(&v))
        .transpose()
        .map_err(|e| {
            format!("CUPFUL_PROVIDER_EXTRA_HEADERS must be a JSON object of strings: {e}")
        })?;
    let vision = flag("VISION");
    let p = ProviderIn {
        label: var("LABEL").unwrap_or_else(|| model.clone()),
        kind: parse_env(var("KIND"), "KIND", "openai_compatible")?,
        base_url,
        model,
        api_key: Some(var("API_KEY")),
        extra_headers: Some(extra_headers),
        supports_vision: vision,
        structured_output_mode: parse_env(var("STRUCTURED_OUTPUT"), "STRUCTURED_OUTPUT", "native")?,
        is_local: flag("LOCAL"),
        is_default_text: true,
        is_default_vision: vision,
    };
    let (kind, url, model) = (name(p.kind), &p.base_url, &p.model);
    let existing = sqlx::query!(
        "SELECT COUNT(*) AS n, COALESCE(SUM(kind = ? AND base_url = ? AND model = ?), 0) AS \"same!: i64\"
         FROM ai_providers",
        kind,
        url,
        model
    )
    .fetch_one(&state.db)
    .await
    .map_err(|e| e.to_string())?;
    if existing.n > 0 {
        if existing.same == 0 {
            tracing::info!(
                "CUPFUL_PROVIDER_* is ignored: providers are already configured; change them in Settings"
            );
        }
        return Ok(());
    }
    save(state, None, &p)
        .await
        .map_err(|e| format!("CUPFUL_PROVIDER_*: {}", e.message()))?;
    tracing::info!("added AI provider {:?} from CUPFUL_PROVIDER_*", p.label);
    Ok(())
}
