-- Service state per physical unit. ADMIN ONLY — never in the public catalog.
--
-- This is `lab_certificates`' real counterpart rather than a forced analogy
-- (DECISIONS.md D6): the fleet register tracks oil, gear oil, ABS and air
-- filter against kilometres, and the ops sheet carries current/last/interval/
-- next km with an overdue flag per unit.
--
-- Why it is not public: two units currently read `overdue`. A "serviced" badge
-- the shop cannot keep accurate is exactly the confident number the
-- data-honesty rule forbids, and unlike a missing price there is no honest dash
-- to render in its place. Nothing under `src/api/` may join this table into a
-- catalog response.
--
-- `current_km`, `last_service_km`, `interval_km` and `next_km` are all counted
-- since purchase, on the same footing as `bike_units.km_since_purchase`, and
-- are NOT odometer readings.
--
-- `service_type` and `status` are deliberately unconstrained TEXT. This is an
-- admin ledger transcribed from a spreadsheet whose vocabulary changes when the
-- shop starts tracking a new part; a CHECK would reject a real record and cost
-- a migration to widen. The money columns in 077 are constrained because a
-- customer reads them — this table no customer reads.
CREATE TABLE IF NOT EXISTS bike_service_records (
    id VARCHAR(36) PRIMARY KEY,
    bike_unit_id VARCHAR(36) NOT NULL
        REFERENCES bike_units(id) ON DELETE CASCADE,
    -- 'oil', 'gear_oil', 'abs', 'air_filter', …
    service_type TEXT NOT NULL,
    current_km INT,
    last_service_km INT,
    interval_km INT,
    next_km INT,
    -- 'ok' | 'due' | 'overdue', as the ops sheet words it.
    status TEXT NOT NULL,
    recorded_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- The per-unit service history, newest first.
CREATE INDEX IF NOT EXISTS idx_bike_service_records_bike_unit_id
    ON bike_service_records(bike_unit_id);
CREATE INDEX IF NOT EXISTS idx_bike_service_records_recorded_at
    ON bike_service_records(recorded_at DESC);
-- The admin "what is due" sweep.
CREATE INDEX IF NOT EXISTS idx_bike_service_records_status
    ON bike_service_records(status);

-- No seed. The de-identified seed file records that two units read `overdue`
-- but not which two, and carries no per-unit kilometres at all — the one
-- mileage figure it mentions is tied to an internal register row that this
-- repository has no key for. Inventing a service history to populate an empty
-- table would put numbers a mechanic will act on into the database on no
-- evidence, so the table ships empty and is filled by the admin surface.
