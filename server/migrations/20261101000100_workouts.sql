-- Exercise sessions pulled from Google Health. Derived data: re-syncable, so not in backups.
-- Kept when Google is disconnected (they are the user's history).
CREATE TABLE workouts (
    id             INTEGER PRIMARY KEY,
    user_id        INTEGER NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    external_id    TEXT    NOT NULL,
    start_ms       INTEGER NOT NULL,
    end_ms         INTEGER NOT NULL,
    date           TEXT    NOT NULL,
    exercise_type  TEXT    NOT NULL,
    name           TEXT    NOT NULL,
    active_seconds INTEGER NOT NULL,
    kcal           REAL,
    distance_m     REAL,
    avg_hr         INTEGER,
    source         TEXT    NOT NULL DEFAULT 'google',
    UNIQUE (user_id, external_id)
) STRICT;
CREATE INDEX workouts_user_date ON workouts (user_id, date);

-- JSON array of exercise types that count as gym minutes; NULL = the default set.
ALTER TABLE profiles ADD COLUMN gym_types TEXT;
