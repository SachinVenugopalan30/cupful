-- Grams per household unit for a food, from USDA portion data ("1 scoop = 28.7 g"). Seeded for staples.
CREATE TABLE food_portions (
    food_id INTEGER NOT NULL REFERENCES foods (id) ON DELETE CASCADE,
    unit    TEXT    NOT NULL, -- canonical unit word: cup, tbsp, scoop, slice, large, fl oz, ...
    grams   REAL    NOT NULL, -- for one unit
    PRIMARY KEY (food_id, unit)
) STRICT;
