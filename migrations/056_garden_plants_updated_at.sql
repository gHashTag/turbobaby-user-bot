-- Cycle #169G: garden_plants gained updated_at writes in 041/force_seed/reset flows,
-- but the column was never added to the table created in 004. Adds it now and
-- backfills existing rows so UPDATEs no longer 500 with "column does not exist".
ALTER TABLE garden_plants
    ADD COLUMN IF NOT EXISTS updated_at TIMESTAMPTZ DEFAULT NOW();

UPDATE garden_plants
SET updated_at = COALESCE(
    to_timestamp(planted_at / 1000.0) AT TIME ZONE 'UTC',
    created_at,
    NOW()
)
WHERE updated_at IS NULL;
