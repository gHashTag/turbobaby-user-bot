-- The fleet, as `data/fleet_seed.json` records it: 14 families, 37 units.
--
-- Every figure below is quoted from that file, which is itself quoted from
-- named sources. Nothing here is computed, averaged or interpolated.
--
-- What the seed deliberately does NOT contain:
--
--   * The 7 `price_list_only` families (PCX 150, ADV 150, PCX 160, ADV 160,
--     Rebel 300, XSR 900, R7). The tariff publishes a price for each and the
--     fleet holds ZERO units of any of them. Seeding them into `bikes` would
--     put a product in the catalog that does not exist, and a customer would
--     be quoted a bike nobody can hand over. They stay out of the database
--     entirely until a unit is bought.
--
--   * `sale_price_thb`. No sale price is published for any family, and
--     DECISIONS.md D14 bars deriving one from the per-unit purchase cost the
--     internal register carries. The column is left absent rather than filled.
--
--   * `description_ru` / `description_en` / `image_url`. The seed publishes no
--     description or image. Writing marketing copy here would be inventing
--     catalog content in a data migration.
--
--   * `km_since_purchase` on every unit. The register's mileage column is not
--     de-identified into the seed file, and the one figure it does quote is
--     tied to an internal register row this repository has no key for.
--
-- The two rows the money-honesty rule exists for:
--
--   * `click-125` — offered FALSE (D12), and `base_rate_thb_day`,
--     `deposit_thb`, `monthly_low_season_thb` all NULL. No rentable tariff is
--     published for it. The owner's live grid still prices it at 187/day; that
--     row is stale, not a tariff, and it is not copied here. Its one unit stays
--     in `bike_units` as `rented`, because the contract that predates the
--     decision is not cancelled.
--
--   * `xadv-750` — `monthly_low_season_thb` NULL. KB_faq publishes a daily rate
--     and a deposit for it but no monthly low-season price, so that one field
--     must render as a dash while the other two render numbers.
--
-- Neither NULL is filled with 0, with a class average, or with the daily rate
-- times thirty.

-- ---------------------------------------------------------------------------
-- Families
-- ---------------------------------------------------------------------------
--
-- `base_rate_thb_day` is the PRE class-discount published tariff (D11).
-- `sort_order` follows the seed file's own order, which runs by displacement.
INSERT INTO bikes (
    key, brand, model, variant_label, class, body, displacement_cc,
    base_rate_thb_day, deposit_thb, monthly_low_season_thb,
    offered, sort_order
)
SELECT v.key,
       v.brand,
       v.model,
       v.variant_label,
       v.class,
       v.body,
       v.displacement_cc,
       -- Cast explicitly: migrations 012/015/020 exist because money columns in
       -- this schema drifted to NUMERIC, and an untyped literal invites it back.
       v.base_rate_thb_day::DOUBLE PRECISION,
       v.deposit_thb::DOUBLE PRECISION,
       v.monthly_low_season_thb::DOUBLE PRECISION,
       v.offered,
       v.sort_order
