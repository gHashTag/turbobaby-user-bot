-- Event media gallery: multiple photos + one video URL per event.
-- Keeps image_url as the primary cover/thumbnail while the gallery
-- is rendered as a carousel on the event detail page.

ALTER TABLE events ADD COLUMN IF NOT EXISTS video_url VARCHAR(2048);

CREATE TABLE IF NOT EXISTS event_photos (
    id            VARCHAR(36) PRIMARY KEY DEFAULT gen_random_uuid(),
    event_id      VARCHAR(36) NOT NULL REFERENCES events(id) ON DELETE CASCADE,
    url           VARCHAR(2048) NOT NULL,
    display_order INTEGER NOT NULL DEFAULT 0,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT event_photos_url_not_empty CHECK (url <> '')
);

CREATE INDEX IF NOT EXISTS idx_event_photos_event_order ON event_photos(event_id, display_order);
