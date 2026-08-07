-- Loop #10: garden push reminders need a timestamp per plant so we
-- don't spam users every time the 6-hour loop runs.
ALTER TABLE garden_plants ADD COLUMN IF NOT EXISTS reminder_sent_at TIMESTAMPTZ;

CREATE INDEX IF NOT EXISTS idx_garden_plants_reminder_sent_at
    ON garden_plants(user_id, reminder_sent_at);
