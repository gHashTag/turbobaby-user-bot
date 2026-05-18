-- Migration 017: Restore original prices and discounts from the old woody-woodpecker bot.
--
-- Source: gHashTag/woody-woodpecker · bot/src/db.ts → seedDefaultData() (commit 426d038^).
-- Migration 014 overwrote 4 accessory prices and zeroed the set discounts;
-- here we put them back to the values the original Telegram bot shipped with.
--
-- Strategy chosen by operator: "Old prices, but don't delete new items".
-- lighter-djeep and tray-metal stay in the catalog (they were added later).

-- ── Accessories: restore 4 prices ──────────────────────────────
UPDATE accessories SET price = v.price
FROM (VALUES
  ('lighter-clipper', 150.0::double precision),  -- was 120฿, original 150฿
  ('pipe-glass',      800.0),                    -- was 450฿, original 800฿
  ('storage-jar',     350.0),                    -- was 250฿, original 350฿
  ('tshirt-woody',    800.0)                     -- was 890฿, original 800฿
) AS v(id, price)
WHERE accessories.id = v.id;

-- ── Accessory sets: restore original discounted prices & percentages ──
UPDATE accessory_sets SET total_price = v.total_price, discount_percent = v.discount_percent
FROM (VALUES
  ('starter-kit',  750.0::double precision, 10.0::double precision),  -- 990→750 with 10% off
  ('pro-kit',     1650.0,                   15.0)                     -- 1950→1650 with 15% off
) AS v(id, total_price, discount_percent)
WHERE accessory_sets.id = v.id;
