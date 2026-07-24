-- Variant C: post-delivery strain reviews for community trust / retention.
-- Match the project's string-id convention (VARCHAR(36)); align with the
-- SeaORM entity that inserts application-generated UUID strings.
DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_name = 'strain_reviews'
          AND column_name = 'id'
          AND data_type = 'uuid'
    ) THEN
        ALTER TABLE strain_reviews ALTER COLUMN id TYPE CHARACTER VARYING(36) USING id::text;
    END IF;
END $$;

CREATE TABLE IF NOT EXISTS strain_reviews (
    id CHARACTER VARYING(36) PRIMARY KEY,
    telegram_id BIGINT NOT NULL,
    strain_id CHARACTER VARYING(36) NOT NULL REFERENCES strains(id) ON DELETE CASCADE,
    order_id CHARACTER VARYING(36) NOT NULL REFERENCES orders(id) ON DELETE CASCADE,
    rating INTEGER NOT NULL CHECK (rating BETWEEN 1 AND 5),
    comment TEXT NOT NULL DEFAULT '',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    approved BOOLEAN NOT NULL DEFAULT TRUE
);

CREATE INDEX IF NOT EXISTS idx_strain_reviews_strain_id ON strain_reviews(strain_id);
CREATE INDEX IF NOT EXISTS idx_strain_reviews_order_id ON strain_reviews(order_id);
CREATE INDEX IF NOT EXISTS idx_strain_reviews_approved_created ON strain_reviews(approved, created_at DESC);
