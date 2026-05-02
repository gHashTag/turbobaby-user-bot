-- Garden config
CREATE TABLE IF NOT EXISTS garden_config (
    id INTEGER PRIMARY KEY DEFAULT 1,
    is_enabled BOOLEAN NOT NULL DEFAULT true,
    reward_discount_percent INTEGER NOT NULL DEFAULT 10,
    reward_bonus_points INTEGER NOT NULL DEFAULT 100,
    reward_expiration_days INTEGER NOT NULL DEFAULT 7,
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

-- Garden plants
CREATE TABLE IF NOT EXISTS garden_plants (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    strain_id TEXT NOT NULL,
    strain_name TEXT NOT NULL,
    current_stage VARCHAR(50) NOT NULL DEFAULT 'seed',
    planted_at BIGINT NOT NULL,
    is_completed BOOLEAN NOT NULL DEFAULT false,
    harvested_at BIGINT,
    reward_claimed BOOLEAN NOT NULL DEFAULT false,
    water_count INTEGER NOT NULL DEFAULT 0,
    last_watered_at BIGINT,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- Garden rewards
CREATE TABLE IF NOT EXISTS garden_rewards (
    id TEXT PRIMARY KEY,
    plant_id TEXT NOT NULL,
    user_id TEXT NOT NULL,
    strain_id TEXT NOT NULL,
    strain_name TEXT NOT NULL,
    discount_percent INTEGER NOT NULL DEFAULT 10,
    bonus_points INTEGER NOT NULL DEFAULT 100,
    expires_at BIGINT NOT NULL,
    is_used BOOLEAN NOT NULL DEFAULT false,
    created_at BIGINT NOT NULL
);

-- Insert default garden config
INSERT INTO garden_config (id, is_enabled, reward_discount_percent, reward_bonus_points, reward_expiration_days)
VALUES (1, true, 10, 100, 7)
ON CONFLICT (id) DO NOTHING;

-- Indexes
CREATE INDEX IF NOT EXISTS idx_garden_plants_user_id ON garden_plants(user_id);
CREATE INDEX IF NOT EXISTS idx_garden_plants_is_completed ON garden_plants(is_completed);
CREATE INDEX IF NOT EXISTS idx_garden_rewards_user_id ON garden_rewards(user_id);
CREATE INDEX IF NOT EXISTS idx_garden_rewards_is_used ON garden_rewards(is_used);
CREATE INDEX IF NOT EXISTS idx_garden_rewards_expires_at ON garden_rewards(expires_at);
