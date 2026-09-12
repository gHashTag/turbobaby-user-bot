-- A cart line can now be a bike rental, and the same family can sit on one cart
-- twice for two different date ranges.
--
-- `migrations/059_cart_tables.sql:24` declares `UNIQUE (cart_id, kind,
-- catalog_id)`. A customer taking an NMAX for this week and another NMAX for
-- next month has one family and two lines, and that constraint collapses them
-- into one — so it widens to include the dates (DECISIONS.md D8).
--
-- On a rental line `catalog_id` is the `bikes.key` FAMILY key, never a
-- `bike_units.unit_code`: the customer books a model and the shop assigns the
-- machine at handover. `quantity` is how many units of that family.

ALTER TABLE cart_items
    ADD COLUMN IF NOT EXISTS rental_start DATE,
    ADD COLUMN IF NOT EXISTS rental_end   DATE;

-- Widen the `kind` vocabulary to admit 'bike_rental'.
--
-- 059 declared that CHECK inline and unnamed, so its name is whatever Postgres
-- generated. It is found here by the column it constrains rather than by a
-- guessed name, because a database that was ever patched by hand can carry a
-- different one, and `DROP CONSTRAINT <wrong name>` would abort the whole
-- migration.
--
-- The four cannabis kinds stay in the list. They are removed by deleting rows,
-- not by narrowing the constraint: `ADD CONSTRAINT` validates existing rows, so
-- dropping 'strain' here would abort on any live cart that still holds one.
DO $$
DECLARE
    kind_attnum SMALLINT;
    con RECORD;
BEGIN
    SELECT a.attnum INTO kind_attnum
    FROM pg_attribute a
    WHERE a.attrelid = 'cart_items'::regclass
      AND a.attname = 'kind'
      AND NOT a.attisdropped;

    FOR con IN
        SELECT c.conname
        FROM pg_constraint c
        WHERE c.conrelid = 'cart_items'::regclass
          AND c.contype = 'c'
          AND c.conkey = ARRAY[kind_attnum]
    LOOP
        EXECUTE format('ALTER TABLE cart_items DROP CONSTRAINT %I', con.conname);
    END LOOP;

    ALTER TABLE cart_items
        ADD CONSTRAINT cart_items_kind
        CHECK (kind IN ('strain', 'set', 'accessory', 'tea', 'bike_rental'));
END $$;

-- Widen the line-identity constraint to include the dates.
--
-- Same rule as above: the old constraint is located by its column set, not by
-- its name. On a schema built straight from 059 that resolves to
-- `cart_items_cart_id_kind_catalog_id_key`; the lookup also matches the new
-- five-column constraint, which makes this block safely re-runnable.
DO $$
DECLARE
    con RECORD;
BEGIN
    FOR con IN
        SELECT c.conname
        FROM pg_constraint c
        WHERE c.conrelid = 'cart_items'::regclass
          AND c.contype = 'u'
          AND (
              SELECT array_agg(a.attname::text ORDER BY a.attname)
              FROM pg_attribute a
              WHERE a.attrelid = c.conrelid
                AND a.attnum = ANY (c.conkey)
          ) IN (
              ARRAY['cart_id', 'catalog_id', 'kind'],
              ARRAY['cart_id', 'catalog_id', 'kind', 'rental_end', 'rental_start']
          )
    LOOP
        EXECUTE format('ALTER TABLE cart_items DROP CONSTRAINT %I', con.conname);
    END LOOP;

    ALTER TABLE cart_items
        ADD CONSTRAINT cart_items_cart_id_kind_catalog_id_rental_dates_key
        UNIQUE (cart_id, kind, catalog_id, rental_start, rental_end);
END $$;

-- The dates arrive as a pair, and in order.
--
-- A half-dated line (start set, end NULL) would fall through every rule below:
-- Postgres treats NULLs in a UNIQUE constraint as distinct, so such a row would
-- never conflict with itself and the cart would grow a duplicate on every
-- click. Existing rows all carry NULL/NULL and satisfy the first branch, so
-- this validates cleanly against a live cart.
--
-- It is deliberately NOT tied to `kind`: a rental line whose dates the customer
-- has not picked yet is still a legal line, and the index below treats it as
-- one line per family, which is the right answer while it has no dates.
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conname = 'cart_items_rental_dates'
          AND conrelid = 'cart_items'::regclass
    ) THEN
        ALTER TABLE cart_items
            ADD CONSTRAINT cart_items_rental_dates CHECK (
                (rental_start IS NULL AND rental_end IS NULL)
                OR (rental_start IS NOT NULL
                    AND rental_end IS NOT NULL
                    AND rental_end >= rental_start)
            );
    END IF;
END $$;

-- 059's invariant, preserved for undated lines.
--
-- Postgres compares NULLs as distinct inside a UNIQUE constraint, so the
-- widened constraint above no longer stops a second accessory line:
-- `(cart, 'accessory', id, NULL, NULL)` does not conflict with itself. Without
-- this index, widening the constraint would silently reintroduce the duplicate
-- cart lines 059 was written to prevent. `UNIQUE NULLS NOT DISTINCT` would say
-- the same thing in one clause but needs Postgres 15, and nothing in this repo
-- pins the server version, so the portable form is used.
--
-- A partial unique index cannot be inferred by a bare `ON CONFLICT (cart_id,
-- kind, catalog_id)`; the upsert has to repeat the predicate. See the note left
-- for `src/api/cart.rs` in the handover.
CREATE UNIQUE INDEX IF NOT EXISTS idx_cart_items_undated_line
    ON cart_items (cart_id, kind, catalog_id)
    WHERE rental_start IS NULL AND rental_end IS NULL;

-- Rental lines by date, for the availability check that has to know how many
-- units of a family are already promised over a range.
CREATE INDEX IF NOT EXISTS idx_cart_items_rental_dates
    ON cart_items (rental_start, rental_end)
    WHERE rental_start IS NOT NULL;
