-- Cycle #XXX: add free-text delivery fields so the real checkout flow can
-- capture address + notes for a cannabis-delivery mini-app in Thailand.
-- ALTER TABLE ADD COLUMN IF NOT EXISTS is safe under the migration runner
-- that re-runs the full concat on every boot.
ALTER TABLE orders ADD COLUMN IF NOT EXISTS delivery_address TEXT;
ALTER TABLE orders ADD COLUMN IF NOT EXISTS delivery_notes TEXT;
