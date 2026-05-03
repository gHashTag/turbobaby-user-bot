-- Migration 007: Referral Events Table
-- Full referral tracking system

CREATE TABLE IF NOT EXISTS referral_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    referrer_id BIGINT NOT NULL,
    referred_id BIGINT NOT NULL UNIQUE,
    code VARCHAR(20) NOT NULL,
    bonus_paid DOUBLE PRECISION NOT NULL DEFAULT 0,
    status VARCHAR(20) NOT NULL DEFAULT 'pending',  -- pending / confirmed / paid
    source VARCHAR(50),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    confirmed_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_referral_events_referrer_id ON referral_events(referrer_id);
CREATE INDEX IF NOT EXISTS idx_referral_events_referred_id ON referral_events(referred_id);
CREATE INDEX IF NOT EXISTS idx_referral_events_status ON referral_events(status);

-- Backfill: import existing loyalty_profiles referral relationships
INSERT INTO referral_events (referrer_id, referred_id, code, status, created_at, confirmed_at)
SELECT
    lp.referred_by AS referrer_id,
    lp.telegram_id AS referred_id,
    COALESCE(ref.referral_code, 'legacy') AS code,
    'confirmed' AS status,
    lp.created_at,
    lp.created_at AS confirmed_at
FROM loyalty_profiles lp
JOIN loyalty_profiles ref ON ref.telegram_id = lp.referred_by
WHERE lp.referred_by IS NOT NULL
ON CONFLICT (referred_id) DO NOTHING;
