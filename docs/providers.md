<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
# AI providers

Cupful sends meal descriptions, and photos if you use them, to an AI provider you configure. It sends them nowhere else.

Cupful runs fine with no hosted provider. A local Ollama is the default, and manual entry always works.

## Add a provider

An admin adds, edits, tests and removes providers in **Settings → AI providers**.

The API is admin-only:

| Request | Purpose |
| --- | --- |
| `GET /api/ai/providers` | List providers. Never returns keys. |
| `POST /api/ai/providers` | Create one. |
| `PUT /api/ai/providers/{id}` | Update one. |
| `DELETE /api/ai/providers/{id}` | Remove one. |
| `POST /api/ai/providers/{id}/test` | Send one small request and report the result. |

Each provider has these fields:

| Field | Meaning |
| --- | --- |
| `label` | Your name for it. |
| `kind` | `openai_compatible`, `anthropic` or `gemini`. |
| `base_url` | API root, for example `https://api.openai.com/v1`. |
| `model` | Model name as the provider spells it. |
| `api_key` | Optional. Stored encrypted and never shown again; the API reports `has_key`. Omit it in an update to keep it. Send `null` to remove it. |
| `extra_headers` | Optional JSON object of headers, encrypted like the key. |
| `supports_vision` | The model can read photos. |
| `structured_output_mode` | `native` (default) uses the provider's JSON-schema output. `prompt` asks for JSON and takes the first JSON object from the reply, for servers without schema support. |
| `is_local` | Runs on your own hardware, so it is exempt from the daily cap. |
| `is_default_text`, `is_default_vision` | Which provider handles typed meals and which handles photos. One of each. |

**Test connection** reports the result in plain words: connected, wrong key, unreachable, timed out. It does not count toward the daily cap.

### Seed one provider from the environment

The `CUPFUL_PROVIDER_*` variables create one provider while the database has none. See `.env.example`.

After that, the database wins. Changing the variables does nothing, and Cupful logs a notice if they differ. Edit providers in Settings instead.

## Adapter kinds

| Kind | API | Covers |
| --- | --- | --- |
| `openai_compatible` | Chat Completions, `POST {base_url}/chat/completions`, `Authorization: Bearer <key>` | OpenAI, Ollama, llama.cpp server, LM Studio, vLLM, OpenRouter, Groq, Mistral, DeepSeek, Together, Fireworks, xAI, Gemini's OpenAI-compatible endpoint |
| `anthropic` | Messages API, `POST {base_url}/messages`, base `https://api.anthropic.com/v1` | Claude models |
| `gemini` | `POST {base_url}/models/{model}:generateContent`, base `https://generativelanguage.googleapis.com/v1beta` | Gemini models |

In `native` mode, structured output uses `response_format` with a JSON schema for `openai_compatible`, `output_config.format` for Anthropic and `responseJsonSchema` for Gemini. Ollama and llama.cpp translate the OpenAI field to their own format or grammar. If a server rejects these fields, switch the provider to `prompt`.

Each request asks for at most 4096 output tokens. `openai_compatible` and Gemini use temperature 0. Anthropic gets no temperature, because its newer models reject non-default sampling settings. OpenAI reasoning models that refuse `temperature` or `max_tokens` need a non-reasoning model here.

Some setups:

- **OpenAI:** `https://api.openai.com/v1`, your key.
- **Ollama:** `http://host.docker.internal:11434/v1`, no key, `is_local`.
- **OpenRouter:** `https://openrouter.ai/api/v1`, your key.
- **Azure OpenAI:** `openai_compatible` with base URL `https://<resource>.openai.azure.com/openai/v1`, the deployment name as `model`, no `api_key`, and `extra_headers` set to `{"api-key": "<key>"}`.
- **AWS Bedrock or Vertex AI with service accounts:** not supported directly, because they need request signing. Use OpenRouter, or run a [LiteLLM](https://docs.litellm.ai/) proxy and add it as `openai_compatible`.

## Ollama from inside Docker

The Cupful container shares the Tailscale sidecar's network, so `localhost` is the container, not your machine.

1. Make Ollama listen on more than localhost. Set `OLLAMA_HOST=0.0.0.0` for the Ollama service and restart it.
2. Use base URL `http://host.docker.internal:11434/v1`. Docker Desktop provides that name. On Linux, `compose.yaml` maps it with `extra_hosts: ["host.docker.internal:host-gateway"]` on the `tailscale` service.
3. If Ollama runs on another machine, use that machine's tailnet name or IP, for example `http://gpu-box.<tailnet>.ts.net:11434/v1`.

## Daily cap

Each user can make at most `CUPFUL_AI_DAILY_CAP` calls per day to hosted providers. The default is 200. The count uses the user's local date. Local providers have no cap.

When the cap is reached, estimates answer `daily_cap` and manual entry still works.

## Reliability

- Timeouts: 10 seconds to connect, 60 seconds per request, 120 seconds with photos.
- A 429 or 5xx answer is retried twice with exponential backoff and jitter. `Retry-After` is honored up to 30 seconds. A wrong-key answer is never retried.
- A reply that is not valid JSON is retried once with the error included.
- Photos over 5 MB are refused.
- Redirects are never followed, so an API key cannot be forwarded to another host.
- Provider error messages are scrubbed of anything that looks like a key before they are logged or shown.

## Key storage

Provider keys and extra headers are encrypted with XChaCha20-Poly1305 under an instance key.

- `CUPFUL_SECRET_KEY` (32 random bytes, base64: `openssl rand -base64 32`) when set.
- Otherwise `/data/secret.key`, which Cupful creates with mode 0600 on first start if nothing is encrypted yet.

A key file next to the database protects a copied `cupful.db` on its own. A key held separately, in the environment or a secrets manager, also protects a copy of the whole data volume.

At startup Cupful checks that the key opens the secrets already saved. With the wrong key, or with the key gone, it refuses to start and says so. It never creates a new key over existing secrets.

Losing the key loses only the provider keys and headers. Recovery steps: [backups.md](backups.md#the-secret-key).

## Choosing a model

Bigger models estimate better. In testing, a 1.2B model left foods out ("a splash of milk"), merged two foods into one item and wrote counts as weights. Cupful works around much of that. Measured USDA portions beat the model's grams, and the review warns about foods the description names but the estimate missed. A 7B or larger instruction model is a better default.

To see exactly what a model replied, run the server with `RUST_LOG=cupful=debug`.
