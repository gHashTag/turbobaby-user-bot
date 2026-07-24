-- Variant C: lab certificate lookup for strains (THC/CBD transparency).
CREATE TABLE IF NOT EXISTS lab_certificates (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    strain_id CHARACTER VARYING(36) NOT NULL REFERENCES strains(id) ON DELETE CASCADE,
    certificate_url TEXT NOT NULL,
    tested_at DATE,
    thc_percent NUMERIC(5,2),
    cbd_percent NUMERIC(5,2),
    uploaded_by_telegram_id BIGINT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_lab_certificates_strain_id ON lab_certificates(strain_id);
CREATE INDEX IF NOT EXISTS idx_lab_certificates_tested_at ON lab_certificates(tested_at DESC);
