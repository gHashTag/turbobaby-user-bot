-- Cycle (after #171): admin form for the general Sets tab and the Tea
-- Sets tab had no image-upload control because the underlying schema
-- never carried `image_url`. Accessory Sets already had it, so the
-- admin UI was inconsistent across the three set types — user-reported
-- as "не получается добавить картинку".
--
-- Symmetric column add: TEXT nullable, default NULL. Backward
-- compatible — existing rows read as NULL → frontend falls back to
-- the `icon` field exactly as before.

ALTER TABLE sets     ADD COLUMN IF NOT EXISTS image_url TEXT;
ALTER TABLE tea_sets ADD COLUMN IF NOT EXISTS image_url TEXT;
