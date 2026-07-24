-- Variant C: lab certificate lookup for strains (THC/CBD transparency).
-- Match the project's string-id convention (VARCHAR(36)); align with the
-- SeaORM entity that inserts application-generated UUID strings.
DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_name = 'lab_certificates'
          AND column_name = 'id'
          AND data_type = 'uuid'
    ) THEN
        ALTER TABLE lab_certificates ALTER COLUMN id TYPE CHARACTER VARYING(36) USING id::text;
    END IF;
END $$;

CREATE TABLE IF NOT EXISTS lab_certificates (
    id CHARACTER VARYING(36) PRIMARY KEY,
    strain_id CHARACTER VARYING(36) NOT NULL REFERENCES strains(id) ON DELETE CASCADE,
    certificate_url TEXT NOT NULL,
    tested_at DATE,
    thc_percent DOUBLE PRECISION,
    cbd_percent DOUBLE PRECISION,
    uploaded_by_telegram_id BIGINT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_lab_certificates_strain_id ON lab_certificates(strain_id);
CREATE INDEX IF NOT EXISTS idx_lab_certificates_tested_at ON lab_certificates(tested_at DESC);
