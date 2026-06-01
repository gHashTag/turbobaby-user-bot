-- Idempotency keys for `POST /api/orders`.
--
-- Solves the Two Generals problem in the checkout path: when the client cannot
-- tell whether a 200 OK was lost in transit, it retries the same logical
-- request and used to risk a duplicate order. Now the client sends a stable
-- `X-Idempotency-Key` (UUID v4) for the duration of one logical submit; the
-- server keys off it so the second attempt returns the original order_id
-- instead of inserting a second row.
--
-- Design notes:
--   * Standalone table, no FK to `orders` — lets the server claim the key
--     *before* the order row is inserted, inside the same transaction. The
--     UNIQUE constraint on `key` is what guarantees mutual exclusion across
--     concurrent retries. If the order INSERT then fails, the whole tx rolls
--     back including this row.
--   * Global PK on `key` (not composite with telegram_id) because UUID v4
--     collisions across users are astronomically unlikely (~2^-122) and the
--     simpler schema is easier to reason about.
--   * `created_at` indexed for a future TTL sweep (keys older than 24h could
--     be purged — not done in this migration, just keeping the option open).

CREATE TABLE IF NOT EXISTS order_idempotency_keys (
    key          TEXT        PRIMARY KEY,
    order_id     TEXT        NOT NULL,
    telegram_id  BIGINT,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_idempotency_keys_created_at
    ON order_idempotency_keys(created_at);
