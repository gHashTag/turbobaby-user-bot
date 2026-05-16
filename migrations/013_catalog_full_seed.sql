-- Migration 013: Full catalog seed (accessories + accessory_sets prices + images)
-- Background: accessories table has rows but all prices are 0 and image_url empty.
-- API /api/sets reads from accessory_sets + tea_sets, so we seed prices there too.
-- Source of truth: old woody-woodpecker/scripts/seed-neon.cjs

-- ── 1. ACCESSORIES (price, image_url, descriptions) ───────────
INSERT INTO accessories (id, name, category, description, price, stock, image_url, is_available)
VALUES
  ('grinder-4p',     'Гриндер 4 части',     'grinder',  'Алюминиевый гриндер с сеткой',         500.0,  20, '/assets/accessories/grinder-4p.webp',     true),
  ('papers-raw',     'RAW Classic',         'papers',   'Бумажки для самокруток King Size',     100.0,  50, '/assets/accessories/papers-raw.webp',     true),
  ('papers-ocb',     'OCB Premium',         'papers',   'Тонкие бумажки',                       120.0,  40, '/assets/accessories/papers-ocb.webp',     true),
  ('lighter-djeep',  'Djeep Lighter',       'lighter',  'Надёжная зажигалка',                   150.0,  30, '/assets/accessories/lighter-djeep.webp',  true),
  ('lighter-clipper','Clipper Lighter',     'lighter',  'Классическая зажигалка с кремнём',     120.0,  30, '/assets/accessories/lighter-clipper.webp', true),
  ('tray-metal',     'Металлический поднос','tray',     'Прочный металлический поднос',         600.0,  15, '/assets/accessories/tray-metal.webp',     true),
  ('storage-jar',    'Банка для хранения',  'storage',  'Герметичная банка',                    250.0,  15, '/assets/accessories/storage-jar.webp',    true),
  ('bong-mini',      'Мини бонг',           'bong',     'Стеклянный бонг 20см',                 1500.0,  5, '/assets/accessories/bong-mini.webp',      true),
  ('pipe-glass',     'Стеклянная трубка',   'pipe',     'Компактная стеклянная трубка',         450.0,  10, '/assets/accessories/pipe-glass.webp',     true),
  ('tshirt-woody',   'Футболка Woody',      'clothing', 'Футболка с логотипом',                 890.0,  20, '/assets/accessories/tshirt-woody.webp',   true)
ON CONFLICT (id) DO UPDATE SET
  name        = EXCLUDED.name,
  category    = EXCLUDED.category,
  description = EXCLUDED.description,
  -- ALWAYS update price/image when previous value is empty/zero, to fix bad seeds:
  price       = CASE WHEN accessories.price = 0 OR accessories.price IS NULL
                     THEN EXCLUDED.price ELSE accessories.price END,
  stock       = GREATEST(COALESCE(accessories.stock, 0), EXCLUDED.stock),
  image_url   = CASE WHEN accessories.image_url IS NULL OR accessories.image_url = ''
                     THEN EXCLUDED.image_url ELSE accessories.image_url END,
  is_available = EXCLUDED.is_available;

-- ── 2. ACCESSORY SETS ─────────────────────────────────────────
-- API /api/sets reads from accessory_sets (column: accessories TEXT[])
INSERT INTO accessory_sets (id, name, description, icon, accessories, total_price, discount_percent, is_available, is_deal_of_day)
VALUES
  (
    'starter-kit',
    'STARTER KIT',
    'Всё для начинающего: гриндер + бумажки + зажигалка',
    '🎁',
    ARRAY['grinder-4p','papers-raw','lighter-clipper'],
    720.0, 0.0, true, false
  ),
  (
    'pro-kit',
    'PRO KIT',
    'Для опытных: гриндер + трубка + банка',
    '⭐',
    ARRAY['grinder-4p','pipe-glass','storage-jar'],
    1200.0, 0.0, true, false
  )
ON CONFLICT (id) DO UPDATE SET
  name             = EXCLUDED.name,
  description      = EXCLUDED.description,
  icon             = EXCLUDED.icon,
  accessories      = EXCLUDED.accessories,
  total_price      = CASE WHEN accessory_sets.total_price = 0 OR accessory_sets.total_price IS NULL
                          THEN EXCLUDED.total_price ELSE accessory_sets.total_price END,
  discount_percent = EXCLUDED.discount_percent,
  is_available     = EXCLUDED.is_available;
