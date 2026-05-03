-- Migration 008: Full strain catalog seed
-- Adds all detail fields for 12 strains: thc/cbd/effect/flavor/description/price/grams

-- Ensure unique constraint on name for upsert
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conname = 'strains_name_unique' AND conrelid = 'strains'::regclass
    ) THEN
        ALTER TABLE strains ADD CONSTRAINT strains_name_unique UNIQUE (name);
    END IF;
END$$;

-- Ensure all detail columns exist (idempotent)
ALTER TABLE strains ADD COLUMN IF NOT EXISTS thc_percent     DOUBLE PRECISION;
ALTER TABLE strains ADD COLUMN IF NOT EXISTS cbd_percent     DOUBLE PRECISION;
ALTER TABLE strains ADD COLUMN IF NOT EXISTS effect          TEXT;
ALTER TABLE strains ADD COLUMN IF NOT EXISTS flavor_profile  TEXT;
ALTER TABLE strains ADD COLUMN IF NOT EXISTS description     TEXT;
ALTER TABLE strains ADD COLUMN IF NOT EXISTS price_per_gram  DOUBLE PRECISION NOT NULL DEFAULT 0;
ALTER TABLE strains ADD COLUMN IF NOT EXISTS available_grams DOUBLE PRECISION;

-- Upsert all 12 strains with full detail data
INSERT INTO strains (name, category, thc_percent, cbd_percent, effect, flavor_profile, description, price_per_gram, available_grams, image_url, is_available)
VALUES
  (
    'Super Runtz',
    'hybrid',
    24.0,
    0.1,
    'Euphoric, happy, relaxed',
    'Limonene, Caryophyllene, Linalool',
    'Hybrid strain with euphoric and happy effects. Perfect for daytime relaxation.',
    450.0,
    50.0,
    '/assets/Super-Runtz.webp',
    true
  ),
  (
    'COLT 45',
    'indica',
    25.0,
    0.1,
    'Deep relaxation, heavy body, calm mind',
    'Myrcene, Caryophyllene, Humulene',
    'Indica-Dominant Hybrid. Deep relaxation and heavy body high. For experienced users.',
    450.0,
    40.0,
    '/assets/Colt-45.webp',
    true
  ),
  (
    'MIAMI VICE',
    'hybrid',
    23.0,
    0.2,
    'Balanced, uplifting, smooth high',
    'Limonene, Myrcene, Caryophyllene',
    'Balanced hybrid with uplifting effects. Great for day or afternoon use.',
    450.0,
    45.0,
    '/assets/Miami-Vice.webp',
    true
  ),
  (
    'MILK MONKEY',
    'indica',
    26.0,
    0.1,
    'Sedative, body-heavy, calming',
    'Myrcene, Linalool, Caryophyllene',
    'Pure Indica with sedative effects. Heavy body high, perfect for night time.',
    500.0,
    30.0,
    '/assets/Milk-Monkey.webp',
    true
  ),
  (
    'Black Mamba',
    'indica',
    27.0,
    0.1,
    'Very strong body stone, deep relaxation',
    'Myrcene, Caryophyllene, Limonene',
    'Very strong Indica. Deep body stone and relaxation. For heavy smokers only.',
    500.0,
    25.0,
    '/assets/Black-Mamba.webp',
    true
  ),
  (
    'SUPER BOOF',
    'hybrid',
    24.0,
    0.2,
    'Energetic, euphoric, creative',
    'Limonene, Caryophyllene, Pinene',
    'Energetic hybrid with euphoric and creative effects. Great for daytime.',
    350.0,
    40.0,
    '/assets/Super-Boof.webp',
    true
  ),
  (
    'SUPER LEMON HAZE',
    'sativa',
    22.0,
    0.2,
    'Energizing, focused, uplifting',
    'Limonene, Terpinolene, Pinene',
    'Classic Sativa with energizing effects. Perfect for morning or daytime use.',
    350.0,
    60.0,
    '/assets/Super-Lemon-Haze.webp',
    true
  ),
  (
    'DIPZ',
    'sativa',
    25.0,
    0.1,
    'Relaxing, smooth body high',
    'Myrcene, Caryophyllene, Linalool',
    'Sativa-Dominant Hybrid with relaxing effects. Good for evening use.',
    350.0,
    35.0,
    '/assets/Dipz.webp',
    true
  ),
  (
    'BANANA FRITTER',
    'sativa',
    26.0,
    0.1,
    'Comforting, relaxing, anxiety relief',
    'Myrcene, Caryophyllene, Limonene',
    'Sativa-Dominant Hybrid. Comforting and relaxing with anxiety relief.',
    350.0,
    30.0,
    '/assets/Banana-fritter.webp',
    true
  ),
  (
    'L.A ULTRA',
    'hybrid',
    24.0,
    0.2,
    'Balanced euphoria, body relaxation',
    'Caryophyllene, Limonene, Myrcene',
    'Balanced Hybrid with euphoria and body relaxation. Good for day or evening.',
    400.0,
    45.0,
    '/assets/LA-Ultra.webp',
    true
  ),
  (
    'Joker Candy',
    'sativa',
    25.0,
    0.1,
    'Energizing, euphoric, creative, boosts mood',
    'Myrcene, Bisabolol, Caryophyllene',
    'Sativa with energizing and euphoric effects. Clear head, light and happy high. Good for experienced users.',
    400.0,
    40.0,
    '/assets/Joker-candy.jpeg',
    true
  ),
  (
    'MAC 1',
    'hybrid',
    24.0,
    0.1,
    'Energizing, euphoric, strong head & body high',
    'Caryophyllene, Limonene, Pinene',
    'Hybrid with balanced, powerful, long-lasting effects. Strong head & body high. Not for beginners.',
    400.0,
    40.0,
    '/assets/Mac-1.jpeg',
    true
  )
ON CONFLICT (name) DO UPDATE SET
    category        = EXCLUDED.category,
    thc_percent     = EXCLUDED.thc_percent,
    cbd_percent     = EXCLUDED.cbd_percent,
    effect          = EXCLUDED.effect,
    flavor_profile  = EXCLUDED.flavor_profile,
    description     = EXCLUDED.description,
    price_per_gram  = EXCLUDED.price_per_gram,
    available_grams = EXCLUDED.available_grams,
    image_url       = EXCLUDED.image_url,
    is_available    = EXCLUDED.is_available;
