-- What the promoter has already written about.
--
-- The agent wakes up, looks for things it has not promoted, writes a post and
-- sends it to the owners. Without a record of what it has already covered it
-- would re-announce the whole catalogue on every tick — and the first tick
-- after a deploy would send one message per product in the shop.
--
-- `dedup_key` is `<kind>:<id>` from `crate::trios::promo::dedup_key`. The kind
-- is in the key, not just the id, because an event legitimately gets two posts:
-- one when it is announced and one the day before, with the seats left in it.
-- Keyed on the id alone the reminder — the post that matters most — would be
-- silently suppressed by the announcement.

CREATE TABLE IF NOT EXISTS promo_posts (
    id           VARCHAR(36) PRIMARY KEY DEFAULT gen_random_uuid(),
    -- `<kind>:<subject id>`. UNIQUE is the whole point of the table: it is what
    -- makes "promote this once" true under a restart, a crash mid-send, or two
    -- instances running at once during a rollover.
    dedup_key    TEXT NOT NULL UNIQUE,
    kind         TEXT NOT NULL,
    subject_id   TEXT NOT NULL,
    subject_name TEXT NOT NULL,
    -- The text that was actually sent, kept so the owner can see what went out
    -- and so a bad prompt can be diagnosed from the result rather than guessed.
    body         TEXT NOT NULL,
    -- Whether the model wrote it or the written fallback did. `GLM_API_KEY` is
    -- unset in production today, so this will read `fallback` until a key is
    -- set — and that distinction should be visible rather than inferred.
    source       TEXT NOT NULL DEFAULT 'fallback',
    -- Set when the post is handed to the owners for review.
    drafted_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    -- Set only when a human presses Publish. NULL means "written, not
    -- published" — a real state, and the one everything starts in.
    published_at TIMESTAMPTZ,
    -- Who pressed it. A public post under the shop's name should say who sent
    -- it.
    published_by BIGINT,
    CONSTRAINT promo_posts_kind_not_empty CHECK (kind <> ''),
    CONSTRAINT promo_posts_body_not_empty CHECK (body <> '')
);

-- The sweeper's only read: "have I covered this yet". The UNIQUE index above
-- already serves it, so no second index is added here.

-- Answering "what did the promoter send this week, and how much of it was
-- published" without scanning the table.
CREATE INDEX IF NOT EXISTS idx_promo_posts_drafted ON promo_posts(drafted_at DESC);

-- Do NOT backfill.
--
-- Every strain, set and event already in the shop is older than this agent and
-- is not news. Seeding the table would be the wrong fix for the same reason:
-- the sweeper is written to promote only rows created after it started
-- watching, so a shop with 137 products does not wake up to 137 drafts.
