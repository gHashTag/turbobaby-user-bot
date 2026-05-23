-- Add standalone fields to hunt_checkpoints so checkpoints can have their own name/lat/lon
-- (old schema required a quest_place reference; new admin UI manages checkpoints directly)
ALTER TABLE hunt_checkpoints
ADD COLUMN IF NOT EXISTS name TEXT,
ADD COLUMN IF NOT EXISTS lat DOUBLE PRECISION NOT NULL DEFAULT 0,
ADD COLUMN IF NOT EXISTS lon DOUBLE PRECISION NOT NULL DEFAULT 0;
