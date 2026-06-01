-- Audit trail for user-block state transitions (cycle #46).
--
-- Cycle #45 added automatic blocking when a telegram_id accumulates ≥5
-- `subtotal_mismatch` fraud events in 24 h, flipping
-- `loyalty_profiles.is_blocked = true`. But the *event* of being blocked is
-- not recorded anywhere queryable — only the resulting state. This table
-- captures every transition (auto-block by the server, manual unblock by an
-- admin via `/unblock`) so:
--   * /engage can show "N auto-blocks today, M unblocks"
--   * support can answer "why is this user blocked?"
--   * admins can be held accountable for unblock decisions
--
-- Design notes:
--   * Append-only — never UPDATE/DELETE rows here.
--   * `action` is a small enum-like text. Keep stable: 'auto_block', 'unblock'.
--   * `actor_admin_id` is NULL for server-initiated `auto_block`, the admin's
--     telegram_id for manual `unblock`.
--   * Index on `(telegram_id, created_at DESC)` so /engage and support
--     queries scan only the rows for one user.

CREATE TABLE IF NOT EXISTS block_history (
    id              BIGSERIAL    PRIMARY KEY,
    created_at      TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
    telegram_id     BIGINT       NOT NULL,
    action          TEXT         NOT NULL,
    reason          TEXT,
    actor_admin_id  BIGINT
);

CREATE INDEX IF NOT EXISTS idx_block_history_user_created
    ON block_history(telegram_id, created_at DESC);
