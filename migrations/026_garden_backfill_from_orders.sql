-- Cycle #15: profile UI promises "Order a strain to get your first seed!"
-- but `complete_order_and_update_loyalty` never seeded plants until now.
-- One-shot backfill: for every user who has a completed order containing a
-- strain item but currently has no active plant, plant a seed using their
-- earliest completed strain order.
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
    item->>'strain_id' AS strain_id,
    item->>'strain_name' AS strain_name,
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
      AND EXISTS (
          SELECT 1 FROM jsonb_array_elements(o.items) AS it
          WHERE it ? 'strain_id'
            AND it->>'strain_id' IS NOT NULL
            AND it->>'strain_id' <> ''
      )
    ORDER BY o.telegram_id, o.created_at ASC
) AS o,
LATERAL (
    SELECT it AS item
    FROM jsonb_array_elements(o.items) AS it
    WHERE it ? 'strain_id'
      AND it->>'strain_id' IS NOT NULL
      AND it->>'strain_id' <> ''
    LIMIT 1
) AS strain_item
WHERE NOT EXISTS (
    SELECT 1 FROM garden_plants p
    WHERE p.user_id = o.telegram_id::text
      AND p.is_completed = false
);
