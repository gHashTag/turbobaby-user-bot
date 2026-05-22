-- Make telegram_id nullable in orders table to support non-TMA (web) checkouts
ALTER TABLE orders ALTER COLUMN telegram_id DROP NOT NULL;
