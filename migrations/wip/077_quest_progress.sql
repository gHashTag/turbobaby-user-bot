-- Quest QR (owner request #4): the 10 checkpoints must be scanned in order, each
-- with its own QR, and progress must unlock ONLY after the correct scan and
-- SURVIVE app restarts. Before this, the checkpoint counter lived only in a
-- frontend signal (reset on close) and the scan endpoint did no ordering.
--
-- 1. sequence_order: the 1..N position of a checkpoint. 0 = legacy/unordered
--    (scan endpoint then skips the sequential gate for backward compatibility).
-- 2. user_quest_progress: one row per (user, scanned checkpoint). Server-side
--    source of truth for "which checkpoints this user has cleared".

ALTER TABLE location_quest_locations
    ADD COLUMN IF NOT EXISTS sequence_order INTEGER NOT NULL DEFAULT 0;

CREATE TABLE IF NOT EXISTS user_quest_progress (
    telegram_id BIGINT  NOT NULL,
    location_id INTEGER NOT NULL,
    scanned_at  BIGINT  NOT NULL,
    PRIMARY KEY (telegram_id, location_id)
);

CREATE INDEX IF NOT EXISTS idx_user_quest_progress_telegram
    ON user_quest_progress(telegram_id);
