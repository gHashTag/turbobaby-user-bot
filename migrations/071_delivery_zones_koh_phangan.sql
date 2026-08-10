-- The shop is on Koh Phangan. The delivery-zone picker offered Pattaya.
--
-- Migration 060 seeded a single "Паттайя" zone as a placeholder matching an
-- older hard-coded fallback, and nothing ever replaced it — so the only zone a
-- customer could pick at checkout was a city 700 km away on the mainland.
-- Meanwhile `src/delivery.rs` has carried the real island zones all along;
-- they were simply never in the table the API reads.
--
-- 060 is left as history: it already ran everywhere, and editing an applied
-- migration would make a fresh database disagree with production about what
-- has been applied. This one corrects the data instead.

-- Retire the placeholder. Deactivated rather than deleted: orders already
-- placed reference `delivery_zone_id`, and deleting the row would orphan them.
UPDATE delivery_zones
SET is_active = FALSE,
    updated_at = now()
WHERE name_en = 'Pattaya' OR name = 'Паттайя';

-- The real zones, matching `DeliveryZones::default()` in src/delivery.rs so
-- the database and the built-in fallback cannot describe different islands.
INSERT INTO delivery_zones (name, name_en, fee, min_order, eta_min, eta_max, sort_order)
SELECT * FROM (
    VALUES
        ('Тонгсала',              'Thong Sala',           60.0,  0.0, 25, 50, 1),
        ('Хаад Рин',              'Haad Rin',             50.0,  0.0, 20, 40, 2),
        ('Сритхану',              'Srithanu',             80.0,  0.0, 30, 60, 3),
        ('Боттл Бич',             'Bottle Beach',        150.0,  0.0, 45, 90, 4),
        ('Самовывоз (Woody Weed)','Pickup at Woody Weed',  0.0,  0.0, 15, 15, 0)
) AS v(name, name_en, fee, min_order, eta_min, eta_max, sort_order)
-- Idempotent by name: re-running must not duplicate a zone, and an admin who
-- has already renamed one by hand must not have it recreated underneath them.
WHERE NOT EXISTS (
    SELECT 1 FROM delivery_zones z WHERE z.name_en = v.name_en
);
