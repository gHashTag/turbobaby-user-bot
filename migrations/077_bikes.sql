-- The catalog TurboBaby actually rents: a bike FAMILY, which is what a
-- customer chooses.
--
-- A family is a product, not a machine. `nmax-155` is one row here and ten rows
-- in `bike_units` (078). The customer books a model and the shop assigns the
-- machine at handover, which is why the cart line in 081 carries this table's
-- `key` and never a unit code.
--
-- Every money column is NULLABLE, and that is load-bearing rather than lax.
-- `data/fleet_seed.json` publishes no rentable tariff for `click-125` and no
-- monthly low-season price for `xadv-750`. `NOT NULL DEFAULT 0` here would
-- price both at zero, and a price of 0 reads to a customer as FREE — the exact
-- defect `migrations/008_strains_full_seed.sql:21` shipped for the strains
-- catalog. An absent number must reach the screen as a dash, so it is stored
-- as NULL and read as `Option<f64>` (DECISIONS.md D9).
--
-- `base_rate_thb_day` is the PRE class-discount published tariff (scooters
-- -25%, motorcycles -15%). Rendering it as the client price without applying
-- the discount overstates every scooter by 33% (D11).
CREATE TABLE IF NOT EXISTS bikes (
    id VARCHAR(36) PRIMARY KEY DEFAULT gen_random_uuid()::text,
    -- Stable catalog handle ('nmax-155', 'xmax-300-new'). This is what a cart
    -- line stores, so it must survive a rename of brand/model text.
    key TEXT NOT NULL UNIQUE,
    brand TEXT NOT NULL,
    model TEXT NOT NULL,
    -- 'NEW 2023+' / 'pre-2023'. NULL when the family has no sibling variant.
    -- `xmax-300` and `xmax-300-new` are two products, not two trims: different
    -- tariff, different deposit.
    variant_label TEXT,
    -- Drives the class discount in 079. Only these two exist: KB_faq files the
    -- 750cc X-ADV with the scooters, so displacement does not decide this.
    class TEXT NOT NULL CHECK (class IN ('scooter', 'motorcycle')),
    -- 'scooter', 'maxi-scooter', 'adventure-scooter', 'naked', 'sport',
    -- 'cruiser'. Left unconstrained: the shop adds body styles when it buys a
    -- bike, and a CHECK here would reject a real one at seed time.
    body TEXT NOT NULL,
    displacement_cc INT NOT NULL,
    -- The four money columns. NULL means "the source publishes nothing", which
    -- renders as a dash. The `> 0` checks make the confident-zero defect
    -- unwritable: an unknown price cannot be smuggled in as 0.0 by the clamp
    -- closure in src/db/strains.rs or by a `try_get_warn!` default.
    base_rate_thb_day DOUBLE PRECISION,
    deposit_thb DOUBLE PRECISION,
    monthly_low_season_thb DOUBLE PRECISION,
    -- Never derived from purchase cost — D14 bars per-unit cost, and any sale
    -- price derived from it, from this repository in any form.
    sale_price_thb DOUBLE PRECISION,
    -- FALSE closes a family to NEW rentals without deleting it: `click-125` is
    -- not offered (D12) yet still has one unit out on a contract that predates
    -- the decision, and that unit is not cancelled.
    offered BOOLEAN NOT NULL DEFAULT TRUE,
    description_ru TEXT,
    description_en TEXT,
    image_url TEXT,
    sort_order INT NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT bikes_base_rate_positive
        CHECK (base_rate_thb_day IS NULL OR base_rate_thb_day > 0),
    CONSTRAINT bikes_deposit_positive
        CHECK (deposit_thb IS NULL OR deposit_thb > 0),
    CONSTRAINT bikes_monthly_positive
        CHECK (monthly_low_season_thb IS NULL OR monthly_low_season_thb > 0),
    CONSTRAINT bikes_sale_price_positive
        CHECK (sale_price_thb IS NULL OR sale_price_thb > 0),
    CONSTRAINT bikes_displacement_positive CHECK (displacement_cc > 0)
);

-- The catalog list query: offered families in display order.
CREATE INDEX IF NOT EXISTS idx_bikes_offered_sort ON bikes(offered, sort_order);
-- Class filtering, and the join that applies the class discount.
CREATE INDEX IF NOT EXISTS idx_bikes_class ON bikes(class);
-- No `idx_bikes_key`: `key TEXT NOT NULL UNIQUE` already builds a unique index
-- over that column, and a second one would only cost writes. This comment
-- exists so the next person does not add one "just in case".
