-- Cycle #168: event bookings can also be waitlisted when capacity is full.
-- Idempotent: drop the old two-value check and replace it with one that
-- includes 'waitlisted'. Safe to re-run every boot because of IF EXISTS/IF NOT EXISTS.
ALTER TABLE event_bookings
    DROP CONSTRAINT IF EXISTS event_bookings_status;

ALTER TABLE event_bookings
    ADD CONSTRAINT event_bookings_status
    CHECK (status IN ('confirmed', 'cancelled', 'waitlisted'));
