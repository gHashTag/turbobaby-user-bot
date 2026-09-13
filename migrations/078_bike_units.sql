-- A physical bike. `bikes` (077) is what a customer chooses; this is what the
-- shop hands over, and what availability is counted from.
--
-- `unit_code` is a synthetic slot label ('nmax-155-01'), NOT a plate number.
-- DECISIONS.md D14 bars plate numbers from this repository in any form, and a
-- plate is the one identifier that makes a machine — and through the rental
-- history, a renter — personally traceable. The register's own row identifiers
-- are not used either, for the same reason.
--
-- `km_since_purchase` counts kilometres travelled since TurboBaby acquired the
-- unit. It is NOT an odometer reading: the register has a unit reading 2900
-- since purchase against 16433 on the clock, so the two numbers are not
-- interchangeable and this column must never be labelled "mileage" or
-- "odometer" in the UI.
--
-- `status` carries the four states the fleet actually has. `rented` and
-- `available` come from the register; `service` and `retired` exist so a unit
-- can leave the available pool without being deleted and orphaning its service
-- history in 080.
CREATE TABLE IF NOT EXISTS bike_units (
    id VARCHAR(36) PRIMARY KEY DEFAULT gen_random_uuid()::text,
    bike_id VARCHAR(36) NOT NULL REFERENCES bikes(id) ON DELETE CASCADE,
    unit_code TEXT NOT NULL UNIQUE,
    -- Both NULLABLE: the register lists the years and colours a family holds
    -- without saying which machine is which, so a slot the evidence does not
    -- reach stays absent rather than borrowing its neighbour's value. See the
    -- distribution rule in 082.
    model_year INT,
    color TEXT,
    km_since_purchase INT,
    status TEXT NOT NULL
        CHECK (status IN ('available', 'rented', 'service', 'retired')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    -- A new unit legitimately reads 0 km since purchase, so 0 is allowed here
    -- and NULL still means "not recorded".
    CONSTRAINT bike_units_km_non_negative
        CHECK (km_since_purchase IS NULL OR km_since_purchase >= 0)
);

-- The per-family unit list and the available-count aggregate.
CREATE INDEX IF NOT EXISTS idx_bike_units_bike_id ON bike_units(bike_id);
CREATE INDEX IF NOT EXISTS idx_bike_units_status ON bike_units(status);
-- No `idx_bike_units_unit_code`: `unit_code TEXT NOT NULL UNIQUE` already
-- builds a unique index over it.
