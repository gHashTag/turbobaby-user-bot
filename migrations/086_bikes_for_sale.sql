-- `bikes.for_sale` — the flag the sale surface has been asking for since it
-- was written, and that nothing has ever been able to answer.
--
-- Issue #11 asks for a bot adapted to bike SALES, not rental alone. The
-- customer half was built: `ui/screens/bike_detail.rs` renders a sale block,
-- and it decides whether to render from
--
--     bike.for_sale == Some(true) || sale_price.is_some()
--
-- with `for_sale: Option<bool>` declared on the UI's own `Bike` struct
-- (`catalog_screen.rs:136`). The struct field is real; the column was not.
-- `/api/bikes` has never served a `for_sale` key, so serde filled it with
-- `None` on every response and the left half of that `||` has been dead since
-- the day it was written — a comment three lines above it in
-- `catalog_screen.rs` says so outright ("`api::bikes` serves no `for_sale`
-- field today") and the comparison was left in place anyway.
--
-- The right half is dead too, in practice rather than by construction: all 14
-- families in `data/fleet_seed.json` carry `sale_price_thb: null`, because a
-- sale price the shop has not published is not a number this repository is
-- allowed to guess at (D11, and D14 bars deriving one from the internal
-- purchase cost). So the whole condition is false for every bike in the
-- fleet, and the sales feature the issue was opened for is unreachable on
-- every screen.
--
-- This migration supplies the missing half. It does NOT supply data: every
-- row defaults to FALSE, which is exactly the fleet's state today — nothing
-- is on the forecourt until the owner says it is. What changes is that
-- saying so becomes possible, from the admin screen, without also having to
-- publish an asking price. That separation is the point of a flag: `for_sale`
-- and `sale_price_thb` answer different questions, and D9 requires that "we
-- sell this one, ask us the price" stay expressible. Collapsing them would
-- force the shop to invent a number to advertise a bike.
--
-- Forward-only, per D2: 001-076 are never edited, and 077's CREATE TABLE is
-- not reopened to add a column six migrations later.

ALTER TABLE bikes
    ADD COLUMN IF NOT EXISTS for_sale BOOLEAN NOT NULL DEFAULT FALSE;

-- Read by the catalog list and the detail screen, both of which filter on it,
-- and by the admin fleet table. Partial because the selective direction is
-- the interesting one: a shop lists a handful of bikes for sale out of a
-- fleet it rents.
CREATE INDEX IF NOT EXISTS idx_bikes_for_sale
    ON bikes (for_sale)
    WHERE for_sale;

COMMENT ON COLUMN bikes.for_sale IS
    'Family is offered for sale. Independent of sale_price_thb: TRUE with a '
    'NULL price is the honest "price on request" state (D9/D11). Never set '
    'from a purchase cost (D14).';
