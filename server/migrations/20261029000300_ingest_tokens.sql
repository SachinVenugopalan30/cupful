-- Bearer tokens for the activity ingest endpoint (PLAN.md §11). Only the SHA-256 is stored;
-- the token itself is high-entropy and shown once, so a fast hash is enough.
CREATE TABLE ingest_tokens (
    id           INTEGER PRIMARY KEY,
    user_id      INTEGER NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    label        TEXT    NOT NULL,
    token_hash   TEXT    NOT NULL UNIQUE,
    created_at   INTEGER NOT NULL,
    last_used_at INTEGER
) STRICT;
CREATE INDEX ingest_tokens_user ON ingest_tokens (user_id);
