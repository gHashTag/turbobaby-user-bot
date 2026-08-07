-- Loop #21: referral milestone awards for 1 / 3 / 5 confirmed invitees.
-- Each milestone is idempotent (UNIQUE referrer_id + milestone) and can carry
-- an extra bonus credit on top of the per-referral payout.
CREATE TABLE IF NOT EXISTS referral_milestones (
    referrer_id BIGINT NOT NULL,
    milestone INT NOT NULL,
    bonus_amount DOUBLE PRECISION NOT NULL DEFAULT 0,
    achieved_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (referrer_id, milestone)
);
