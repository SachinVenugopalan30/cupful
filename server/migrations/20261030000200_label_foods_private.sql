-- Label entries ("enter from the label") were stored with user_id NULL, so every user's barcode
-- scan could hit someone else's macros. Make them private to the user in source_ref.
UPDATE foods SET user_id = CAST(source_ref AS INTEGER)
WHERE source = 'user' AND user_id IS NULL
  AND source_ref GLOB '[0-9]*' AND source_ref NOT GLOB '*[^0-9]*'
  AND CAST(source_ref AS INTEGER) IN (SELECT id FROM users);

-- Drop other users' saved-product links to those now-private foods.
DELETE FROM user_products
WHERE food_id IN (SELECT id FROM foods WHERE source = 'user' AND user_id IS NOT NULL)
  AND user_id != (SELECT user_id FROM foods WHERE id = user_products.food_id);
