-- 089: the referral credit (owner, 2026-09-26, R3; DECISIONS.md "R1-R3, the owner's answers of
-- 2026-09-26"). 10% of every completed rental of an invited friend, excluding deposit and delivery,
-- credited to the inviter in whole baht, rounded down, when a manager records that rental; spent on
-- the inviter's own rental or paid out by a manager by hand. Additive only: three new tables. No
-- existing row is read, rewritten or deleted here, and loyalty points never enter these tables.
-- Forward-only (D2).

CREATE TABLE IF NOT EXISTS referral_rentals (
    id                   BIGSERIAL    PRIMARY KEY,
    customer_telegram_id BIGINT       NOT NULL,
    order_id             VARCHAR(36)  NULL,
    rental_amount_thb    BIGINT       NOT NULL CHECK (rental_amount_thb BETWEEN 1 AND 100000000),
    applied_thb          BIGINT       NOT NULL DEFAULT 0 CHECK (applied_thb >= 0),
    inviter_telegram_id  BIGINT       NULL,
    credit_thb           BIGINT       NOT NULL DEFAULT 0,
    note                 VARCHAR(200) NULL,
    idempotency_key      VARCHAR(100) NOT NULL UNIQUE,
    recorded_by          BIGINT       NOT NULL,
    recorded_at          TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
    reversed_at          TIMESTAMPTZ  NULL,
    reversed_by          BIGINT       NULL,
    reversal_note        VARCHAR(200) NULL,
    CONSTRAINT referral_rentals_applied_within_amount CHECK (applied_thb <= rental_amount_thb),
    CONSTRAINT referral_rentals_no_self_credit CHECK (inviter_telegram_id IS NULL OR inviter_telegram_id <> customer_telegram_id),
    CONSTRAINT referral_rentals_credit_is_the_rule CHECK (
           (inviter_telegram_id IS NULL AND credit_thb = 0)
        OR (inviter_telegram_id IS NOT NULL AND credit_thb = (rental_amount_thb - applied_thb) * 10 / 100)),
    CONSTRAINT referral_rentals_reversal_is_whole CHECK ((reversed_at IS NULL) = (reversed_by IS NULL))
);
CREATE UNIQUE INDEX IF NOT EXISTS referral_rentals_one_live_per_order
    ON referral_rentals (order_id) WHERE order_id IS NOT NULL AND reversed_at IS NULL;
CREATE INDEX IF NOT EXISTS referral_rentals_by_customer ON referral_rentals (customer_telegram_id);
CREATE INDEX IF NOT EXISTS referral_rentals_by_inviter ON referral_rentals (inviter_telegram_id) WHERE inviter_telegram_id IS NOT NULL;

CREATE TABLE IF NOT EXISTS referral_requests (
    id          BIGSERIAL    PRIMARY KEY,
    telegram_id BIGINT       NOT NULL,
    kind        VARCHAR(10)  NOT NULL CHECK (kind IN ('payout', 'redeem')),
    amount_thb  BIGINT       NOT NULL CHECK (amount_thb > 0),
    status      VARCHAR(10)  NOT NULL DEFAULT 'open' CHECK (status IN ('open', 'applied', 'paid', 'declined')),
    created_at  TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
    resolved_at TIMESTAMPTZ  NULL,
    resolved_by BIGINT       NULL,
    rental_id   BIGINT       NULL REFERENCES referral_rentals (id),
    applied_thb BIGINT       NULL,
    admin_note  VARCHAR(200) NULL,
    CONSTRAINT referral_requests_status_fits_kind CHECK (
           (status = 'open'     AND resolved_at IS NULL     AND rental_id IS NULL     AND applied_thb IS NULL)
        OR (status = 'declined' AND resolved_at IS NOT NULL AND rental_id IS NULL     AND applied_thb IS NULL)
        OR (status = 'paid'     AND kind = 'payout' AND resolved_at IS NOT NULL AND rental_id IS NULL AND applied_thb IS NULL)
        OR (status = 'applied'  AND kind = 'redeem' AND resolved_at IS NOT NULL AND rental_id IS NOT NULL
                                AND applied_thb BETWEEN 1 AND amount_thb))
);
CREATE UNIQUE INDEX IF NOT EXISTS referral_requests_one_open_per_person
    ON referral_requests (telegram_id) WHERE status = 'open';

CREATE TABLE IF NOT EXISTS referral_ledger (
    id          BIGSERIAL   PRIMARY KEY,
    telegram_id BIGINT      NOT NULL,
    kind        VARCHAR(20) NOT NULL CHECK (kind IN ('accrual', 'accrual_reversal', 'redeem', 'redeem_reversal', 'payout')),
    amount_thb  BIGINT      NOT NULL,
    rental_id   BIGINT      NULL REFERENCES referral_rentals (id),
    request_id  BIGINT      NULL REFERENCES referral_requests (id),
    created_by  BIGINT      NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT referral_ledger_shape CHECK (
           (kind = 'accrual'          AND amount_thb > 0 AND rental_id IS NOT NULL AND request_id IS NULL)
        OR (kind = 'accrual_reversal' AND amount_thb < 0 AND rental_id IS NOT NULL AND request_id IS NULL)
        OR (kind = 'redeem'           AND amount_thb < 0 AND rental_id IS NOT NULL AND request_id IS NOT NULL)
        OR (kind = 'redeem_reversal'  AND amount_thb > 0 AND rental_id IS NOT NULL AND request_id IS NULL)
        OR (kind = 'payout'           AND amount_thb < 0 AND rental_id IS NULL     AND request_id IS NOT NULL))
);
CREATE UNIQUE INDEX IF NOT EXISTS referral_ledger_once_per_rental  ON referral_ledger (kind, rental_id)  WHERE rental_id IS NOT NULL;
CREATE UNIQUE INDEX IF NOT EXISTS referral_ledger_once_per_request ON referral_ledger (kind, request_id) WHERE request_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS referral_ledger_by_person ON referral_ledger (telegram_id);
