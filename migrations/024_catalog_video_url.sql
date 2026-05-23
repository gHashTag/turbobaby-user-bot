-- Add video_url column to catalog tables
ALTER TABLE accessories ADD COLUMN IF NOT EXISTS video_url TEXT;
ALTER TABLE tea_products ADD COLUMN IF NOT EXISTS video_url TEXT;
ALTER TABLE sets ADD COLUMN IF NOT EXISTS video_url TEXT;
ALTER TABLE accessory_sets ADD COLUMN IF NOT EXISTS video_url TEXT;
ALTER TABLE tea_sets ADD COLUMN IF NOT EXISTS video_url TEXT;