FROM (
    VALUES
        -- key, brand, model, variant_label, class, body, cc,
        --   rate, deposit, monthly, offered, sort
        ('click-125', 'Honda', 'Click 125', NULL, 'scooter', 'scooter', 125,
            NULL, NULL, NULL, FALSE, 1),
        ('nmax-155', 'Yamaha', 'NMAX 155', NULL, 'scooter', 'scooter', 155,
            449.0, 3000.0, 5000.0, TRUE, 2),
        ('xsr-155', 'Yamaha', 'XSR 155', NULL, 'motorcycle', 'naked', 155,
            590.0, 7000.0, 6490.0, TRUE, 3),
        ('cb-300r', 'Honda', 'CB300R', NULL, 'motorcycle', 'naked', 300,
            890.0, 15000.0, 9900.0, TRUE, 4),
        ('forza-300', 'Honda', 'Forza 300', NULL, 'scooter', 'maxi-scooter', 300,
            690.0, 5000.0, 7900.0, TRUE, 5),
        ('mt-03-300', 'Yamaha', 'MT-03', NULL, 'motorcycle', 'naked', 300,
            1090.0, 15000.0, 10990.0, TRUE, 6),
        -- Two products, not two trims of one: different tariff, different
        -- deposit. The register's own NEW marker draws the line, which agrees
        -- with year >= 2023, so the label says pre-2023 rather than repeating
        -- KB_faq's "2020-2022" band that two of the three units do not fit.
        ('xmax-300', 'Yamaha', 'XMAX 300', 'pre-2023', 'scooter', 'maxi-scooter', 300,
            790.0, 5000.0, 8900.0, TRUE, 7),
        ('xmax-300-new', 'Yamaha', 'XMAX 300', 'NEW 2023+', 'scooter', 'maxi-scooter', 300,
            939.0, 7000.0, 9900.0, TRUE, 8),
        ('adv-350', 'Honda', 'ADV 350', NULL, 'scooter', 'adventure-scooter', 350,
            998.0, 7000.0, 10900.0, TRUE, 9),
        ('ninja-400', 'Kawasaki', 'Ninja 400', NULL, 'motorcycle', 'sport', 400,
            1185.0, 20000.0, 11900.0, TRUE, 10),
        ('cb-650r', 'Honda', 'CB650R', NULL, 'motorcycle', 'naked', 650,
            1798.0, 20000.0, 19900.0, TRUE, 11),
        ('cbr-650r', 'Honda', 'CBR650R', NULL, 'motorcycle', 'sport', 650,
            1798.0, 20000.0, 19900.0, TRUE, 12),
        ('vulcan-s-650', 'Kawasaki', 'Vulcan S 650', NULL, 'motorcycle', 'cruiser', 650,
            1798.0, 20000.0, 19900.0, TRUE, 13),
        -- KB_faq files this with the scooters (25% class discount) despite the
        -- 750cc engine, so `class` is 'scooter' here on purpose.
        ('xadv-750', 'Honda', 'X-ADV 750', NULL, 'scooter', 'adventure-scooter', 750,
            2788.0, 25000.0, NULL, TRUE, 14)
) AS v(key, brand, model, variant_label, class, body, displacement_cc,
       base_rate_thb_day, deposit_thb, monthly_low_season_thb,
       offered, sort_order)
-- Idempotent by key, and it never overwrites. An admin who has hidden a family
-- or corrected a price through the admin surface must not find the file's
-- version back in place after a redeploy — the defect
-- `migrations/008_strains_full_seed.sql` documents at length.
WHERE NOT EXISTS (
    SELECT 1 FROM bikes b WHERE b.key = v.key
);

