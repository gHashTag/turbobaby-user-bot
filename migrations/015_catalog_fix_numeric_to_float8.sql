-- Migration 015: Convert NUMERIC columns in catalog tables to DOUBLE PRECISION.
-- Same root cause as migration 012 (strains) — production DB was created
-- manually with NUMERIC for price/total_price, so tokio-postgres try_get::<f64>
-- silently fails and the API returns 0.0. Migrations 013/014 set values but
-- the type mismatch made them invisible to the API.

DISCARD PLANS;

-- ── accessories.price ─────────────────────────────────────────
DO $$
BEGIN
  IF (SELECT data_type FROM information_schema.columns
      WHERE table_name='accessories' AND column_name='price') <> 'double precision' THEN
    ALTER TABLE accessories ALTER COLUMN price TYPE DOUBLE PRECISION
      USING price::double precision;
  END IF;
END $$;

-- ── accessory_sets.total_price + discount_percent ─────────────
DO $$
BEGIN
  IF (SELECT data_type FROM information_schema.columns
      WHERE table_name='accessory_sets' AND column_name='total_price') <> 'double precision' THEN
    ALTER TABLE accessory_sets ALTER COLUMN total_price TYPE DOUBLE PRECISION
      USING total_price::double precision;
  END IF;
END $$;

DO $$
BEGIN
  IF (SELECT data_type FROM information_schema.columns
      WHERE table_name='accessory_sets' AND column_name='discount_percent') <> 'double precision' THEN
    ALTER TABLE accessory_sets ALTER COLUMN discount_percent TYPE DOUBLE PRECISION
      USING discount_percent::double precision;
  END IF;
END $$;

-- ── tea_products.price ────────────────────────────────────────
DO $$
BEGIN
  IF (SELECT data_type FROM information_schema.columns
      WHERE table_name='tea_products' AND column_name='price') <> 'double precision' THEN
    ALTER TABLE tea_products ALTER COLUMN price TYPE DOUBLE PRECISION
      USING price::double precision;
  END IF;
END $$;

-- ── tea_sets.total_price + discount_percent ───────────────────
DO $$
BEGIN
  IF (SELECT data_type FROM information_schema.columns
      WHERE table_name='tea_sets' AND column_name='total_price') <> 'double precision' THEN
    ALTER TABLE tea_sets ALTER COLUMN total_price TYPE DOUBLE PRECISION
      USING total_price::double precision;
  END IF;
END $$;

DO $$
BEGIN
  IF (SELECT data_type FROM information_schema.columns
      WHERE table_name='tea_sets' AND column_name='discount_percent') <> 'double precision' THEN
    ALTER TABLE tea_sets ALTER COLUMN discount_percent TYPE DOUBLE PRECISION
      USING discount_percent::double precision;
  END IF;
END $$;

-- After type cast — re-apply prices (in case 013/014 silently failed earlier)
UPDATE accessories SET price = v.price, image_url = v.image_url
FROM (VALUES
  ('grinder-4p',       500.0::double precision, '/assets/accessories/grinder-4p.webp'),
  ('papers-raw',       100.0,                   '/assets/accessories/papers-raw.webp'),
  ('papers-ocb',       120.0,                   '/assets/accessories/papers-ocb.webp'),
  ('lighter-djeep',    150.0,                   '/assets/accessories/lighter-djeep.webp'),
  ('lighter-clipper',  120.0,                   '/assets/accessories/lighter-clipper.webp'),
  ('tray-metal',       600.0,                   '/assets/accessories/tray-metal.webp'),
  ('storage-jar',      250.0,                   '/assets/accessories/storage-jar.webp'),
  ('bong-mini',       1500.0,                   '/assets/accessories/bong-mini.webp'),
  ('pipe-glass',       450.0,                   '/assets/accessories/pipe-glass.webp'),
  ('tshirt-woody',     890.0,                   '/assets/accessories/tshirt-woody.webp')
) AS v(id, price, image_url)
WHERE accessories.id = v.id;

UPDATE accessory_sets SET total_price = v.total_price
FROM (VALUES
  ('starter-kit',  720.0::double precision),
  ('pro-kit',     1200.0)
) AS v(id, total_price)
WHERE accessory_sets.id = v.id;
