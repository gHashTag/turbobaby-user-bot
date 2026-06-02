-- Idempotency keys for `POST /api/loyalty/:telegram_id/add-bonus` and the
-- matching `use-bonus`. Cycle #158 phase 1 (memory/idempotency-loyalty-deferred.md).
--
-- Same shape as migration 029 (`order_idempotency_keys`) but separate table:
-- order and loyalty are different resources, and keeping their key spaces
-- distinct avoids any "did this key belong to an order or a bonus?" ambiguity
-- in audit logs.
--
-- The matching `tx_id` is the `bonus_transactions.id` UUID that the original
-- (non-retried) request inserted. When a retry presents the same X-Idempotency-Key,
-- the handler returns this tx_id instead of inserting a second bonus row.
--
-- See also:
--   * migrations/029_order_idempotency_keys.sql — design template
--   * src/api/orders.rs::create_order (cycle #57) — reference handler shape
--   * src/db/entities/loyalty_idempotency_key.rs — sibling entity

CREATE TABLE IF NOT EXISTS loyalty_idempotency_keys (
    key          TEXT        PRIMARY KEY,
    tx_id        TEXT        NOT NULL,
    telegram_id  BIGINT      NOT NULL,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_loyalty_idempotency_keys_created_at
    ON loyalty_idempotency_keys(created_at);
