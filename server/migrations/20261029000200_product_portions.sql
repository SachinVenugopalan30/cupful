-- A product's serving is a portion the estimate pipeline can use. Units parsed from serving_text
-- ("1 scoop (31 g)") are added when a product is next written; this covers the serving itself.
UPDATE foods SET default_portion_g = serving_g
WHERE source IN ('off', 'user') AND barcode IS NOT NULL AND serving_g IS NOT NULL;

INSERT OR REPLACE INTO food_portions (food_id, unit, grams)
SELECT id, 'serving', serving_g FROM foods
WHERE source IN ('off', 'user') AND barcode IS NOT NULL AND serving_g IS NOT NULL;
