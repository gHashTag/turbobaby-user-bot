-- Garden overhaul B2 (choose-any-product + product-scoped discount).
--
-- The garden moves from "auto-seed the strain you ordered" to "the customer
-- chooses ANY live product to grow a discount for". A plant/reward now snapshots
-- the chosen product (catalog + id + name + photo); the photo becomes the seed
-- image. Admins can opt a product out of the garden via `garden_eligible`.
--
-- Idempotent ADD COLUMN IF NOT EXISTS (prod-drift safe — the migration runner
-- re-applies the whole MIGRATION_SQL every boot with no tracking table).

-- The chosen target product on a plant (the seed). Legacy strain_id/strain_name
-- stay for old rows; new plants fill target_*.
ALTER TABLE garden_plants ADD COLUMN IF NOT EXISTS target_catalog    VARCHAR(20);
ALTER TABLE garden_plants ADD COLUMN IF NOT EXISTS target_product_id TEXT;
ALTER TABLE garden_plants ADD COLUMN IF NOT EXISTS target_name       TEXT;
ALTER TABLE garden_plants ADD COLUMN IF NOT EXISTS target_image_url  TEXT;

-- The reward carries the target so checkout can scope the discount to that
-- product. scope = 'cart' (legacy, whole-cart) | 'product' (the new model).
ALTER TABLE garden_rewards ADD COLUMN IF NOT EXISTS target_catalog    VARCHAR(20);
ALTER TABLE garden_rewards ADD COLUMN IF NOT EXISTS target_product_id TEXT;
ALTER TABLE garden_rewards ADD COLUMN IF NOT EXISTS scope             VARCHAR(20) NOT NULL DEFAULT 'cart';

-- Per-product garden eligibility (default true = all products are choosable;
-- admins opt specific products OUT). Mirrors is_available across the 6 catalogs.
ALTER TABLE strains        ADD COLUMN IF NOT EXISTS garden_eligible BOOLEAN NOT NULL DEFAULT true;
ALTER TABLE accessories    ADD COLUMN IF NOT EXISTS garden_eligible BOOLEAN NOT NULL DEFAULT true;
ALTER TABLE tea_products   ADD COLUMN IF NOT EXISTS garden_eligible BOOLEAN NOT NULL DEFAULT true;
ALTER TABLE sets           ADD COLUMN IF NOT EXISTS garden_eligible BOOLEAN NOT NULL DEFAULT true;
ALTER TABLE accessory_sets ADD COLUMN IF NOT EXISTS garden_eligible BOOLEAN NOT NULL DEFAULT true;
ALTER TABLE tea_sets       ADD COLUMN IF NOT EXISTS garden_eligible BOOLEAN NOT NULL DEFAULT true;
