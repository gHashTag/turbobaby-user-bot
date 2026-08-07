-- Loop #11: dynamic delivery zones so fees/ETA/min-order can change without redeploy.
CREATE TABLE IF NOT EXISTS delivery_zones (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT NOT NULL,
    name_en TEXT,
    fee DOUBLE PRECISION NOT NULL DEFAULT 0 CHECK (fee >= 0),
    min_order DOUBLE PRECISION NOT NULL DEFAULT 0 CHECK (min_order >= 0),
    eta_min INT NOT NULL DEFAULT 30 CHECK (eta_min >= 0),
    eta_max INT NOT NULL DEFAULT 60 CHECK (eta_max >= 0),
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    sort_order INT NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Seed a default "Pattaya" zone matching the previous hard-coded fallback so
-- existing checkouts keep working immediately after migration.
INSERT INTO delivery_zones (name, name_en, fee, min_order, eta_min, eta_max, sort_order)
VALUES ('Паттайя', 'Pattaya', 0, 0, 30, 45, 0)
ON CONFLICT DO NOTHING;
