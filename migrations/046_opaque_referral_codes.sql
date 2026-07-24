-- Cycle #next: migrate legacy telegram_id-based referral codes to opaque
-- 8-character alphanumeric codes. Existing referral_events.code rows are
-- synchronized so referrer attribution keeps working after deploy.

CREATE OR REPLACE FUNCTION _ww_generate_opaque_code() RETURNS TEXT AS $$
DECLARE
    chars TEXT := 'ABCDEFGHJKLMNPQRSTUVWXYZ23456789';
    code TEXT := '';
    i INT;
BEGIN
    FOR i IN 1..8 LOOP
        code := code || substr(chars, 1 + floor(random() * length(chars))::int, 1);
    END LOOP;
    RETURN code;
END;
$$ LANGUAGE plpgsql;

DO $$
DECLARE
    rec RECORD;
    new_code TEXT;
BEGIN
    FOR rec IN
        SELECT telegram_id, referral_code
        FROM loyalty_profiles
        WHERE referral_code IS NOT NULL
          AND referral_code ~ '^\d+$'
    LOOP
        LOOP
            new_code := _ww_generate_opaque_code();
            EXIT WHEN NOT EXISTS (SELECT 1 FROM loyalty_profiles WHERE referral_code = new_code);
        END LOOP;
        UPDATE loyalty_profiles SET referral_code = new_code WHERE telegram_id = rec.telegram_id;
        UPDATE referral_events SET code = new_code WHERE code = rec.referral_code;
    END LOOP;
END $$;

DROP FUNCTION IF EXISTS _ww_generate_opaque_code();
