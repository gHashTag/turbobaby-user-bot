-- A3: event reminders sent before the event starts.
-- Track which confirmed bookings already got a reminder so we don't spam users.

ALTER TABLE event_bookings ADD COLUMN IF NOT EXISTS reminder_sent_at TIMESTAMPTZ;

CREATE INDEX IF NOT EXISTS idx_event_bookings_reminder_pending
    ON event_bookings(event_id, telegram_id)
    WHERE status = 'confirmed' AND reminder_sent_at IS NULL;
