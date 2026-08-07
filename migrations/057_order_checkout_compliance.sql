-- Loop #7: order-level compliance fields for checkout conversion.
-- age_confirmed captures the explicit 20+ per-order confirmation required
-- by the cannabis-delivery compliance path. delivery_zone_id stores the
-- config zone chosen at checkout so the order record stays authoritative.
ALTER TABLE orders
ADD COLUMN IF NOT EXISTS age_confirmed BOOLEAN NOT NULL DEFAULT FALSE,
ADD COLUMN IF NOT EXISTS delivery_zone_id TEXT;
