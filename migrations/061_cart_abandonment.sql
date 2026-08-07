-- Loop #12: abandoned-cart recovery.
-- Tracks whether (and when) a customer has already received a reminder so we
-- don't spam them on every background sweep. A single reminder per cart is
-- enough; the cart is cleared or refreshed on the next order.
ALTER TABLE carts
    ADD COLUMN IF NOT EXISTS reminder_sent_at TIMESTAMPTZ;

ALTER TABLE carts
    ADD COLUMN IF NOT EXISTS reminder_count INT NOT NULL DEFAULT 0;
