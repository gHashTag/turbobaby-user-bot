-- Add commission_rate to managers table
ALTER TABLE managers ADD COLUMN IF NOT EXISTS commission_rate DOUBLE PRECISION DEFAULT 0;
