-- Loop #11: server-side cart persistence.
-- Carts are tied to telegram_id; items reference catalog rows by kind + id.
-- Price/snapshot fields are stored so the cart survives catalog edits until
-- checkout re-validates against current DB prices.
CREATE TABLE IF NOT EXISTS carts (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    telegram_id BIGINT NOT NULL UNIQUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at TIMESTAMPTZ NOT NULL DEFAULT (now() + interval '30 days')
);

CREATE TABLE IF NOT EXISTS cart_items (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cart_id UUID NOT NULL REFERENCES carts(id) ON DELETE CASCADE,
    kind TEXT NOT NULL CHECK (kind IN ('strain', 'set', 'accessory', 'tea')),
    catalog_id TEXT NOT NULL,
    quantity INT NOT NULL CHECK (quantity > 0),
    unit_price DOUBLE PRECISION NOT NULL CHECK (unit_price >= 0),
    name TEXT NOT NULL,
    image_url TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (cart_id, kind, catalog_id)
);

CREATE INDEX IF NOT EXISTS idx_carts_telegram_id ON carts(telegram_id);
CREATE INDEX IF NOT EXISTS idx_cart_items_cart_id ON cart_items(cart_id);
