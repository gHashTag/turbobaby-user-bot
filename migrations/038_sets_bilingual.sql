-- Sets bilingual follow-up: migration 016 added name_en/description_en to
-- accessory_sets and tea_sets but SKIPPED the general `sets` table, so the
-- admin "Sets" tab (strain + accessory bundles) had no English name/description
-- while the other two set types did — an asymmetry surfaced while fixing the
-- "sets aren't being added" bug (public /api/sets now also reads `sets`).
--
-- Symmetric, backward-compatible add (TEXT nullable, default NULL). Existing
-- rows read as NULL → non-RU users fall back to the RU primary via
-- lang::localized exactly as before.

ALTER TABLE sets ADD COLUMN IF NOT EXISTS name_en        TEXT;
ALTER TABLE sets ADD COLUMN IF NOT EXISTS description_en TEXT;
