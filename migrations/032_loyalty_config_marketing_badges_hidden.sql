-- TZ #2 §5: admin-controlled toggle for hiding Sale / Best Seller /
-- New Arrival badges from customer-facing /api/strains responses.
--
-- Pre-cycle #136 this lived in the `HIDE_MARKETING_BADGES` env var —
-- which violated the ТЗ "Все изменения должны происходить без участия
-- программиста" because changing the env requires a Railway redeploy.
-- Moving it to a DB column makes the toggle self-service.
--
-- Lands on the existing `loyalty_config` singleton (id = 1) instead of
-- introducing a new app_settings table. The env var still wins when
-- set, as an ops kill-switch independent of DB state.

ALTER TABLE loyalty_config
    ADD COLUMN IF NOT EXISTS marketing_badges_hidden BOOLEAN NOT NULL DEFAULT FALSE;
