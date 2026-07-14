-- Stars (⭐) internal currency.
-- Game earns Stars, Telegram shop (Plot) spends them. Single server-side balance.
--
-- user_stars: per-user balance (telegram_id is canonical identity).
-- stars_transactions: append-only ledger for audit and history.
-- stars_idempotency_keys: deduplication of game credit requests by external_tx_id.

CREATE TABLE IF NOT EXISTS user_stars (
    telegram_id     BIGINT PRIMARY KEY REFERENCES loyalty_profiles(telegram_id) ON DELETE CASCADE,
    balance         BIGINT NOT NULL DEFAULT 0,
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_user_stars_balance ON user_stars(balance);

CREATE TABLE IF NOT EXISTS stars_transactions (
    id              VARCHAR(36) PRIMARY KEY,
    telegram_id     BIGINT NOT NULL,
    amount          BIGINT NOT NULL,            -- positive = credit, negative = debit
    balance_after   BIGINT NOT NULL,            -- snapshot after this tx
    source          VARCHAR(50) NOT NULL,       -- 'woodshop', 'garden', 'plot', etc
    reason          VARCHAR(100) NOT NULL,      -- 'level_complete', 'purchase', etc
    external_tx_id  VARCHAR(100),               -- game-generated unique id for credits
    related_order_id VARCHAR(36),               -- for debits tied to an order
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_stars_transactions_user_created
    ON stars_transactions(telegram_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_stars_transactions_external
    ON stars_transactions(external_tx_id) WHERE external_tx_id IS NOT NULL;

CREATE TABLE IF NOT EXISTS stars_idempotency_keys (
    external_tx_id  VARCHAR(100) PRIMARY KEY,
    telegram_id     BIGINT NOT NULL,
    amount          BIGINT NOT NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_stars_idempotency_created
    ON stars_idempotency_keys(created_at);

-- Stars can be spent at checkout; store how many were applied so a
-- rejected order can refund them exactly.
ALTER TABLE orders ADD COLUMN IF NOT EXISTS stars_used BIGINT NOT NULL DEFAULT 0;
