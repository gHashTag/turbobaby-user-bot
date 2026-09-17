-- The cannabis catalog and the garden mechanic leave the schema.
--
-- `strains` is the table the shop was built around, `strain_reviews` (049) and
-- `lab_certificates` (050) hang off it, and the garden trio (004) exists to
-- plant a virtual seed when somebody buys cannabis. A motorbike rental has no
-- botanical counterpart and the replacement game is the biker ride
-- (DECISIONS.md D5), so the mechanic is removed rather than repointed. Service
-- state does have a real counterpart and moved to `bike_service_records` in
-- 080 (D6).
--
-- Forward-only. Migrations 001, 004, 008, 026, 036, 037, 041, 049 and 050 stay
-- on disk untouched (D2): `run_migrations` backfills `_schema_migrations` for
-- an established database, so editing them in place would change fresh
-- installs only and leave production holding the cannabis catalog for ever.
-- On a fresh install those migrations create these tables a moment before this
-- one drops them, which is wasteful and correct.
--
-- =====================================================================
-- BEFORE THIS RUNS, `src/db/mod.rs:495` MUST NO LONGER PROBE `strains`.
-- =====================================================================
--
-- That line reads `SELECT to_regclass('public.strains') IS NOT NULL AS est`
-- and is how `run_migrations` decides whether a database is established. With
-- `strains` dropped and the tracker empty for any reason, the probe returns
-- false on a populated production database, the backfill branch is skipped,
-- the runner concludes the database is FRESH, and it applies 001-076 against
-- live data — including the seed migrations that resurrect deleted rows and
-- the price migrations that overwrite admin edits. D3 repoints it at
-- `public.orders`, created in `001_initial.sql:57`, which survives the rebrand
-- because a rental is still an order.

-- =====================================================================
-- GUARD: this migration assumed the database it ran on was the bike
-- database. It never checked. On 2026-09-13 11:49:12 UTC it ran against
-- the client's live cannabis shop — DATABASE_URL still pointed there —
-- and took out a catalog going back to April: Black Truffle, DIPZ,
-- MAC 1, V6 Haze, Pink Oreoz, Death Star OG and the rest. Those names
-- survive only because `sets.strains`, `strain_of_day_log` and
-- `user_plants` reference them by name; the rows themselves went.
--
-- A fresh install reaches this point with 12 seed strains from 008 and
-- an EMPTY `orders` table, because nobody has bought anything yet. A
-- live shop has both. That asymmetry is the discriminator: order history
-- is the one thing a fresh install cannot fake.
--
-- Escape hatch for a deliberate conversion of a shop that does have
-- history: create the marker table first, in the same database.
--
--     CREATE TABLE cannabis_drop_approved ();
--
-- Naming it by hand is the point — it cannot happen by deploying.
-- =====================================================================
DO $guard$
DECLARE
    live_strains bigint;
    live_orders  bigint;
BEGIN
    IF to_regclass('public.strains') IS NULL THEN
        RETURN;  -- already dropped, or never existed; nothing to protect
    END IF;
    IF to_regclass('public.cannabis_drop_approved') IS NOT NULL THEN
        RAISE NOTICE 'cannabis_drop_approved present - proceeding with the drop';
        RETURN;
    END IF;

    EXECUTE 'SELECT count(*) FROM public.strains' INTO live_strains;
    live_orders := 0;
    IF to_regclass('public.orders') IS NOT NULL THEN
        EXECUTE 'SELECT count(*) FROM public.orders' INTO live_orders;
    END IF;

    IF live_strains > 0 AND live_orders > 0 THEN
        RAISE EXCEPTION
            'REFUSING to drop the cannabis catalog: this database has % strains and % orders, so it is a live shop, not a fresh install. If the conversion is intended, run: CREATE TABLE cannabis_drop_approved ();',
            live_strains, live_orders;
    END IF;
END
$guard$;

-- Children first. `CASCADE` would reach them anyway through their foreign
-- keys, but naming them makes the drop list auditable rather than implied.
DROP TABLE IF EXISTS lab_certificates CASCADE;
DROP TABLE IF EXISTS strain_reviews CASCADE;
DROP TABLE IF EXISTS strains CASCADE;

-- The garden. `garden_plants.strain_id` / `garden_rewards.strain_id` are plain
-- TEXT with no foreign key, so nothing above cascaded into these.
DROP TABLE IF EXISTS garden_rewards CASCADE;
DROP TABLE IF EXISTS garden_plants CASCADE;
DROP TABLE IF EXISTS garden_config CASCADE;

-- Not dropped here, and why:
--
--   * The eight `garden_*` rows migration 066 seeded into `achievements`.
--     `achievements` is a shared table that the biker ride game (#12-#15) will
--     use, so this migration would be deleting another feature's rows on a
--     guess about what replaces them. They are dead weight, not a hazard.
--
--   * `cart_items` rows with `kind = 'strain'`. 081 keeps 'strain' in the kind
--     vocabulary precisely so a live cart holding one does not abort that
--     migration; clearing those lines is a decision about customer data, taken
--     by whoever owns the cart, not a side effect of a schema drop.
