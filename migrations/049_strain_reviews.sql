-- Variant C: post-delivery strain reviews for community trust / retention.
CREATE TABLE IF NOT EXISTS strain_reviews (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
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
