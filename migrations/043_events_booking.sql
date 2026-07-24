-- Events Calendar + seat booking MVP.
-- Designed for free bookings in v1, with columns that let us add paid
-- bookings later (price_baht, order_id) without a follow-up migration.

CREATE TABLE IF NOT EXISTS events (
    id              VARCHAR(36) PRIMARY KEY DEFAULT gen_random_uuid(),
    title           VARCHAR(200) NOT NULL,
    title_en        VARCHAR(200),
    description     TEXT,
    description_en  TEXT,
    starts_at       TIMESTAMPTZ NOT NULL,
    ends_at         TIMESTAMPTZ,
    location_text   VARCHAR(300),
    image_url       VARCHAR(2048),
    max_seats       INTEGER,                  -- NULL = unlimited; UI shows "spots left"
    price_baht      NUMERIC(12,2),             -- NULL = free; future paid booking reserve
    is_public       BOOLEAN NOT NULL DEFAULT TRUE,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT events_max_seats_non_negative CHECK (max_seats IS NULL OR max_seats >= 0),
    CONSTRAINT events_price_non_negative CHECK (price_baht IS NULL OR price_baht >= 0)
);

CREATE INDEX IF NOT EXISTS idx_events_starts_at ON events(starts_at);
CREATE INDEX IF NOT EXISTS idx_events_public    ON events(is_public, starts_at);

CREATE TABLE IF NOT EXISTS event_bookings (
    id                VARCHAR(36) PRIMARY KEY DEFAULT gen_random_uuid(),
    event_id          VARCHAR(36) NOT NULL REFERENCES events(id) ON DELETE CASCADE,
    telegram_id       BIGINT NOT NULL,
    seats             INTEGER NOT NULL DEFAULT 1,
    status            VARCHAR(20) NOT NULL DEFAULT 'confirmed',
    idempotency_key   VARCHAR(100),
    order_id          VARCHAR(36),              -- reserved for future paid bookings
    created_at        TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at        TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT event_bookings_seats_positive CHECK (seats > 0),
    CONSTRAINT event_bookings_status CHECK (status IN ('confirmed', 'cancelled')),
    UNIQUE(event_id, telegram_id, idempotency_key)
);

CREATE INDEX IF NOT EXISTS idx_event_bookings_event ON event_bookings(event_id);
CREATE INDEX IF NOT EXISTS idx_event_bookings_user   ON event_bookings(telegram_id, created_at DESC);

-- Prevent overlapping active bookings for the same user on the same event
-- when idempotency_key is NULL (simple MVP guard against double-click dups).
CREATE UNIQUE INDEX IF NOT EXISTS idx_event_bookings_one_active_per_user
    ON event_bookings(event_id, telegram_id)
    WHERE status = 'confirmed' AND idempotency_key IS NULL;
