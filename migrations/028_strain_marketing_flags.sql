-- TZ #2 / Phase 1: marketing flags on strains.
--
-- Adds discount, sale, best-seller and new-arrival flags so the admin can
-- manage promotional state without code changes. Strain-of-the-day already
-- has its own columns from earlier migrations and stays untouched.
--
-- All new columns nullable / default-off so existing strains keep behaving
-- exactly as before until the admin opts in.

ALTER TABLE strains
    -- Sale / discount
    ADD COLUMN IF NOT EXISTS discount_percent DOUBLE PRECISION NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS sale_price       DOUBLE PRECISION,
    ADD COLUMN IF NOT EXISTS sale_active      BOOLEAN          NOT NULL DEFAULT FALSE,
    ADD COLUMN IF NOT EXISTS sale_until       TIMESTAMPTZ,

    -- Best seller (manual flag)
    ADD COLUMN IF NOT EXISTS is_best_seller   BOOLEAN          NOT NULL DEFAULT FALSE,

    -- New arrival (with optional expiry)
    ADD COLUMN IF NOT EXISTS is_new_arrival   BOOLEAN          NOT NULL DEFAULT FALSE,
    ADD COLUMN IF NOT EXISTS new_until        TIMESTAMPTZ,

    -- Manual sort order — lower number shows first within the same group
    ADD COLUMN IF NOT EXISTS display_order    INTEGER          NOT NULL DEFAULT 0;

-- Indexes that the menu-priority ORDER BY will use.
CREATE INDEX IF NOT EXISTS idx_strains_sale_active     ON strains(sale_active)     WHERE sale_active = TRUE;
CREATE INDEX IF NOT EXISTS idx_strains_is_best_seller  ON strains(is_best_seller)  WHERE is_best_seller = TRUE;
CREATE INDEX IF NOT EXISTS idx_strains_is_new_arrival  ON strains(is_new_arrival)  WHERE is_new_arrival = TRUE;
