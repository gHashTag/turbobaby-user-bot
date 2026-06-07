-- Cycle #169: referral code is now the raw telegram_id (human-readable
-- deep-links). The old VARCHAR(20) limit was designed for 8-char base-62
-- hashes and is too small for a 19-digit i64.

ALTER TABLE referral_events
    ALTER COLUMN code TYPE VARCHAR(50);
