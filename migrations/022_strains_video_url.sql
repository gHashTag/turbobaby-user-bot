-- Add video_url column to strains table
ALTER TABLE strains ADD COLUMN IF NOT EXISTS video_url TEXT;
