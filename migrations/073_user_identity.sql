-- Record who a person is, so the garden can stop calling everybody "Friend".
--
-- The friends panel printed `Friend #6794` for every invitee. The formatting
-- was never the problem: `user_languages.first_name` was NULL for essentially
-- everybody, because the one function that writes it — `save_user_name` — had
-- no callers anywhere in the codebase. `COALESCE(first_name, 'Friend')` did
-- the only thing it could with an empty column.
--
-- `user_languages` is this project's user table (there is no `users`). It held
-- `first_name` and nothing else about identity, so even once names started
-- being written there was nowhere to put a surname or a handle.

ALTER TABLE user_languages
    ADD COLUMN IF NOT EXISTS last_name TEXT,
    ADD COLUMN IF NOT EXISTS username  TEXT;

-- Backfill from what the shop already knows.
--
-- `orders.customer_telegram` stores the handle with its leading `@` — migration
-- 070 had to strip it for the same reason. This is the only historical source
-- of a username in the database, and it covers exactly the people who have
-- ordered, which is a subset of who appears in a friends list. Everybody else
-- gets a name the first time they touch the bot from now on.
--
-- `DISTINCT ON` picks the most recent order per customer: a username can be
-- changed, and the newest row is the closest thing to current. Rows whose
-- handle is not a Telegram handle (free text somebody typed into the field)
-- are left alone rather than printed with an `@` in front.
UPDATE user_languages ul
SET username = src.handle
FROM (
    SELECT DISTINCT ON (o.telegram_id)
           o.telegram_id,
           TRIM(LEADING '@' FROM o.customer_telegram) AS handle
    FROM orders o
    WHERE o.telegram_id IS NOT NULL
      AND o.customer_telegram IS NOT NULL
      AND TRIM(LEADING '@' FROM o.customer_telegram) ~ '^[A-Za-z0-9_]{1,32}$'
    ORDER BY o.telegram_id, o.created_at DESC
) src
WHERE ul.telegram_id = src.telegram_id
  AND ul.username IS NULL;

-- Same for the name. `orders.customer_name` is what the customer typed at
-- checkout — it is their name, given deliberately, and it is the only place in
-- this database where most people's names exist at all. Only rows with no name
-- are filled, so a name recorded from Telegram always wins over an old order.
UPDATE user_languages ul
SET first_name = src.name
FROM (
    SELECT DISTINCT ON (o.telegram_id)
           o.telegram_id,
           BTRIM(o.customer_name) AS name
    FROM orders o
    WHERE o.customer_name IS NOT NULL
      AND BTRIM(o.customer_name) <> ''
    ORDER BY o.telegram_id, o.created_at DESC
) src
WHERE ul.telegram_id = src.telegram_id
  AND (ul.first_name IS NULL OR BTRIM(ul.first_name) = '');

-- The invitees query joins on `telegram_id`, which is already the primary key,
-- so no index is needed here. This comment exists so the next person does not
-- add one "just in case".
