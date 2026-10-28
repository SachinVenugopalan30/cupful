// SPDX-License-Identifier: AGPL-3.0-or-later
//! Gemini `generateContent` (`POST {base}/models/{model}:generateContent`, base
//! `https://generativelanguage.googleapis.com/v1beta`). Native mode sends `responseJsonSchema`.

use serde_json::{Value, json};

use super::{JsonRequest, MAX_TOKENS, Mode, ProviderConfig, data_base64, join_url};

pub fn request(c: &ProviderConfig, req: &JsonRequest) -> (String, Value) {
    let mut parts: Vec<Value> = req
        .images
        .iter()
        .map(|i| json!({"inlineData": {"mimeType": i.media_type, "data": data_base64(i)}}))
        .collect();
    parts.push(json!({"text": req.user_text}));
    let mut config = json!({"temperature": 0, "maxOutputTokens": MAX_TOKENS});
    if c.mode == Mode::Native {
        config["responseMimeType"] = json!("application/json");
        config["responseJsonSchema"] = req.schema.clone();
    }
    let body = json!({
        "systemInstruction": {"parts": [{"text": req.instruction}]},
        "contents": [{"role": "user", "parts": parts}],
        "generationConfig": config,
    });
    let url = join_url(&c.base_url, &format!("models/{}:generateContent", c.model));
    (url, body)
}

pub fn reply_text(v: &Value) -> Option<String> {
    let parts = v["candidates"][0]["content"]["parts"].as_array()?;
    Some(parts.iter().filter_map(|p| p["text"].as_str()).collect())
}
