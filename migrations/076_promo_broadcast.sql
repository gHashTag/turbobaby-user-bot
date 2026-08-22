-- The broadcast: publishing a post no longer requires an external channel.
--
-- The owner asked for the bot itself to carry the mailing to every client,
-- and PROMO_CHANNEL_ID has sat unset since the agent shipped (#96). The
-- Publish button therefore fans the post out as direct messages, and this
-- migration stores what that fan-out needs on the row it already had.
--
-- `link_payload` / `image_url`: the Publish button knows the post only by its
-- `dedup_key`. The deep-link payload cannot be rebuilt from the row — a
-- `bestseller` is about a strain or a set, and `kind` alone does not say
-- which — so the sweep records both at draft time, where the full `Subject`
-- is still in hand. Rows drafted before this migration have NULLs: their
-- broadcasts go out without the open button and the cover, which is better
-- than not going out.
ALTER TABLE promo_posts
    ADD COLUMN IF NOT EXISTS link_payload TEXT,
    ADD COLUMN IF NOT EXISTS image_url TEXT;

-- One row per client a published post was actually handed to.
--
-- Delivery, not intent: the recipients query runs before the send and a user
-- who blocked the bot is in it. Recording the outcome per person is what lets
-- the next broadcast report honestly ("N доставлено, M не дошло") instead of
-- guessing, and keeps a dead chat from being counted as a customer reached.
CREATE TABLE IF NOT EXISTS promo_deliveries (
    dedup_key   TEXT NOT NULL,
    telegram_id BIGINT NOT NULL,
    -- 'sent' | 'failed'
    status      TEXT NOT NULL,
    -- Why a send failed, when it did — the summary names the class of
    -- failure, not every user.
    detail      TEXT,
    sent_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (dedup_key, telegram_id),
    CONSTRAINT promo_deliveries_status CHECK (status IN ('sent', 'failed'))
);

CREATE INDEX IF NOT EXISTS idx_promo_deliveries_telegram_id
    ON promo_deliveries(telegram_id);
