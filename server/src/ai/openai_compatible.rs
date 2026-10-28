// SPDX-License-Identifier: AGPL-3.0-or-later
//! Chat Completions (`POST {base}/chat/completions`): OpenAI, Ollama, llama.cpp, LM Studio, vLLM,
//! OpenRouter, Groq, Azure OpenAI (v1 URL + `api-key` extra header) and the rest.
//! Native mode sends `response_format: json_schema`; Ollama's and llama.cpp's OpenAI endpoints map
//! it to their own `format` / grammar, so this one field covers them.

use serde_json::{Value, json};

use super::{JsonRequest, MAX_TOKENS, Mode, ProviderConfig, data_base64, join_url};

pub fn request(c: &ProviderConfig, req: &JsonRequest) -> (String, Value) {
    let user = if req.images.is_empty() {
        // A plain string: some servers reject content arrays.
        json!(req.user_text)
    } else {
        let images = req.images.iter().map(|i| {
            json!({"type": "image_url", "image_url": {
                "url": format!("data:{};base64,{}", i.media_type, data_base64(i))
            }})
        });
        std::iter::once(json!({"type": "text", "text": req.user_text}))
            .chain(images)
            .collect()
    };
    let mut body = json!({
        "model": c.model,
        "temperature": 0,
        "max_tokens": MAX_TOKENS,
        "messages": [
            {"role": "system", "content": req.instruction},
            {"role": "user", "content": user},
        ],
    });
    if c.mode == Mode::Native {
        body["response_format"] =
            json!({"type": "json_schema", "json_schema": {"name": "reply", "schema": req.schema}});
    }
    (join_url(&c.base_url, "chat/completions"), body)
}

pub fn reply_text(v: &Value) -> Option<String> {
    v["choices"][0]["message"]["content"]
        .as_str()
        .map(str::to_owned)
}
