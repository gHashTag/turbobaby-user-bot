-- Cycle fix: lab_certificates thc/cbd were created as NUMERIC, but the
-- SeaORM entity maps them to f64 (FLOAT8). Convert idempotently.
DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_name = 'lab_certificates'
          AND column_name = 'thc_percent'
          AND data_type = 'numeric'
    ) THEN
        ALTER TABLE lab_certificates ALTER COLUMN thc_percent TYPE DOUBLE PRECISION USING thc_percent::double precision;
    END IF;

    IF EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_name = 'lab_certificates'
          AND column_name = 'cbd_percent'
          AND data_type = 'numeric'
    ) THEN
        ALTER TABLE lab_certificates ALTER COLUMN cbd_percent TYPE DOUBLE PRECISION USING cbd_percent::double precision;
    END IF;
END $$;
