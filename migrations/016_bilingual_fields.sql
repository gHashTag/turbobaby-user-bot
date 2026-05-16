-- Migration 016: Add English-language fields to all catalog tables.
-- RU columns remain primary; *_en columns are nullable additions.
-- Existing data is untouched; UI falls back to RU when *_en is NULL/empty.

-- ── strains ─────────────────────────────────────────────────────────
ALTER TABLE strains ADD COLUMN IF NOT EXISTS name_en           TEXT;
ALTER TABLE strains ADD COLUMN IF NOT EXISTS description_en    TEXT;
ALTER TABLE strains ADD COLUMN IF NOT EXISTS effect_en         TEXT;
ALTER TABLE strains ADD COLUMN IF NOT EXISTS flavor_profile_en TEXT;
ALTER TABLE strains ADD COLUMN IF NOT EXISTS strain_type_en    TEXT;

-- ── accessories ─────────────────────────────────────────────────────
-- NOTE: the accessories table uses "category" (not "subcategory"), so the
-- EN counterpart is "category_en" to match the existing column name.
ALTER TABLE accessories ADD COLUMN IF NOT EXISTS name_en        TEXT;
ALTER TABLE accessories ADD COLUMN IF NOT EXISTS description_en TEXT;
ALTER TABLE accessories ADD COLUMN IF NOT EXISTS category_en    TEXT;

-- ── accessory_sets ──────────────────────────────────────────────────
ALTER TABLE accessory_sets ADD COLUMN IF NOT EXISTS name_en        TEXT;
ALTER TABLE accessory_sets ADD COLUMN IF NOT EXISTS description_en TEXT;

-- ── tea_products ────────────────────────────────────────────────────
ALTER TABLE tea_products ADD COLUMN IF NOT EXISTS name_en        TEXT;
ALTER TABLE tea_products ADD COLUMN IF NOT EXISTS description_en TEXT;
ALTER TABLE tea_products ADD COLUMN IF NOT EXISTS subcategory_en TEXT;

-- ── tea_sets ────────────────────────────────────────────────────────
ALTER TABLE tea_sets ADD COLUMN IF NOT EXISTS name_en        TEXT;
ALTER TABLE tea_sets ADD COLUMN IF NOT EXISTS description_en TEXT;
