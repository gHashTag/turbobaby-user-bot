-- Cycle #169C: universal backfill for garden_plants.
--
-- Previous backfills (026 and 036) were too narrow:
--   * 026 only looked at orders with a strain_id.
--   * 036 only looked at users whose EARLIEST completed order had NO strain_id.
--
-- Users who had a strain order early (026 handled it), harvested the plant,
-- and later ordered sets/accessories/tea were still left without an active
-- plant because neither migration considered "post-harvest re-seeding".
--
-- This migration is simpler and universal: EVERY user without an active
-- (is_completed = false) plant gets a seed from their earliest completed
-- order that contains ANY catalog item with a non-empty id.
--
-- Idempotent via WHERE NOT EXISTS (safe to rerun).

WITH users_needing_plant AS (
    SELECT DISTINCT o.telegram_id
    FROM orders o
    WHERE o.telegram_id IS NOT NULL
      AND o.status = 'completed'
      -- Only users who currently have NO active plant at all
      AND NOT EXISTS (
          SELECT 1 FROM garden_plants p
          WHERE p.user_id = o.telegram_id::text
            AND p.is_completed = false
      )
),
earliest_order_per_user AS (
    SELECT DISTINCT ON (o.telegram_id)
        o.telegram_id,
        o.created_at,
        o.items
    FROM orders o
    WHERE o.telegram_id IN (SELECT telegram_id FROM users_needing_plant)
      AND o.status = 'completed'
    ORDER BY o.telegram_id, o.created_at ASC
),
first_valid_item AS (
    SELECT
        eo.telegram_id,
        eo.created_at,
        it AS item
    FROM earliest_order_per_user eo,
    LATERAL jsonb_array_elements(eo.items) AS it
    WHERE
        (it->>'strain_id' IS NOT NULL AND it->>'strain_id' <> '')
        OR (it->>'set_id'      IS NOT NULL AND it->>'set_id'      <> '')
        OR (it->>'accessory_id' IS NOT NULL AND it->>'accessory_id' <> '')
        OR (it->>'tea_id'       IS NOT NULL AND it->>'tea_id'       <> '')
    -- If an order has multiple items, pick the first array element that
    -- matches the filter above. jsonb_array_elements preserves order.
)
INSERT INTO garden_plants (
    id,
    user_id,
    strain_id,
    strain_name,
    current_stage,
    planted_at,
    is_completed,
    water_count
)
SELECT DISTINCT ON (telegram_id)
    gen_random_uuid()::text AS id,
    telegram_id::text AS user_id,
    COALESCE(
        NULLIF(item->>'strain_id', ''),
        NULLIF(item->>'set_id', ''),
        NULLIF(item->>'accessory_id', ''),
        NULLIF(item->>'tea_id', '')
    ) AS strain_id,
    COALESCE(
        NULLIF(item->>'strain_name', ''),
        NULLIF(item->>'set_name', ''),
        NULLIF(item->>'accessory_name', ''),
        NULLIF(item->>'tea_name', '')
    ) AS strain_name,
    'seed' AS current_stage,
    EXTRACT(EPOCH FROM created_at)::bigint * 1000 AS planted_at,
    false AS is_completed,
    0 AS water_count
FROM first_valid_item
WHERE NOT EXISTS (
    SELECT 1 FROM garden_plants p
    WHERE p.user_id = first_valid_item.telegram_id::text
      AND p.is_completed = false
)
ORDER BY telegram_id, created_at ASC;
