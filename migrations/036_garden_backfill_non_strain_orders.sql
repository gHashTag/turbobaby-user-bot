-- Cycle #169B: backfill garden plants for users whose completed orders
-- contained ONLY non-strain items (sets, accessories, tea). Migration 026
-- only handled orders with a `strain_id`; users who ordered sets / accessories
-- / tea before cycle #169A were silently skipped and still see
-- "Order any product to get your first seed!" with no plant.
--
-- Idempotent via WHERE NOT EXISTS (no double-seed if rerun).

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
SELECT
    gen_random_uuid()::text AS id,
    o.telegram_id::text AS user_id,
    COALESCE(item->>'set_id', item->>'accessory_id', item->>'tea_id') AS strain_id,
    COALESCE(item->>'set_name', item->>'accessory_name', item->>'tea_name') AS strain_name,
    'seed' AS current_stage,
    EXTRACT(EPOCH FROM o.created_at)::bigint * 1000 AS planted_at,
    false AS is_completed,
    0 AS water_count
FROM (
    SELECT DISTINCT ON (o.telegram_id)
        o.telegram_id,
        o.created_at,
        o.items
    FROM orders o
    WHERE o.telegram_id IS NOT NULL
      AND o.status = 'completed'
      -- No active plant already (same guard as migration 026)
      AND NOT EXISTS (
          SELECT 1 FROM garden_plants p
          WHERE p.user_id = o.telegram_id::text
            AND p.is_completed = false
      )
      -- Order has at least one non-strain catalog item with a non-empty id
      AND EXISTS (
          SELECT 1 FROM jsonb_array_elements(o.items) AS it
          WHERE (
              (it->>'set_id' IS NOT NULL AND it->>'set_id' <> '')
               OR
              (it->>'accessory_id' IS NOT NULL AND it->>'accessory_id' <> '')
               OR
              (it->>'tea_id' IS NOT NULL AND it->>'tea_id' <> '')
          )
      )
      -- Make sure we don't re-process users already covered by migration 026
      -- (strain orders). We only want users whose FIRST completed order that
      -- would have produced a plant is a non-strain one, OR users who had no
      -- strain orders at all. The DISTINCT ON + ORDER BY created_at ASC picks
      -- the earliest completed order per user. If that earliest order has a
      -- strain, migration 026 already handled them. So we additionally guard
      -- that the earliest order has NO strain items.
      AND NOT EXISTS (
          SELECT 1 FROM jsonb_array_elements(o.items) AS it
          WHERE it->>'strain_id' IS NOT NULL AND it->>'strain_id' <> ''
      )
    ORDER BY o.telegram_id, o.created_at ASC
) AS o,
LATERAL (
    SELECT it AS item
    FROM jsonb_array_elements(o.items) AS it
    WHERE (
        (it->>'set_id' IS NOT NULL AND it->>'set_id' <> '')
         OR
        (it->>'accessory_id' IS NOT NULL AND it->>'accessory_id' <> '')
         OR
        (it->>'tea_id' IS NOT NULL AND it->>'tea_id' <> '')
    )
    LIMIT 1
) AS first_item
WHERE NOT EXISTS (
    SELECT 1 FROM garden_plants p
    WHERE p.user_id = o.telegram_id::text
      AND p.is_completed = false
);
