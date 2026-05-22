-- Migration 020: Принудительно конвертируем ВСЕ NUMERIC колонки на DOUBLE PRECISION.
--
-- Корневая причина (та же что в 012/015): прод-БД создавалась вручную с NUMERIC
-- для координат, сумм, наград и балансов. Исходные DDL уже DOUBLE PRECISION, но
-- на проде типы остались NUMERIC, и tokio-postgres `try_get::<f64>` падает с
-- "error serializing parameter / error converting" → API возвращает 500.
--
-- Wave 2 чинила INSERT-пути через явные `::float8` касты в SeaORM. Эта миграция
-- убирает корень проблемы целиком — после неё касты в коде не нужны.
--
-- Идемпотентно: оборачиваем каждый ALTER в DO-блок с проверкой текущего типа.

DISCARD PLANS;

-- ── Хелпер ──
-- Используем повторяющийся паттерн через DO-блоки.
-- Каждый блок: если колонка существует и тип != double precision, то ALTER.

-- ╔═══════════════════════════════════════════════════════════════╗
-- ║ quest_places: lat, lon                                         ║
-- ╚═══════════════════════════════════════════════════════════════╝
DO $$
BEGIN
  IF EXISTS (SELECT 1 FROM information_schema.columns
             WHERE table_name='quest_places' AND column_name='lat'
               AND data_type <> 'double precision') THEN
    ALTER TABLE quest_places ALTER COLUMN lat TYPE DOUBLE PRECISION USING lat::double precision;
  END IF;
  IF EXISTS (SELECT 1 FROM information_schema.columns
             WHERE table_name='quest_places' AND column_name='lon'
               AND data_type <> 'double precision') THEN
    ALTER TABLE quest_places ALTER COLUMN lon TYPE DOUBLE PRECISION USING lon::double precision;
  END IF;
END $$;

-- ╔═══════════════════════════════════════════════════════════════╗
-- ║ treasure_hunts: start_lat, start_lon                           ║
-- ╚═══════════════════════════════════════════════════════════════╝
DO $$
BEGIN
  IF EXISTS (SELECT 1 FROM information_schema.columns
             WHERE table_name='treasure_hunts' AND column_name='start_lat'
               AND data_type <> 'double precision') THEN
    ALTER TABLE treasure_hunts ALTER COLUMN start_lat TYPE DOUBLE PRECISION USING start_lat::double precision;
  END IF;
  IF EXISTS (SELECT 1 FROM information_schema.columns
             WHERE table_name='treasure_hunts' AND column_name='start_lon'
               AND data_type <> 'double precision') THEN
    ALTER TABLE treasure_hunts ALTER COLUMN start_lon TYPE DOUBLE PRECISION USING start_lon::double precision;
  END IF;
END $$;

-- ╔═══════════════════════════════════════════════════════════════╗
-- ║ orders: subtotal, bonus_used, total                            ║
-- ╚═══════════════════════════════════════════════════════════════╝
DO $$
BEGIN
  IF EXISTS (SELECT 1 FROM information_schema.columns
             WHERE table_name='orders' AND column_name='subtotal'
               AND data_type <> 'double precision') THEN
    ALTER TABLE orders ALTER COLUMN subtotal TYPE DOUBLE PRECISION USING subtotal::double precision;
  END IF;
  IF EXISTS (SELECT 1 FROM information_schema.columns
             WHERE table_name='orders' AND column_name='bonus_used'
               AND data_type <> 'double precision') THEN
    ALTER TABLE orders ALTER COLUMN bonus_used TYPE DOUBLE PRECISION USING bonus_used::double precision;
  END IF;
  IF EXISTS (SELECT 1 FROM information_schema.columns
             WHERE table_name='orders' AND column_name='total'
               AND data_type <> 'double precision') THEN
    ALTER TABLE orders ALTER COLUMN total TYPE DOUBLE PRECISION USING total::double precision;
  END IF;
END $$;

-- ╔═══════════════════════════════════════════════════════════════╗
-- ║ loyalty_profiles: total_spent, bonus_balance                   ║
-- ╚═══════════════════════════════════════════════════════════════╝
DO $$
BEGIN
  IF EXISTS (SELECT 1 FROM information_schema.columns
             WHERE table_name='loyalty_profiles' AND column_name='total_spent'
               AND data_type <> 'double precision') THEN
    ALTER TABLE loyalty_profiles ALTER COLUMN total_spent TYPE DOUBLE PRECISION USING total_spent::double precision;
  END IF;
  IF EXISTS (SELECT 1 FROM information_schema.columns
             WHERE table_name='loyalty_profiles' AND column_name='bonus_balance'
               AND data_type <> 'double precision') THEN
    ALTER TABLE loyalty_profiles ALTER COLUMN bonus_balance TYPE DOUBLE PRECISION USING bonus_balance::double precision;
  END IF;
END $$;

-- ╔═══════════════════════════════════════════════════════════════╗
-- ║ bonus_transactions: amount                                     ║
-- ╚═══════════════════════════════════════════════════════════════╝
DO $$
BEGIN
  IF EXISTS (SELECT 1 FROM information_schema.columns
             WHERE table_name='bonus_transactions' AND column_name='amount'
               AND data_type <> 'double precision') THEN
    ALTER TABLE bonus_transactions ALTER COLUMN amount TYPE DOUBLE PRECISION USING amount::double precision;
  END IF;
END $$;

-- ╔═══════════════════════════════════════════════════════════════╗
-- ║ garden_rewards: discount_percent, bonus_points (если есть NUMERIC)║
-- ╚═══════════════════════════════════════════════════════════════╝
-- В DDL они INTEGER, но если на проде создались как NUMERIC — поправим.
DO $$
BEGIN
  IF EXISTS (SELECT 1 FROM information_schema.columns
             WHERE table_name='garden_rewards' AND column_name='discount_percent'
               AND data_type = 'numeric') THEN
    ALTER TABLE garden_rewards ALTER COLUMN discount_percent TYPE INTEGER USING discount_percent::integer;
  END IF;
  IF EXISTS (SELECT 1 FROM information_schema.columns
             WHERE table_name='garden_rewards' AND column_name='bonus_points'
               AND data_type = 'numeric') THEN
    ALTER TABLE garden_rewards ALTER COLUMN bonus_points TYPE INTEGER USING bonus_points::integer;
  END IF;
END $$;

-- ╔═══════════════════════════════════════════════════════════════╗
-- ║ garden_config: reward_discount_percent, reward_bonus_points,   ║
-- ║                reward_expiration_days                          ║
-- ╚═══════════════════════════════════════════════════════════════╝
DO $$
BEGIN
  IF EXISTS (SELECT 1 FROM information_schema.columns
             WHERE table_name='garden_config' AND column_name='reward_discount_percent'
               AND data_type = 'numeric') THEN
    ALTER TABLE garden_config ALTER COLUMN reward_discount_percent TYPE INTEGER USING reward_discount_percent::integer;
  END IF;
  IF EXISTS (SELECT 1 FROM information_schema.columns
             WHERE table_name='garden_config' AND column_name='reward_bonus_points'
               AND data_type = 'numeric') THEN
    ALTER TABLE garden_config ALTER COLUMN reward_bonus_points TYPE INTEGER USING reward_bonus_points::integer;
  END IF;
  IF EXISTS (SELECT 1 FROM information_schema.columns
             WHERE table_name='garden_config' AND column_name='reward_expiration_days'
               AND data_type = 'numeric') THEN
    ALTER TABLE garden_config ALTER COLUMN reward_expiration_days TYPE INTEGER USING reward_expiration_days::integer;
  END IF;
END $$;
