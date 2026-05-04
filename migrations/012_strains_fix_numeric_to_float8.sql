-- Migration 012: Принудительно конвертируем NUMERIC колонки strains в DOUBLE PRECISION
-- Анкер: φ² + φ⁻² = 3

-- Сбросить любые серверные cached plans перед ALTER TYPE
DISCARD PLANS;
--
-- Причина: продовая БД создавалась вручную с типом NUMERIC для price/thc/cbd колонок.
-- tokio-postgres `try_get::<f64>` на NUMERIC возвращает Err → from_row даёт 0.0/None.
-- ALTER TABLE ADD COLUMN IF NOT EXISTS из 008 был no-op потому что колонки уже были.
-- Здесь делаем явный TYPE-cast, идемпотентно (DO blocks с проверкой data_type).

DO $$
BEGIN
  IF (SELECT data_type FROM information_schema.columns
      WHERE table_name='strains' AND column_name='price_per_gram') <> 'double precision' THEN
    ALTER TABLE strains ALTER COLUMN price_per_gram TYPE DOUBLE PRECISION
      USING price_per_gram::double precision;
  END IF;
END $$;

DO $$
BEGIN
  IF (SELECT data_type FROM information_schema.columns
      WHERE table_name='strains' AND column_name='thc_percent') <> 'double precision' THEN
    ALTER TABLE strains ALTER COLUMN thc_percent TYPE DOUBLE PRECISION
      USING thc_percent::double precision;
  END IF;
END $$;

DO $$
BEGIN
  IF (SELECT data_type FROM information_schema.columns
      WHERE table_name='strains' AND column_name='cbd_percent') <> 'double precision' THEN
    ALTER TABLE strains ALTER COLUMN cbd_percent TYPE DOUBLE PRECISION
      USING cbd_percent::double precision;
  END IF;
END $$;

DO $$
BEGIN
  IF (SELECT data_type FROM information_schema.columns
      WHERE table_name='strains' AND column_name='available_grams') <> 'double precision' THEN
    ALTER TABLE strains ALTER COLUMN available_grams TYPE DOUBLE PRECISION
      USING available_grams::double precision;
  END IF;
END $$;

DO $$
BEGIN
  IF (SELECT data_type FROM information_schema.columns
      WHERE table_name='strains' AND column_name='strain_of_day_discount') <> 'double precision' THEN
    ALTER TABLE strains ALTER COLUMN strain_of_day_discount TYPE DOUBLE PRECISION
      USING strain_of_day_discount::double precision;
  END IF;
END $$;

-- После каста типов — повторный UPDATE цен (на случай если 011 не отработал из-за типов)
-- BANANA FRITTER
UPDATE strains SET thc_percent = 26.0, cbd_percent = 0.1, price_per_gram = 350.0,
  available_grams = 30.0
  WHERE LOWER(name) = 'banana fritter';

-- BLACK MAMBA
UPDATE strains SET thc_percent = 27.0, cbd_percent = 0.1, price_per_gram = 500.0,
  available_grams = 25.0
  WHERE LOWER(name) = 'black mamba';

-- COLT 45
UPDATE strains SET thc_percent = 25.0, cbd_percent = 0.1, price_per_gram = 450.0,
  available_grams = 40.0
  WHERE LOWER(name) = 'colt 45';

-- DIPZ
UPDATE strains SET thc_percent = 25.0, cbd_percent = 0.1, price_per_gram = 350.0,
  available_grams = 35.0
  WHERE LOWER(name) = 'dipz';

-- JOKER CANDY
UPDATE strains SET thc_percent = 25.0, cbd_percent = 0.1, price_per_gram = 400.0,
  available_grams = 40.0
  WHERE LOWER(name) = 'joker candy';

-- L.A ULTRA
UPDATE strains SET thc_percent = 24.0, cbd_percent = 0.2, price_per_gram = 400.0,
  available_grams = 45.0
  WHERE LOWER(name) IN ('l.a ultra', 'la ultra');

-- MAC 1
UPDATE strains SET thc_percent = 24.0, cbd_percent = 0.1, price_per_gram = 400.0,
  available_grams = 40.0
  WHERE LOWER(name) = 'mac 1';

-- MIAMI VICE
UPDATE strains SET thc_percent = 23.0, cbd_percent = 0.2, price_per_gram = 450.0,
  available_grams = 45.0
  WHERE LOWER(name) = 'miami vice';

-- MILK MONKEY
UPDATE strains SET thc_percent = 26.0, cbd_percent = 0.1, price_per_gram = 500.0,
  available_grams = 30.0
  WHERE LOWER(name) = 'milk monkey';

-- SUPER BOOF
UPDATE strains SET thc_percent = 24.0, cbd_percent = 0.2, price_per_gram = 350.0,
  available_grams = 40.0
  WHERE LOWER(name) = 'super boof';

-- SUPER LEMON HAZE
UPDATE strains SET thc_percent = 22.0, cbd_percent = 0.2, price_per_gram = 350.0,
  available_grams = 60.0
  WHERE LOWER(name) = 'super lemon haze';

-- SUPER RUNTZ
UPDATE strains SET thc_percent = 24.0, cbd_percent = 0.1, price_per_gram = 450.0,
  available_grams = 50.0
  WHERE LOWER(name) = 'super runtz';
