-- Accounts, profiles, logged data and the sync bookkeeping (PLAN.md §5, §6).
-- Times are INTEGER unix milliseconds (UTC); dates are TEXT 'YYYY-MM-DD' in the user's local calendar.

CREATE TABLE users (
    id              INTEGER PRIMARY KEY,
    username        TEXT    NOT NULL UNIQUE COLLATE NOCASE,
    password_hash   TEXT,
    tailscale_login TEXT    UNIQUE,
    is_admin        INTEGER NOT NULL DEFAULT 0 CHECK (is_admin IN (0, 1)),
    created_at      INTEGER NOT NULL
) STRICT;

CREATE TABLE profiles (
    user_id     INTEGER PRIMARY KEY REFERENCES users (id) ON DELETE CASCADE,
    timezone    TEXT    NOT NULL,
    weight_unit TEXT    NOT NULL DEFAULT 'kg' CHECK (weight_unit IN ('kg', 'lb')),
    height_unit TEXT    NOT NULL DEFAULT 'cm' CHECK (height_unit IN ('cm', 'ftin'))
) STRICT;

-- Saving the profile writes the version effective from the user's today; a second save that day replaces it.
CREATE TABLE profile_versions (
    id               INTEGER PRIMARY KEY,
    user_id          INTEGER NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    effective_from   TEXT    NOT NULL,
    sex              TEXT    NOT NULL CHECK (sex IN ('male', 'female')),
    birth_year       INTEGER NOT NULL,
    height_cm        REAL    NOT NULL,
    start_weight_kg  REAL    NOT NULL,
    goal_kg          REAL,
    pace_kg_per_week REAL    NOT NULL,
    typical_steps    INTEGER NOT NULL,
    UNIQUE (user_id, effective_from)
) STRICT;

CREATE TABLE days (
    user_id            INTEGER NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    date               TEXT    NOT NULL,
    steps              INTEGER,
    steps_source       TEXT CHECK (steps_source IN ('manual', 'shortcut', 'import', 'google')),
    steps_updated_at   INTEGER,
    gym_minutes        INTEGER,
    gym_source         TEXT CHECK (gym_source IN ('manual', 'shortcut', 'import', 'google')),
    gym_updated_at     INTEGER,
    trained            TEXT,    -- JSON array of body parts
    trained_updated_at INTEGER,
    weight_kg          REAL,
    weight_updated_at  INTEGER,
    PRIMARY KEY (user_id, date)
) STRICT;

CREATE TABLE meals (
    id          TEXT    PRIMARY KEY, -- client-generated UUID
    user_id     INTEGER NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    date        TEXT    NOT NULL,
    meal_type   TEXT    NOT NULL CHECK (meal_type IN ('breakfast', 'lunch', 'dinner', 'snack')),
    description TEXT    NOT NULL,
    scale       REAL    NOT NULL DEFAULT 1,
    assumptions TEXT,
    confidence  TEXT CHECK (confidence IN ('low', 'medium', 'high')),
    source      TEXT    NOT NULL CHECK (source IN ('ai', 'manual', 'repeat', 'import')),
    provider_id INTEGER,
    updated_at  INTEGER NOT NULL, -- effective time of the last write (§6)
    received_at INTEGER NOT NULL,
    deleted_at  INTEGER          -- tombstone
) STRICT;
CREATE INDEX meals_user_date ON meals (user_id, date);

CREATE TABLE meal_items (
    id           TEXT    PRIMARY KEY, -- client-generated UUID
    meal_id      TEXT    NOT NULL REFERENCES meals (id) ON DELETE CASCADE,
    position     INTEGER NOT NULL,
    name         TEXT    NOT NULL,
    portion_text TEXT    NOT NULL DEFAULT '',
    grams        REAL,
    kcal         REAL    NOT NULL,
    protein_g    REAL    NOT NULL,
    carbs_g      REAL    NOT NULL,
    fat_g        REAL    NOT NULL,
    food_id      INTEGER,
    origin       TEXT    NOT NULL CHECK (origin IN ('food_db', 'ai_estimate'))
) STRICT;
CREATE INDEX meal_items_meal ON meal_items (meal_id);

-- Remembered 30 days, only to answer `duplicate`.
CREATE TABLE sync_ops (
    user_id     INTEGER NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    op_id       TEXT    NOT NULL,
    received_at INTEGER NOT NULL,
    PRIMARY KEY (user_id, op_id)
) STRICT;

-- tower-sessions records, stored by our own small store (src/auth.rs).
CREATE TABLE sessions (
    id          TEXT    PRIMARY KEY,
    user_id     INTEGER,
    data        TEXT    NOT NULL,
    expiry_date INTEGER NOT NULL -- unix seconds
) STRICT;
CREATE INDEX sessions_expiry ON sessions (expiry_date);
CREATE INDEX sessions_user ON sessions (user_id);
