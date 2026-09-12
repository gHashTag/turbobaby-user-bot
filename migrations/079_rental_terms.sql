-- The two published discount ladders a quote is built from.
--
-- The class discount is the whole reason three different numbers were
-- circulating for the same bikes. The tariff publishes 939/day for XMAX 300
-- NEW; the owner's live quote sheet says 704; 939 x 0.75 = 704.25. Both sheets
-- were right about different stages, and `data/fleet_seed.json` stores the
-- PRE-discount tariff. Keeping the rate in the database is what lets a renderer
-- close that gap instead of overstating every scooter by 33% (DECISIONS.md
-- D11). It does not make the bot a price oracle: the door still quotes the
-- client-facing number, and when the door is silent the bot emits none at all.
CREATE TABLE IF NOT EXISTS class_discounts (
    -- Mirrors the CHECK on `bikes.class` (077) so the two tables cannot
    -- disagree about which classes exist.
    class TEXT PRIMARY KEY CHECK (class IN ('scooter', 'motorcycle')),
    -- A fraction, not a percentage: 0.25, never 25.
    discount DOUBLE PRECISION NOT NULL
        CHECK (discount >= 0 AND discount < 1)
);

-- Published class discounts, quoted from `class_discounts` in the seed file.
INSERT INTO class_discounts (class, discount)
SELECT v.class, v.discount::DOUBLE PRECISION
FROM (
    VALUES
        ('scooter',    0.25),
        ('motorcycle', 0.15)
) AS v(class, discount)
-- Idempotent by class, and it never overwrites: if the owner changes a rate
-- through the admin surface, a re-run must not put the file's number back.
WHERE NOT EXISTS (
    SELECT 1 FROM class_discounts d WHERE d.class = v.class
);

-- Term discounts are published as BANDS, not multipliers.
--
-- KB_faq says "a week is 6 to 15% off", never a single figure, and the owner's
-- own sheet uses a step factor the file cannot reproduce. A band cannot be
-- collapsed into a price, so both edges are stored and neither the midpoint nor
-- the lower edge may be served as "the" discount — averaging these two would be
-- the same invention D9 forbids for an absent rate.
CREATE TABLE IF NOT EXISTS rental_terms (
    id VARCHAR(36) PRIMARY KEY,
    -- 'week' | 'two_weeks' | 'month'.
    band TEXT NOT NULL UNIQUE,
    min_days INT NOT NULL CHECK (min_days > 0),
    -- NULL = open-ended: the longest published band has no upper edge.
    max_days INT,
    discount_min DOUBLE PRECISION NOT NULL,
    discount_max DOUBLE PRECISION NOT NULL,
    CONSTRAINT rental_terms_days_ordered
        CHECK (max_days IS NULL OR max_days >= min_days),
    CONSTRAINT rental_terms_discount_ordered
        CHECK (discount_max >= discount_min),
    CONSTRAINT rental_terms_discount_is_a_fraction
        CHECK (discount_min >= 0 AND discount_max < 1)
);

-- The three published bands.
--
-- `discount_min` / `discount_max` are quoted verbatim from
-- `term_discount_bands` in the seed file.
--
-- `min_days` / `max_days` are not published as numbers anywhere — KB_faq names
-- the bands and gives only their discount ranges. The day edges follow from the
-- names (a week is 7 days, two weeks 14, a month 30) and from the bands being
-- contiguous, so each one ends the day before the next begins. That derivation
-- is written down here rather than left implicit, because it is the only figure
-- in this migration that is not a direct quote. The observed terms run 7, 30,
-- 90, 120, 150 and 180 days and no band above "month" is published, so the
-- last band is open-ended instead of being capped at a guess.
INSERT INTO rental_terms (id, band, min_days, max_days, discount_min, discount_max)
SELECT gen_random_uuid()::text,
       v.band,
       v.min_days,
       v.max_days,
       v.discount_min::DOUBLE PRECISION,
       v.discount_max::DOUBLE PRECISION
FROM (
    VALUES
        ('week',      7,  13,        0.06, 0.15),
        ('two_weeks', 14, 29,        0.15, 0.25),
        ('month',     30, NULL::INT, 0.35, 0.50)
) AS v(band, min_days, max_days, discount_min, discount_max)
WHERE NOT EXISTS (
    SELECT 1 FROM rental_terms t WHERE t.band = v.band
);

-- No indexes on either table: `class_discounts` holds two rows and
-- `rental_terms` three, both reached through their primary key or their unique
-- `band`. An index would be read overhead with nothing to skip.
