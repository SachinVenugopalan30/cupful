// SPDX-License-Identifier: AGPL-3.0-or-later
//! Anthropic Messages API (`POST {base}/messages`, base `https://api.anthropic.com/v1`).
//! Native mode uses structured outputs (`output_config.format`). No `temperature`: newer models
//! reject non-default sampling settings.

use serde_json::{Value, json};

use super::{JsonRequest, MAX_TOKENS, Mode, ProviderConfig, data_base64, join_url};

pub fn request(c: &ProviderConfig, req: &JsonRequest) -> (String, Value) {
    let mut content: Vec<Value> = req
        .images
        .iter()
        .map(|i| {
            json!({"type": "image", "source": {
                "type": "base64", "media_type": i.media_type, "data": data_base64(i)
            }})
        })
        .collect();
    content.push(json!({"type": "text", "text": req.user_text}));
    let mut body = json!({
        "model": c.model,
        "max_tokens": MAX_TOKENS,
        "system": req.instruction,
        "messages": [{"role": "user", "content": content}],
    });
    if c.mode == Mode::Native {
        body["output_config"] = json!({"format": {"type": "json_schema", "schema": req.schema}});
    }
    (join_url(&c.base_url, "messages"), body)
}

pub fn reply_text(v: &Value) -> Option<String> {
    v["content"]
        .as_array()?
        .iter()
        .find(|b| b["type"] == "text")?["text"]
        .as_str()
        .map(str::to_owned)
}
