-- 087: TurboBaby's own delivery zones, on Phuket, replace the Koh Phangan map.
--
-- Owner's decisions, 2026-09-24 (chat). Until today the picker offered the
-- other shop's villages -- 071 seeded Thong Sala, Haad Rin, Srithanu and
-- Bottle Beach, and 085 kept them "until the owner defines TurboBaby's Phuket
-- zones" (DECISIONS.md D19). The owner has decided:
--
--   1. The zone table is the owner's live Bridge sheet (read-only snapshot
--      D:/turbobaby-bot/fixtures/delivery_zones_get.live.json, taken
--      2026-07-15, rows [name, lat, lon, price, radius]) -- all 16 of its
--      zones at the sheet's own prices -- plus the two districts the brain's
--      delivery rule names and the sheet lacks: Kathu 490 and Nai Harn 590
--      (brain business_rules, "ПОДТВЕРДИЛОСЬ ЖИВОЙ ПРАКТИКОЙ (сверка 20.08)").
--      Pa Khlok is 490 by owner decision 06.09.2026-3, Mai Khao 990 by owner
--      decision 06.09.2026-4, and the airport 690 is the owner's choice today
--      from the rule's 590-690 range. 18 zones.
--   2. No delivery time in minutes: none is published, the shop promises a
--      window. eta_min / eta_max become nullable with no default, and every
--      row -- the old island's, the pickup row, the placeholder from 060 --
--      is set to NULL. An absent ETA is served as null and shown as nothing.
--   3. The Koh Phangan rows are DEACTIVATED, not deleted: placed orders
--      reference delivery_zone_id, and a deleted row would orphan them (the
--      same reason 071 gave for the Pattaya placeholder). The shop's own
--      pickup row (renamed by 085; src/delivery.rs places the shop in Kamala)
--      stays active at fee 0 and loses only its invented ETA.
--   4. The sheet's out-of-belt settings (OUT_BELT_KM 5, OUT_BELT_PRICE 1490,
--      OUT_BEYOND "by agreement") and the 17:30 delivery cut-off are NOT
--      modelled here; specs/turbobaby/delivery_terms.t27 records both as
--      known, unmodelled rules with their sources.
--
-- SORT ORDER. The pickup row keeps 0 from 071, so it stays the first option
-- and the checkout's default. The 18 places take 1-18 in the order the shop
-- itself lists its tariff: by ladder step, cheapest first (290, 390, 490,
-- 590, 690, 990); within a step, in the live sheet's own row order; the two
-- districts only the brain names close their step. No distance, no
-- popularity and no geography nobody measured decides a position.
--
-- min_order is 0 on every row: nothing publishes a minimum order for
-- delivery, and 0 is this column's "no minimum", not a price.
--
-- Forward-only, per D2: 060 and 071 are applied history and stay unedited.

ALTER TABLE delivery_zones ALTER COLUMN eta_min DROP NOT NULL;
ALTER TABLE delivery_zones ALTER COLUMN eta_min DROP DEFAULT;
ALTER TABLE delivery_zones ALTER COLUMN eta_max DROP NOT NULL;
ALTER TABLE delivery_zones ALTER COLUMN eta_max DROP DEFAULT;

-- Every row, active or not: an ETA nobody measured is not history worth
-- keeping, and a reactivated row must not bring one back.
UPDATE delivery_zones
   SET eta_min = NULL,
       eta_max = NULL,
       updated_at = now()
 WHERE eta_min IS NOT NULL OR eta_max IS NOT NULL;

-- The other island's four villages, by either name so a row an admin renamed
-- in one language is still caught.
UPDATE delivery_zones
   SET is_active = FALSE,
       updated_at = now()
 WHERE name_en IN ('Thong Sala', 'Haad Rin', 'Srithanu', 'Bottle Beach')
    OR name IN ('Тонгсала', 'Хаад Рин', 'Сритхану', 'Боттл Бич');

-- The Phuket zones. Idempotent by name_en, like 071: re-running must not
-- duplicate a zone, and a zone an admin already created by hand is left as
-- the admin made it rather than recreated underneath them.
INSERT INTO delivery_zones (name, name_en, fee, min_order, sort_order)
SELECT v.name, v.name_en, v.fee, v.min_order, v.sort_order FROM (
    VALUES
        ('Банг Тао',      'Bang Tao',      290.0, 0.0,  1),
        ('Сурин',         'Surin',         290.0, 0.0,  2),
        ('Камала',        'Kamala',        290.0, 0.0,  3),
        ('Патонг',        'Patong',        290.0, 0.0,  4),
        ('Таланг север',  'Thalang North', 390.0, 0.0,  5),
        ('Карон',         'Karon',         390.0, 0.0,  6),
        ('Пхукет-таун',   'Phuket Town',   490.0, 0.0,  7),
        ('Паклок',        'Pa Khlok',      490.0, 0.0,  8),
        ('Таланг восток', 'Thalang East',  490.0, 0.0,  9),
        ('Ката',          'Kata',          490.0, 0.0, 10),
        ('Кату',          'Kathu',         490.0, 0.0, 11),
        ('Раваи',         'Rawai',         590.0, 0.0, 12),
        ('Чалонг',        'Chalong',       590.0, 0.0, 13),
        ('Кейп Панва',    'Cape Panwa',    590.0, 0.0, 14),
        ('Найтон',        'Nai Thon',      590.0, 0.0, 15),
        ('Найхарн',       'Nai Harn',      590.0, 0.0, 16),
        ('Аэропорт',      'Airport',       690.0, 0.0, 17),
        ('Майкхао',       'Mai Khao',      990.0, 0.0, 18)
) AS v(name, name_en, fee, min_order, sort_order)
WHERE NOT EXISTS (
    SELECT 1 FROM delivery_zones z WHERE z.name_en = v.name_en
);
