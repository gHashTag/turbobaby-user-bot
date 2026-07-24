-- A1: paid event bookings using internal Stars currency.
-- Adds an event price in Stars and records the Stars tx id used to pay.

ALTER TABLE events ADD COLUMN IF NOT EXISTS price_stars BIGINT;

ALTER TABLE event_bookings ADD COLUMN IF NOT EXISTS stars_paid BIGINT;
ALTER TABLE event_bookings ADD COLUMN IF NOT EXISTS stars_tx_id VARCHAR(36);

-- Paid bookings must record how many Stars were charged so refunds/cancellations
-- can return the exact amount later. NULL means free booking.
-- Stars must be non-negative when present.
ALTER TABLE event_bookings DROP CONSTRAINT IF EXISTS event_bookings_stars_paid_non_negative;
ALTER TABLE event_bookings ADD CONSTRAINT event_bookings_stars_paid_non_negative CHECK (stars_paid IS NULL OR stars_paid >= 0);
