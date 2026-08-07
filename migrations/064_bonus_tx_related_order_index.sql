-- Loop #14: speed up order-detail cashback lookup so the success screen and
-- order history can read the earned cashback without scanning the whole
-- bonus_transactions table.
CREATE INDEX IF NOT EXISTS idx_bonus_transactions_related_order_id
    ON bonus_transactions(related_order_id)
    WHERE related_order_id IS NOT NULL;
