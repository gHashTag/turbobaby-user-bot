-- Quest Places
CREATE TABLE IF NOT EXISTS quest_places (
    id TEXT PRIMARY KEY DEFAULT gen_random_uuid()::text,
    name TEXT NOT NULL,
    category TEXT NOT NULL DEFAULT 'location',
    lat DOUBLE PRECISION NOT NULL DEFAULT 0,
    lon DOUBLE PRECISION NOT NULL DEFAULT 0,
    description TEXT,
    image_url TEXT,
    is_available BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Treasure Hunts
CREATE TABLE IF NOT EXISTS treasure_hunts (
    id TEXT PRIMARY KEY DEFAULT gen_random_uuid()::text,
    name TEXT NOT NULL,
    description TEXT,
    image_url TEXT,
    black_mark_title TEXT NOT NULL DEFAULT '',
    black_mark_description TEXT,
    black_mark_image_url TEXT,
    is_active BOOLEAN NOT NULL DEFAULT true,
    starts_at TIMESTAMPTZ,
    ends_at TIMESTAMPTZ,
    start_lat DOUBLE PRECISION NOT NULL DEFAULT 0,
    start_lon DOUBLE PRECISION NOT NULL DEFAULT 0,
    start_name TEXT NOT NULL DEFAULT '',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Hunt Checkpoints
CREATE TABLE IF NOT EXISTS hunt_checkpoints (
    id TEXT PRIMARY KEY DEFAULT gen_random_uuid()::text,
    hunt_id TEXT NOT NULL REFERENCES treasure_hunts(id) ON DELETE CASCADE,
    quest_place_id TEXT NOT NULL REFERENCES quest_places(id),
    checkpoint_order INTEGER NOT NULL DEFAULT 0,
    qr_token TEXT NOT NULL UNIQUE DEFAULT gen_random_uuid()::text,
    partner_reward_text TEXT,
    partner_logo_url TEXT,
    hint_text TEXT,
    is_active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Location Quest Locations
CREATE TABLE IF NOT EXISTS location_quest_locations (
    id SERIAL PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    category TEXT NOT NULL DEFAULT 'location',
    map_url TEXT NOT NULL DEFAULT '',
    qr_token TEXT NOT NULL UNIQUE DEFAULT gen_random_uuid()::text,
    is_active BOOLEAN NOT NULL DEFAULT true,
    is_final BOOLEAN NOT NULL DEFAULT false,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
