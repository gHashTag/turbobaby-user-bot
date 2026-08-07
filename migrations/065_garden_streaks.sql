-- Loop #17: garden streak tracking and reminder/expiry state.
ALTER TABLE garden_plants
    ADD COLUMN IF NOT EXISTS streak INTEGER NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS max_streak INTEGER NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS streak_last_watered_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS streak_broken_at TIMESTAMPTZ;

-- Track whether an expiry nudge was already sent for a reward to avoid spam.
ALTER TABLE garden_rewards
    ADD COLUMN IF NOT EXISTS expiry_nudge_sent_at TIMESTAMPTZ;

-- Index for the reminder sweep: active plants needing a water/harvest nudge.
CREATE INDEX IF NOT EXISTS idx_garden_plants_streak
    ON garden_plants(user_id, streak, max_streak)
    WHERE is_completed = false AND harvested_at IS NULL;

-- Index for the expiry sweep: active rewards nearing expiration.
CREATE INDEX IF NOT EXISTS idx_garden_rewards_expiry_nudge
    ON garden_rewards(user_id, expires_at)
    WHERE is_used = false;
