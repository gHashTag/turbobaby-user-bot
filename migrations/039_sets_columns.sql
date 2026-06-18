-- Prod hotfix: the `sets` table on production is missing columns that
-- `create_set` / `update_set` / `get_sets` reference (observed in prod logs:
-- `get_sets sets: column "strain_ids" does not exist`). Cause: the `sets` table
-- was first created on prod with an older shape, and migration 002's
-- `CREATE TABLE IF NOT EXISTS sets (...)` is a no-op against an existing table —
-- so the array/flag columns added to 002's CREATE later never landed, and there
-- was no ALTER to add them. This broke BOTH:
--   * adding a set in admin (INSERT references strain_ids → 500 "ошибка добавления"), and
--   * the public /api/sets `sets` source (SELECT strain_ids → degrades to empty).
--
-- Idempotent ADD COLUMN IF NOT EXISTS for every `sets` column the code needs
-- that an old table may lack (no-op where already present, e.g. on fresh DBs).

ALTER TABLE sets ADD COLUMN IF NOT EXISTS description      TEXT             NOT NULL DEFAULT '';
ALTER TABLE sets ADD COLUMN IF NOT EXISTS icon             TEXT             NOT NULL DEFAULT '';
ALTER TABLE sets ADD COLUMN IF NOT EXISTS strain_ids       TEXT[]           NOT NULL DEFAULT '{}';
ALTER TABLE sets ADD COLUMN IF NOT EXISTS accessory_ids    TEXT[]           NOT NULL DEFAULT '{}';
ALTER TABLE sets ADD COLUMN IF NOT EXISTS total_price      DOUBLE PRECISION NOT NULL DEFAULT 0;
ALTER TABLE sets ADD COLUMN IF NOT EXISTS discount_percent DOUBLE PRECISION NOT NULL DEFAULT 0;
ALTER TABLE sets ADD COLUMN IF NOT EXISTS is_available     BOOLEAN          NOT NULL DEFAULT true;
ALTER TABLE sets ADD COLUMN IF NOT EXISTS is_deal_of_day   BOOLEAN          NOT NULL DEFAULT false;
