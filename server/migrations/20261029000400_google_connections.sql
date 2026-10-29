-- One Google Health connection per user (PLAN.md §15). Tokens are encrypted with `crypto::Cipher`.
CREATE TABLE google_connections (
    user_id           INTEGER PRIMARY KEY REFERENCES users (id) ON DELETE CASCADE,
    refresh_token     TEXT    NOT NULL,
    access_token      TEXT,
    access_expires_at INTEGER,
    connected_at      INTEGER NOT NULL,
    last_sync_at      INTEGER,
    last_error        TEXT,
    needs_reconnect   INTEGER NOT NULL DEFAULT 0
) STRICT;