-- ---------------------------------------------------------------------------
-- Units
-- ---------------------------------------------------------------------------
--
-- 37 rows: 11 `rented`, 26 `available`. The internal register's header claims
-- 38 units but only 37 rows exist; the seed file records that discrepancy and
-- resolves it by seeding the 37 that are there, and no 38th unit is invented
-- here to satisfy a header.
--
-- `unit_code` is generated as '<family-key>-NN', zero-padded from 01. It is a
-- slot label, never a plate number (D14) and never the register's own row id.
--
-- How years and colours are distributed, and where they stop:
--
--   * The register lists which years and which colours a family holds, not
--     which machine is which. Values are therefore assigned to slots in the
--     order the seed file lists them.
--
--   * Where a family lists fewer colours than it has units, the remaining
--     slots get NULL. Repeating a colour to fill them would assert a fact the
--     register does not carry — `nmax-155` has 10 units and 8 colours, so two
--     units have no colour recorded, and `click-125`, `cb-300r`, `ninja-400`,
--     `cbr-650r` and `vulcan-s-650` list none at all.
--
--   * Years follow the same rule with one exception that is evidence, not
--     convenience. A family listing exactly ONE year has that year on every
--     unit: the year list is the set of years present in the family, so a
--     single entry determines all of them. A family listing several gets them
--     assigned in order and NULL for the remainder — `nmax-155` holds 2020 and
--     2021 across 10 units and nothing says which, so 8 slots have no year.
--
--   * `xmax-300` is fully determined despite listing two years: the seed file
--     records that two of its three units are 2019.
--
-- Which slots are `rented` is likewise arbitrary within a family — the seed
-- gives counts, not identities — so the first N slots carry it. The counts are
-- what the availability figures are computed from, and they match the register
-- family by family.
INSERT INTO bike_units (bike_id, unit_code, model_year, color, status)
SELECT b.id, v.unit_code, v.model_year, v.color, v.status
FROM (
    VALUES
        -- click-125: 1 unit, 1 rented. No colours recorded.
        ('click-125', 'click-125-01', 2017, NULL, 'rented'),

        -- nmax-155: 10 units, 5 rented. 2 years and 8 colours for 10 slots.
        ('nmax-155', 'nmax-155-01', 2020, 'black gold',  'rented'),
        ('nmax-155', 'nmax-155-02', 2021, 'black green', 'rented'),
        ('nmax-155', 'nmax-155-03', NULL, 'black',       'rented'),
        ('nmax-155', 'nmax-155-04', NULL, 'green',       'rented'),
        ('nmax-155', 'nmax-155-05', NULL, 'grey gold',   'rented'),
        ('nmax-155', 'nmax-155-06', NULL, 'grey',        'available'),
        ('nmax-155', 'nmax-155-07', NULL, 'race',        'available'),
        ('nmax-155', 'nmax-155-08', NULL, 'red white',   'available'),
        ('nmax-155', 'nmax-155-09', NULL, NULL,          'available'),
        ('nmax-155', 'nmax-155-10', NULL, NULL,          'available'),

        -- xsr-155: 2 units, 1 rented. One year, so both units carry it.
        ('xsr-155', 'xsr-155-01', 2021, 'black', 'rented'),
        ('xsr-155', 'xsr-155-02', 2021, 'green', 'available'),

        -- cb-300r: 1 unit, none rented. No colours recorded.
        ('cb-300r', 'cb-300r-01', 2018, NULL, 'available'),

        -- forza-300: 1 unit, 1 rented.
        ('forza-300', 'forza-300-01', 2019, 'white', 'rented'),

        -- mt-03-300: 1 unit, 1 rented.
        ('mt-03-300', 'mt-03-300-01', 2021, 'blue', 'rented'),

        -- xmax-300: 3 units, none rented. Two are 2019, one is 2020.
        ('xmax-300', 'xmax-300-01', 2019, 'blue',  'available'),
        ('xmax-300', 'xmax-300-02', 2019, 'green', 'available'),
        ('xmax-300', 'xmax-300-03', 2020, 'grey',  'available'),

        -- xmax-300-new: 7 units, 1 rented. One year, so all seven carry it;
        -- 4 colours for 7 slots.
        ('xmax-300-new', 'xmax-300-new-01', 2023, 'red gloss', 'rented'),
        ('xmax-300-new', 'xmax-300-new-02', 2023, 'black',     'available'),
        ('xmax-300-new', 'xmax-300-new-03', 2023, 'blue',      'available'),
        ('xmax-300-new', 'xmax-300-new-04', 2023, 'brown',     'available'),
        ('xmax-300-new', 'xmax-300-new-05', 2023, NULL,        'available'),
        ('xmax-300-new', 'xmax-300-new-06', 2023, NULL,        'available'),
        ('xmax-300-new', 'xmax-300-new-07', 2023, NULL,        'available'),

        -- adv-350: 5 units, none rented. 4 years and 5 colours for 5 slots.
        ('adv-350', 'adv-350-01', 2022, 'grey',       'available'),
        ('adv-350', 'adv-350-02', 2023, 'dark grey',  'available'),
        ('adv-350', 'adv-350-03', 2024, 'red',        'available'),
        ('adv-350', 'adv-350-04', 2025, 'light grey', 'available'),
        ('adv-350', 'adv-350-05', NULL, 'black gold', 'available'),

        -- ninja-400: 1 unit, none rented. No colours recorded.
        ('ninja-400', 'ninja-400-01', 2020, NULL, 'available'),

        -- cb-650r: 1 unit, none rented.
        ('cb-650r', 'cb-650r-01', 2022, 'black', 'available'),

        -- cbr-650r: 1 unit, none rented. No colours recorded.
        ('cbr-650r', 'cbr-650r-01', 2018, NULL, 'available'),

        -- vulcan-s-650: 1 unit, none rented. No colours recorded.
        ('vulcan-s-650', 'vulcan-s-650-01', 2019, NULL, 'available'),

        -- xadv-750: 2 units, 1 rented. One year, so both units carry it.
        ('xadv-750', 'xadv-750-01', 2022, 'black', 'rented'),
        ('xadv-750', 'xadv-750-02', 2022, 'grey',  'available')
) AS v(bike_key, unit_code, model_year, color, status)
-- An inner join, so a unit whose family is absent is silently skipped rather
-- than failing the migration. That is the desired behaviour for exactly one
-- case: an admin who deleted a family before this seed ran should not have it
-- resurrected through its units.
JOIN bikes b ON b.key = v.bike_key
WHERE NOT EXISTS (
    SELECT 1 FROM bike_units u WHERE u.unit_code = v.unit_code
);
