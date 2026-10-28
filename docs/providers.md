# AI providers

Cupful sends meal descriptions (and photos, if you use them) to an AI provider you configure, and nowhere else. It runs fine with no hosted provider at all: a local Ollama is the default, and manual entry always works.

## Adding providers

Admins add, edit, test and remove providers in **Settings → AI providers** (API: `GET/POST /api/ai/providers`, `PUT/DELETE /api/ai/providers/{id}`, `POST /api/ai/providers/{id}/test`). Each provider has:

| Field | Meaning |
|---|---|
| `label` | Your name for it. |
| `kind` | `openai_compatible`, `anthropic` or `gemini` (below). |
| `base_url` | API root, e.g. `https://api.openai.com/v1`. |
| `model` | Model name as the provider spells it. |
| `api_key` | Optional. Stored encrypted, never shown again (the API reports `has_key`). Leave it out of an update to keep it; send `null` to remove it. |
| `extra_headers` | Optional JSON object of headers, stored encrypted like the key. |
| `supports_vision` | The model can read photos. |
| `structured_output_mode` | `native` (default): the provider's JSON-schema output. `prompt`: ask for "only JSON" and take the first JSON object from the reply, for servers without schema support. |
| `is_local` | Runs on your own hardware: exempt from the daily cap. |
| `is_default_text`, `is_default_vision` | Which provider handles typed meals and which handles photos. One of each. |

**Test connection** sends one tiny request and reports the result in plain words (connected, wrong key, unreachable, timed out, and so on). It doesn't count toward the daily cap.

### First start from the environment

`CUPFUL_PROVIDER_*` variables (see `.env.example`) create one provider, but only while there are none. After that the database wins: changing the variables does nothing, and Cupful logs a notice that they are ignored. Edit providers in Settings instead.

## The three adapter kinds

| Kind | API | Covers |
|---|---|---|
| `openai_compatible` | Chat Completions, `POST {base_url}/chat/completions`, `Authorization: Bearer <key>` | OpenAI, Ollama, llama.cpp server, LM Studio, vLLM, OpenRouter, Groq, Mistral, DeepSeek, Together, Fireworks, xAI, Gemini's OpenAI-compatible endpoint |
| `anthropic` | Messages API, `POST {base_url}/messages`, base `https://api.anthropic.com/v1` | Claude models |
| `gemini` | `POST {base_url}/models/{model}:generateContent`, base `https://generativelanguage.googleapis.com/v1beta` | Gemini models |

Structured output in `native` mode: `response_format` with a JSON schema for `openai_compatible` (Ollama and llama.cpp translate it to their own `format` / grammar), `output_config.format` for Anthropic, `responseJsonSchema` for Gemini. If a server rejects those fields, switch the provider to `prompt`.

Every request uses at most 1024 output tokens and temperature 0 (Anthropic gets no temperature: its newer models reject non-default sampling settings). OpenAI reasoning models that refuse `temperature`/`max_tokens` need a non-reasoning model here.

Some setups:

- **OpenAI:** `https://api.openai.com/v1`, your key.
- **Ollama:** `http://host.docker.internal:11434/v1`, no key, `is_local`.
- **OpenRouter:** `https://openrouter.ai/api/v1`, your key.
- **Azure OpenAI:** `openai_compatible` with base URL `https://<resource>.openai.azure.com/openai/v1`, the **deployment name** as `model`, no `api_key`, and `extra_headers` `{"api-key": "<key>"}`.
- **AWS Bedrock, Vertex AI with service accounts:** not supported directly (they need request signing). Go through OpenRouter, or run a [LiteLLM](https://docs.litellm.ai/) proxy and add it as `openai_compatible`.

## Ollama from inside Docker

Cupful runs in a container that shares the Tailscale sidecar's network, so `localhost` is the container, not your machine.

1. Make Ollama listen on more than localhost: set `OLLAMA_HOST=0.0.0.0` for the Ollama service and restart it.
2. Use base URL `http://host.docker.internal:11434/v1`. Docker Desktop provides that name; on Linux, `compose.yaml` already maps it with `extra_hosts: ["host.docker.internal:host-gateway"]` on the `tailscale` service.
3. If Ollama runs on another machine, use that machine's tailnet name or IP instead, e.g. `http://gpu-box.<tailnet>.ts.net:11434/v1`.

## Daily cap

Each user can make at most `CUPFUL_AI_DAILY_CAP` calls (default 200) per day to hosted providers, counted by the user's own local date. Local providers (`is_local`) have no cap. When the cap is reached, estimates answer `daily_cap` and manual entry still works.

## Reliability

Timeouts: 10 s to connect, 60 s per request (120 s with photos). 429 and 5xx answers are retried twice with exponential backoff and jitter, honoring `Retry-After` up to 30 s; wrong-key answers are never retried. A reply that isn't valid JSON is retried once with the error included. Photos over 5 MB are refused. Error messages from providers are scrubbed of anything that looks like a key before they are logged or shown.

## Key storage

Provider keys and extra headers are encrypted (XChaCha20-Poly1305) with an instance key:

- `CUPFUL_SECRET_KEY` (32 random bytes, base64: `openssl rand -base64 32`) if set;
- otherwise `/data/secret.key`, which Cupful creates (mode 0600) on first start if nothing is encrypted yet.

**Trade-off:** a key file next to the database only protects against someone who gets the database without the rest of the data volume, such as a copied backup of `cupful.db` alone. A key held separately, in the environment or a secrets manager, protects backups of the whole volume too.

At startup Cupful checks that the key opens the secrets already saved. With the wrong key, or with the key gone (for example `CUPFUL_SECRET_KEY` removed after keys were saved with it), it refuses to start and says so; it never creates a new key over existing secrets.

**Losing the key** loses only the provider keys and headers. To recover, clear them and enter them again: stop Cupful, run (with the `sqlite3` tool on the host, against `cupful.db` in the data volume; the image has no shell) `sqlite3 cupful.db "UPDATE ai_providers SET api_key = NULL, extra_headers = NULL; DELETE FROM secret_check;"`, start it with the new key (or none, to generate one), and re-enter the keys in Settings. Meals, days and everything else are untouched.
