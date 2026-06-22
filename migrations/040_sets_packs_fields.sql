-- Packs (Наборы) Phase 1: the customer-facing "Наборы" section needs each
-- strain pack to carry a total weight (shown as "10G Total · N сортов по Xг")
-- and an optional promo badge (SALE / SPECIAL OFFER / LIMITED EDITION) used both
-- for the on-card chip and the "promo packs first" carousel ordering.
--
-- Only the general `sets` table (strain bundles: Party / All Day / Night /
-- Woody Best Of) gets these — accessory_sets/tea_sets are out of scope.
--
-- Idempotent ADD COLUMN IF NOT EXISTS (prod-drift safe: the migration runner
-- re-applies the whole MIGRATION_SQL every boot with no tracking table — see the
-- migration-runner note). No-op where already present (fresh DBs).

-- Total weight of the pack in grams (admin-entered). "per strain" is derived in
-- the UI as total / strain_count, so only one number is stored.
ALTER TABLE sets ADD COLUMN IF NOT EXISTS total_weight_grams DOUBLE PRECISION NOT NULL DEFAULT 0;

-- One promo badge per pack. Allowed values: 'none' | 'sale' | 'special' | 'limited'
-- (validated server-side in catalog.rs; default 'none' = no badge).
ALTER TABLE sets ADD COLUMN IF NOT EXISTS badge VARCHAR(20) NOT NULL DEFAULT 'none';
