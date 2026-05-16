-- Migration 014: Force-update accessory & accessory_set prices.
-- Migration 013 used a CASE-guarded UPDATE that did not catch existing zero-priced
-- rows for some reason (probably created_at type subtlety or already-applied state).
-- This one unconditionally overwrites prices to the correct values from the old seed.

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
