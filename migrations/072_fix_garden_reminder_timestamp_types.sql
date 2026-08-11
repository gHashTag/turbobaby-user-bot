-- Fix: both garden background sweeps died on every tick.
--
--   garden reminders sweep failed: ... operator does not exist:
--     timestamp with time zone <= bigint
--   garden reward expiry sweep failed: ... (same)
--
-- `src/api/garden.rs` compares and writes these two columns as epoch
-- milliseconds — the convention already used by every other time column on
-- these tables (`planted_at`, `last_watered_at`, `harvested_at`,
-- `expires_at`, and `streak_last_watered_at` since migration 069). But
-- migration 058 created `garden_plants.reminder_sent_at` and migration 065
-- created `garden_rewards.expiry_nudge_sent_at` as TIMESTAMPTZ, so Postgres
-- rejected the WHERE clause and the whole sweep returned an error before
-- doing any work.
--
-- The consequence was silent: watering reminders were never sent, and
-- rewards nearing expiry were never nudged, for as long as both columns have
-- existed. Nothing 500-ed, because the sweeps only log a warning.
--
-- This is the same defect as migration 069, on the two columns that fix
-- missed. Align the schema with the code rather than the other way round, so
-- one table does not carry two time conventions.

-- Guarded on the current type. The runner applies each migration exactly once,
-- but these files also get run by hand with `psql -f` against a scratch
-- database, and `EXTRACT(EPOCH FROM <bigint>)` is an error — so an unguarded
-- re-run would fail rather than no-op.
DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_name = 'garden_plants'
          AND column_name = 'reminder_sent_at'
          AND data_type = 'timestamp with time zone'
    ) THEN
        ALTER TABLE garden_plants
            ALTER COLUMN reminder_sent_at TYPE BIGINT
            USING (EXTRACT(EPOCH FROM reminder_sent_at) * 1000)::BIGINT;
    END IF;

    IF EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_name = 'garden_rewards'
          AND column_name = 'expiry_nudge_sent_at'
          AND data_type = 'timestamp with time zone'
    ) THEN
        ALTER TABLE garden_rewards
            ALTER COLUMN expiry_nudge_sent_at TYPE BIGINT
            USING (EXTRACT(EPOCH FROM expiry_nudge_sent_at) * 1000)::BIGINT;
    END IF;
END $$;
