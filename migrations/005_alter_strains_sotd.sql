-- Fix missing columns in strains table (idempotent)
ALTER TABLE strains ADD COLUMN IF NOT EXISTS strain_of_day_set_at TIMESTAMPTZ;
ALTER TABLE strains ADD COLUMN IF NOT EXISTS is_strain_of_day BOOLEAN NOT NULL DEFAULT false;
ALTER TABLE strains ADD COLUMN IF NOT EXISTS strain_of_day_discount DOUBLE PRECISION NOT NULL DEFAULT 0;
