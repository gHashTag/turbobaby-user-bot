-- Loop #18: garden social & visual upgrade tables.

-- Per-user achievement unlocks. achievements catalog already exists (010).
CREATE TABLE IF NOT EXISTS user_achievements (
    telegram_id    BIGINT       NOT NULL,
    achievement_id VARCHAR(50)  NOT NULL,
    unlocked_at    TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
    notified       BOOLEAN      NOT NULL DEFAULT false,
    PRIMARY KEY (telegram_id, achievement_id)
);

CREATE INDEX IF NOT EXISTS idx_user_achievements_user
    ON user_achievements(telegram_id, unlocked_at DESC);

-- Share events for viral attribution: what was shared, by whom, where.
CREATE TABLE IF NOT EXISTS share_events (
    id           TEXT PRIMARY KEY,
    telegram_id  BIGINT       NOT NULL,
    channel      VARCHAR(20)  NOT NULL,  -- telegram_chat, story, copy, etc.
    content_kind VARCHAR(30)  NOT NULL,  -- garden_plant, garden_reward, product, etc.
    content_id   TEXT         NOT NULL,
    shared_at    TIMESTAMPTZ  NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_share_events_user
    ON share_events(telegram_id, shared_at DESC);
CREATE INDEX IF NOT EXISTS idx_share_events_content
    ON share_events(content_kind, content_id);

-- Extend game_high_scores with a display name hint (not real username).
-- Existing rows remain valid because the column is added with default.
ALTER TABLE game_high_scores
    ADD COLUMN IF NOT EXISTS display_name TEXT NOT NULL DEFAULT 'Player';

-- Garden-specific achievements. Use ON CONFLICT so the seed is idempotent.
INSERT INTO achievements (id, name, description, icon, xp_reward, requirement, category)
VALUES
    ('garden_first_water',  'First Water',       'Watered your first plant',                  '💧', 10, 'water_count >= 1',            'garden'),
    ('garden_first_harvest','First Harvest',     'Harvested your first plant',                '🏆', 25, 'harvest_count >= 1',          'garden'),
    ('garden_streak_3',     'Growing Streak',    'Watered 3 days in a row',                   '🔥', 15, 'max_streak >= 3',             'garden'),
    ('garden_streak_7',     'Weekly Grower',     'Watered 7 days in a row',                   '🌿', 35, 'max_streak >= 7',             'garden'),
    ('garden_streak_14',    'Fortnight Farmer',  'Watered 14 days in a row',                  '🌳', 75, 'max_streak >= 14',            'garden'),
    ('garden_harvest_5',    'Green Thumb',       'Harvested 5 plants',                        '🧤', 50, 'harvest_count >= 5',          'garden'),
    ('garden_harvest_25',   'Master Grower',     'Harvested 25 plants',                       '👑', 150, 'harvest_count >= 25',         'garden'),
    ('garden_zero_miss',    'Perfect Grow',      'Reached harvest without missing a day',       '⭐', 50, 'max_streak >= total_waters',  'garden')
ON CONFLICT (id) DO UPDATE SET
    name = EXCLUDED.name,
    description = EXCLUDED.description,
    icon = EXCLUDED.icon,
    xp_reward = EXCLUDED.xp_reward,
    requirement = EXCLUDED.requirement,
    category = EXCLUDED.category;
