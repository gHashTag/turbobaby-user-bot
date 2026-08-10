-- Store who booked, not just their numeric id.
--
-- `event_bookings` held only `telegram_id`, so the admin list of attendees was
-- a column of bare numbers with no way to reach the person. The owner needs a
-- clickable @username to message them.
--
-- Snapshotted onto the booking row rather than looked up at read time on
-- purpose: usernames change, and what the owner wants is a handle that
-- identifies the person who booked *this* seat.

ALTER TABLE event_bookings
    ADD COLUMN IF NOT EXISTS username   TEXT,
    ADD COLUMN IF NOT EXISTS first_name TEXT;

-- Backfill what we can for bookings made before this column existed. Orders
-- carry `customer_telegram`, which is the same handle captured from the Mini
-- App, so any customer who has ever ordered can be recovered. Everyone else
-- stays NULL and renders as a plain id link.
UPDATE event_bookings b
SET username = TRIM(LEADING '@' FROM o.customer_telegram)
FROM (
    SELECT DISTINCT ON (telegram_id) telegram_id, customer_telegram
    FROM orders
    WHERE telegram_id IS NOT NULL
      AND customer_telegram IS NOT NULL
      AND customer_telegram <> ''
    ORDER BY telegram_id, created_at DESC
) o
WHERE b.telegram_id = o.telegram_id
  AND b.username IS NULL;

-- First names live in `user_languages` (the entity is called `user`, but the
-- table is not `users` — referencing that name fails the whole migration and
-- every one after it).
UPDATE event_bookings b
SET first_name = u.first_name
FROM user_languages u
WHERE b.telegram_id = u.telegram_id
  AND b.first_name IS NULL
  AND u.first_name IS NOT NULL;
