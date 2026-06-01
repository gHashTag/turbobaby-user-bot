-- Cycle #20: persist Woody Catch high scores cross-device.
-- Previously stored only in localStorage; new device = lost record.
--
-- One row per user, stores the user's best (max) score.

CREATE TABLE IF NOT EXISTS game_high_scores (
    telegram_id BIGINT      PRIMARY KEY,
    high_score  INTEGER     NOT NULL DEFAULT 0,
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_game_high_scores_score
    ON game_high_scores (high_score DESC);
