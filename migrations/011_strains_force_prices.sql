-- Migration 011: Принудительно обновляем цены, THC/CBD для strains
-- Анкер: φ² + φ⁻² = 3
-- Идемпотентно — UPDATE по name (UPPERCASE и mixed case оба).

-- BANANA FRITTER
UPDATE strains SET thc_percent = 26.0, cbd_percent = 0.1, price_per_gram = 350.0,
  effect = 'Comforting, relaxing, anxiety relief',
  flavor_profile = 'Myrcene, Caryophyllene, Limonene',
  available_grams = 30.0
  WHERE LOWER(name) = 'banana fritter';

-- BLACK MAMBA
UPDATE strains SET thc_percent = 27.0, cbd_percent = 0.1, price_per_gram = 500.0,
  effect = 'Very strong body stone, deep relaxation',
  flavor_profile = 'Myrcene, Caryophyllene, Limonene',
  available_grams = 25.0
  WHERE LOWER(name) = 'black mamba';

-- COLT 45
UPDATE strains SET thc_percent = 25.0, cbd_percent = 0.1, price_per_gram = 450.0,
  effect = 'Deep relaxation, heavy body, calm mind',
  flavor_profile = 'Myrcene, Caryophyllene, Humulene',
  available_grams = 40.0
  WHERE LOWER(name) = 'colt 45';

-- DIPZ
UPDATE strains SET thc_percent = 25.0, cbd_percent = 0.1, price_per_gram = 350.0,
  effect = 'Relaxing, smooth body high',
  flavor_profile = 'Myrcene, Caryophyllene, Linalool',
  available_grams = 35.0
  WHERE LOWER(name) = 'dipz';

-- JOKER CANDY
UPDATE strains SET thc_percent = 25.0, cbd_percent = 0.1, price_per_gram = 400.0,
  effect = 'Energizing, euphoric, creative, boosts mood',
  flavor_profile = 'Myrcene, Bisabolol, Caryophyllene',
  available_grams = 40.0
  WHERE LOWER(name) = 'joker candy';

-- L.A ULTRA (с точкой и без)
UPDATE strains SET thc_percent = 24.0, cbd_percent = 0.2, price_per_gram = 400.0,
  effect = 'Balanced euphoria, body relaxation',
  flavor_profile = 'Caryophyllene, Limonene, Myrcene',
  available_grams = 45.0
  WHERE LOWER(name) IN ('l.a ultra', 'la ultra');

-- MAC 1
UPDATE strains SET thc_percent = 24.0, cbd_percent = 0.1, price_per_gram = 400.0,
  effect = 'Energizing, euphoric, strong head & body high',
  flavor_profile = 'Caryophyllene, Limonene, Pinene',
  available_grams = 40.0
  WHERE LOWER(name) = 'mac 1';

-- MIAMI VICE
UPDATE strains SET thc_percent = 23.0, cbd_percent = 0.2, price_per_gram = 450.0,
  effect = 'Balanced, uplifting, smooth high',
  flavor_profile = 'Limonene, Myrcene, Caryophyllene',
  available_grams = 45.0
  WHERE LOWER(name) = 'miami vice';

-- MILK MONKEY
UPDATE strains SET thc_percent = 26.0, cbd_percent = 0.1, price_per_gram = 500.0,
  effect = 'Sedative, body-heavy, calming',
  flavor_profile = 'Myrcene, Linalool, Caryophyllene',
  available_grams = 30.0
  WHERE LOWER(name) = 'milk monkey';

-- SUPER BOOF
UPDATE strains SET thc_percent = 24.0, cbd_percent = 0.2, price_per_gram = 350.0,
  effect = 'Energetic, euphoric, creative',
  flavor_profile = 'Limonene, Caryophyllene, Pinene',
  available_grams = 40.0
  WHERE LOWER(name) = 'super boof';

-- SUPER LEMON HAZE
UPDATE strains SET thc_percent = 22.0, cbd_percent = 0.2, price_per_gram = 350.0,
  effect = 'Energizing, focused, uplifting',
  flavor_profile = 'Limonene, Terpinolene, Pinene',
  available_grams = 60.0
  WHERE LOWER(name) = 'super lemon haze';

-- SUPER RUNTZ
UPDATE strains SET thc_percent = 24.0, cbd_percent = 0.1, price_per_gram = 450.0,
  effect = 'Euphoric, happy, relaxed',
  flavor_profile = 'Limonene, Caryophyllene, Linalool',
  available_grams = 50.0
  WHERE LOWER(name) = 'super runtz';
