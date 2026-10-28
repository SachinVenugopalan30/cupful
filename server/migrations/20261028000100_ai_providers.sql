-- AI providers, per-user daily usage and the secret-key check value (PLAN.md §5, §9).

CREATE TABLE ai_providers (
    id                     INTEGER PRIMARY KEY,
    label                  TEXT    NOT NULL,
    kind                   TEXT    NOT NULL CHECK (kind IN ('openai_compatible', 'anthropic', 'gemini')),
    base_url               TEXT    NOT NULL,
    model                  TEXT    NOT NULL,
    api_key                TEXT,             -- encrypted (crypto.rs), NULL = none
    extra_headers          TEXT,             -- encrypted JSON object of header -> value, NULL = none
    supports_vision        INTEGER NOT NULL DEFAULT 0 CHECK (supports_vision IN (0, 1)),
    -- native: the provider's schema-constrained output; prompt: "reply with only JSON" + extraction.
    structured_output_mode TEXT    NOT NULL DEFAULT 'native' CHECK (structured_output_mode IN ('native', 'prompt')),
    -- Local providers (Ollama on your own machine) are exempt from the daily call cap.
    is_local               INTEGER NOT NULL DEFAULT 0 CHECK (is_local IN (0, 1)),
    is_default_text        INTEGER NOT NULL DEFAULT 0 CHECK (is_default_text IN (0, 1)),
    is_default_vision      INTEGER NOT NULL DEFAULT 0 CHECK (is_default_vision IN (0, 1)),
    created_at             INTEGER NOT NULL
) STRICT;

-- At most one default of each kind.
CREATE UNIQUE INDEX ai_providers_default_text ON ai_providers (is_default_text) WHERE is_default_text = 1;
CREATE UNIQUE INDEX ai_providers_default_vision ON ai_providers (is_default_vision) WHERE is_default_vision = 1;

-- Calls per user per local date per provider; the cap sums hosted (is_local = 0) providers.
CREATE TABLE ai_usage (
    user_id     INTEGER NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    date        TEXT    NOT NULL,
    provider_id INTEGER NOT NULL REFERENCES ai_providers (id) ON DELETE CASCADE,
    calls       INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (user_id, date, provider_id)
) STRICT;

-- A known constant encrypted with the instance key, written on the first encryption. Startup
-- refuses to run if the configured key can't decrypt it.
CREATE TABLE secret_check (
    id    INTEGER PRIMARY KEY CHECK (id = 1),
    value TEXT    NOT NULL
) STRICT;
