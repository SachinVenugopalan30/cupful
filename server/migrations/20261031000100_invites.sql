-- SPDX-License-Identifier: AGPL-3.0-or-later
-- One-use invite links (PLAN.md §10). The token is shown once; only its SHA-256 hex is stored.
CREATE TABLE invites (
    id           INTEGER PRIMARY KEY,
    token_hash   TEXT    NOT NULL UNIQUE,
    created_by   INTEGER NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    created_at   INTEGER NOT NULL,
    expires_at   INTEGER NOT NULL,
    used_by      INTEGER REFERENCES users (id) ON DELETE SET NULL,
    used_at      INTEGER
) STRICT;

CREATE INDEX invites_by_created ON invites (created_at);
