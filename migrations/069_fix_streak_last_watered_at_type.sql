-- Fix: `POST /api/garden/plants/:id/water` returned HTTP 500 for every
-- request ("растения не поливаются").
--
-- Migration 065 created `streak_last_watered_at` / `streak_broken_at` as
-- TIMESTAMPTZ, but `src/api/garden.rs::water_plant` reads them as `i64` and
-- writes `chrono::Utc::now().timestamp_millis()` — the same epoch-millis
-- convention already used by `planted_at`, `last_watered_at` and
-- `harvested_at` on this table. Postgres rejected the UPDATE with
--   column "streak_last_watered_at" is of type timestamp with time zone
--   but expression is of type bigint
-- so the transaction aborted and the handler 500-ed before any water was
-- persisted.
--
-- Align the schema with the code (and with the rest of the table): store
-- epoch milliseconds as BIGINT, converting any existing rows in place.

ALTER TABLE garden_plants
    ALTER COLUMN streak_last_watered_at TYPE BIGINT
    USING (EXTRACT(EPOCH FROM streak_last_watered_at) * 1000)::BIGINT;

ALTER TABLE garden_plants
    ALTER COLUMN streak_broken_at TYPE BIGINT
    USING (EXTRACT(EPOCH FROM streak_broken_at) * 1000)::BIGINT;
