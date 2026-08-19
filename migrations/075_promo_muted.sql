-- Who asked the promoter to stop.
--
-- The Publish button and this one live in the same message: an owner who wants
-- the drafts to stop and has to go looking for how will instead learn to ignore
-- them, which is the same outcome with none of the signal.
--
-- A row per silenced admin rather than a flag on the agent, because one owner
-- muting it must not silence the others.
CREATE TABLE IF NOT EXISTS promo_muted (
    telegram_id BIGINT PRIMARY KEY,
    muted_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
