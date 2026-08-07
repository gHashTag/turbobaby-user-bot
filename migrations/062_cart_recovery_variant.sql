-- Loop #13: cart recovery A/B variant and deeper sequencing.
-- Stores which reminder copy/timing variant a cart was assigned to, and the
-- campaign source/start_param that brought the customer into the cart so we
-- can correlate sends → opens → orders.
ALTER TABLE carts
    ADD COLUMN IF NOT EXISTS reminder_variant TEXT NOT NULL DEFAULT 'v1';

ALTER TABLE carts
    ADD COLUMN IF NOT EXISTS start_param TEXT;

ALTER TABLE carts
    ADD COLUMN IF NOT EXISTS first_reminder_sent_at TIMESTAMPTZ;

-- Index for the second-nudge sweep: carts with exactly one reminder that is
-- old enough to merit a soft-perk escalation.
CREATE INDEX IF NOT EXISTS idx_carts_second_nudge
    ON carts(reminder_count, first_reminder_sent_at)
    WHERE reminder_count = 1;
