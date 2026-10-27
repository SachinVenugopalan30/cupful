-- Food database, aliases, search index, user portions and saved packaged products (PLAN.md §5, §8).
-- Shared contract for M2: the nutrition pipeline and barcode products both build on it.

CREATE TABLE foods (
    id                INTEGER PRIMARY KEY,
    source            TEXT    NOT NULL CHECK (source IN ('staple', 'usda', 'user', 'off')),
    source_ref        TEXT,             -- USDA FDC id, citation, user id, or Open Food Facts barcode
    user_id           INTEGER REFERENCES users (id) ON DELETE CASCADE, -- NULL = visible to everyone
    name              TEXT    NOT NULL,
    name_norm         TEXT    NOT NULL,
    brand             TEXT,
    barcode           TEXT,             -- GTIN digits; several rows may share one (an `off` row and label entries)
    is_prepared       INTEGER NOT NULL DEFAULT 0 CHECK (is_prepared IN (0, 1)),
    kcal              REAL    NOT NULL, -- per 100 g
    protein_g         REAL    NOT NULL,
    carbs_g           REAL    NOT NULL,
    fat_g             REAL    NOT NULL,
    default_portion_g REAL,
    serving_g         REAL,             -- one label serving
    serving_text      TEXT,
    fetched_at        INTEGER           -- unix ms, `off` rows
) STRICT;
CREATE INDEX foods_barcode ON foods (barcode) WHERE barcode IS NOT NULL;
CREATE UNIQUE INDEX foods_source_ref ON foods (source, source_ref) WHERE source IN ('staple', 'usda', 'off');

CREATE TABLE food_aliases (
    id         INTEGER PRIMARY KEY,
    food_id    INTEGER NOT NULL REFERENCES foods (id) ON DELETE CASCADE,
    alias      TEXT    NOT NULL,
    alias_norm TEXT    NOT NULL,
    user_id    INTEGER REFERENCES users (id) ON DELETE CASCADE -- NULL = shipped alias
) STRICT;
CREATE INDEX food_aliases_norm ON food_aliases (alias_norm);

-- One row per food name and per alias. Kept in sync by triggers, so any insert anywhere is searchable.
CREATE VIRTUAL TABLE foods_fts USING fts5 (
    text,
    food_id  UNINDEXED,
    user_id  UNINDEXED, -- from the food (private foods) or the alias (a user's own alias)
    alias_id UNINDEXED  -- NULL for the food's own name
);

CREATE TRIGGER foods_fts_ai AFTER INSERT ON foods BEGIN
    INSERT INTO foods_fts (text, food_id, user_id, alias_id) VALUES (new.name_norm, new.id, new.user_id, NULL);
END;
CREATE TRIGGER foods_fts_au AFTER UPDATE OF name_norm, user_id ON foods BEGIN
    DELETE FROM foods_fts WHERE food_id = old.id AND alias_id IS NULL;
    INSERT INTO foods_fts (text, food_id, user_id, alias_id) VALUES (new.name_norm, new.id, new.user_id, NULL);
END;
CREATE TRIGGER foods_fts_ad AFTER DELETE ON foods BEGIN
    DELETE FROM foods_fts WHERE food_id = old.id;
END;
CREATE TRIGGER food_aliases_fts_ai AFTER INSERT ON food_aliases BEGIN
    INSERT INTO foods_fts (text, food_id, user_id, alias_id) VALUES (new.alias_norm, new.food_id, new.user_id, new.id);
END;
CREATE TRIGGER food_aliases_fts_ad AFTER DELETE ON food_aliases BEGIN
    DELETE FROM foods_fts WHERE alias_id = old.id;
END;

CREATE TABLE user_portions (
    user_id     INTEGER NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    phrase_norm TEXT    NOT NULL,
    food_id     INTEGER NOT NULL REFERENCES foods (id) ON DELETE CASCADE,
    grams       REAL    NOT NULL,
    PRIMARY KEY (user_id, phrase_norm)
) STRICT;

CREATE TABLE user_products (
    user_id       INTEGER NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    food_id       INTEGER NOT NULL REFERENCES foods (id) ON DELETE CASCADE,
    last_servings REAL,
    last_used_at  INTEGER NOT NULL,
    PRIMARY KEY (user_id, food_id)
) STRICT;
