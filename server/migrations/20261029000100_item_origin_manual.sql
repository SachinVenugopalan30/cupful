-- Hand-entered items get their own origin (they were stored as 'ai_estimate').
-- SQLite can't alter a CHECK, so rebuild the table. Nothing references meal_items.
CREATE TABLE meal_items_new (
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
    origin       TEXT    NOT NULL CHECK (origin IN ('food_db', 'ai_estimate', 'manual'))
) STRICT;
INSERT INTO meal_items_new SELECT * FROM meal_items;
DROP TABLE meal_items;
ALTER TABLE meal_items_new RENAME TO meal_items;
CREATE INDEX meal_items_meal ON meal_items (meal_id);

UPDATE meal_items SET origin = 'manual'
WHERE origin = 'ai_estimate' AND food_id IS NULL
  AND meal_id IN (SELECT id FROM meals WHERE source = 'manual');
