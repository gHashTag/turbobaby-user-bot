-- Fraud / sanity-check audit log for the create_order endpoint (cycle #44).
--
-- Cycles #39–#42 wired server-side price authority + structured tracing logs,
-- but those logs live in stdout: an admin opening Telegram cannot see them.
-- This table records one row per rejected create_order attempt so /engage can
-- render a "Suspicious activity (24h)" panel and operators can spot patterns
-- (e.g. one telegram_id repeatedly tampering subtotals).
--
-- Design notes:
--   * Single denormalized table — fraud events are append-only, low cardinality,
--     and never updated; star schema would be overkill.
--   * `code` mirrors the wire-contract codes returned to the client
--     (`subtotal_mismatch`, `unknown_item`, `unavailable`, `malformed`).
--   * `expected_subtotal` is *server* truth and never leaked to the client
--     (anti price-probing). Stored only here, for forensics.
--   * Index on `(created_at DESC)` makes the 24h-window aggregation cheap.

CREATE TABLE IF NOT EXISTS order_fraud_events (
    id                BIGSERIAL    PRIMARY KEY,
    created_at        TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
    telegram_id       BIGINT,
    code              TEXT         NOT NULL,
    catalog           TEXT,
    item_id           TEXT,
    claimed_subtotal  DOUBLE PRECISION,
    expected_subtotal DOUBLE PRECISION
);

CREATE INDEX IF NOT EXISTS idx_fraud_events_created_at
    ON order_fraud_events(created_at DESC);

CREATE INDEX IF NOT EXISTS idx_fraud_events_telegram_id_created
    ON order_fraud_events(telegram_id, created_at DESC)
    WHERE telegram_id IS NOT NULL;
