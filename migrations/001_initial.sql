-- User languages
CREATE TABLE IF NOT EXISTS user_languages (
    telegram_id BIGINT PRIMARY KEY,
    language VARCHAR(10) NOT NULL DEFAULT 'en',
    timezone VARCHAR(50) DEFAULT 'Asia/Bangkok',
    first_name TEXT,
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

-- Loyalty profiles
CREATE TABLE IF NOT EXISTS loyalty_profiles (
    telegram_id BIGINT PRIMARY KEY,
    total_spent DOUBLE PRECISION NOT NULL DEFAULT 0,
    bonus_balance DOUBLE PRECISION NOT NULL DEFAULT 0,
    tier VARCHAR(20) NOT NULL DEFAULT 'none',
    referral_code VARCHAR(50) UNIQUE,
    referred_by BIGINT,
    referral_count INTEGER NOT NULL DEFAULT 0,
    first_purchase_at TIMESTAMPTZ,
    manager_telegram_id BIGINT,
    is_blocked BOOLEAN NOT NULL DEFAULT false,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Bonus transactions
CREATE TABLE IF NOT EXISTS bonus_transactions (
    id VARCHAR(36) PRIMARY KEY,
    telegram_id BIGINT NOT NULL,
    amount DOUBLE PRECISION NOT NULL,
    tx_type VARCHAR(50) NOT NULL,
    description TEXT,
    related_order_id VARCHAR(36),
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- Strains
CREATE TABLE IF NOT EXISTS strains (
    id VARCHAR(36) PRIMARY KEY DEFAULT gen_random_uuid()::text,
    name TEXT NOT NULL,
    category VARCHAR(50),
    thc_percent DOUBLE PRECISION,
    cbd_percent DOUBLE PRECISION,
    effect TEXT,
    flavor_profile TEXT,
    description TEXT,
    price_per_gram DOUBLE PRECISION NOT NULL DEFAULT 0,
    available_grams DOUBLE PRECISION,
    image_url TEXT,
    is_available BOOLEAN NOT NULL DEFAULT true,
    is_strain_of_day BOOLEAN NOT NULL DEFAULT false,
    strain_of_day_discount DOUBLE PRECISION NOT NULL DEFAULT 0,
    strain_of_day_set_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- Orders
CREATE TABLE IF NOT EXISTS orders (
    id VARCHAR(36) PRIMARY KEY,
    telegram_id BIGINT NOT NULL,
    customer_name TEXT,
    customer_phone TEXT,
    customer_telegram TEXT,
    items JSONB NOT NULL DEFAULT '[]',
    subtotal DOUBLE PRECISION NOT NULL DEFAULT 0,
    bonus_used DOUBLE PRECISION NOT NULL DEFAULT 0,
    total DOUBLE PRECISION NOT NULL DEFAULT 0,
    status VARCHAR(30) NOT NULL DEFAULT 'pending',
    shop_id VARCHAR(36),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Managers
CREATE TABLE IF NOT EXISTS managers (
    telegram_id BIGINT PRIMARY KEY,
    name TEXT NOT NULL,
    username TEXT,
    ref_code VARCHAR(50) UNIQUE,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- Loyalty config
CREATE TABLE IF NOT EXISTS loyalty_config (
    id INTEGER PRIMARY KEY DEFAULT 1,
    config JSONB NOT NULL DEFAULT '{}'
);

-- Insert default loyalty config
INSERT INTO loyalty_config (id, config) VALUES (1, '{
    "bronze_threshold": 3000,
    "silver_threshold": 10000,
    "gold_threshold": 30000,
    "bronze_cashback_pct": 5,
    "silver_cashback_pct": 7,
    "gold_cashback_pct": 10,
    "referral_bonus": 200,
    "max_bonus_usage_pct": 30,
    "progressive_cashback": [2, 3, 4, 5, 6, 7, 8, 9, 10],
    "happy_hour_start": 14,
    "happy_hour_end": 17,
    "happy_hour_enabled": false,
    "happy_hour_discount": 10
}'::jsonb) ON CONFLICT (id) DO NOTHING;

-- Indexes
CREATE INDEX IF NOT EXISTS idx_orders_telegram_id ON orders(telegram_id);
CREATE INDEX IF NOT EXISTS idx_orders_status ON orders(status);
CREATE INDEX IF NOT EXISTS idx_orders_created_at ON orders(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_bonus_transactions_telegram_id ON bonus_transactions(telegram_id);
CREATE INDEX IF NOT EXISTS idx_strains_is_available ON strains(is_available);
CREATE INDEX IF NOT EXISTS idx_strains_strain_of_day ON strains(is_strain_of_day);
