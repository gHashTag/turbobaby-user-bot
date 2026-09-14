-- 085: unpublish the Woody WeedPecker catalogue this database was born with.
--
-- Mandate, owner, 2026-09-14: «из WOODY идут события рассылки, а надо чтобы у
-- каждого своя!! сейчас все путается!», «раздели и почини всё». This repo forked
-- from the woody bot carrying its live data: 30 events (DJ sets, cannabis
-- sommelier nights — all venue Woody WeedPecker), 70 accessories, 30 teas,
-- 8 sets. TurboBaby rents motorbikes; that catalogue is another shop's goods,
-- and while it stayed public this bot promoted it: promo drafts about woody
-- events to the admins, event reminders to booked customers, and the whole
-- catalogue in the miniapp.
--
-- Rows are KEPT, only the visibility flags flip:
--   * `events.is_public = FALSE`     — hides the calendar, stops the promo
--                                       event scans and the 24h reminder loop
--                                       (all three filter `is_public = TRUE`);
--   * `*.is_available = FALSE`       — hides the goods lists, stops the promo
--                                       catalog/bestseller scans (they filter
--                                       `is_available = TRUE`).
-- Old orders and bookings still resolve their item names, and everything is
-- reversible by flipping the flags back. New TurboBaby events, when the owner
-- creates them, start from a clean public slate.
--
-- No table is dropped and no row is deleted (DECISIONS.md, D8).

UPDATE events         SET is_public    = FALSE WHERE is_public    = TRUE;
UPDATE accessories    SET is_available = FALSE WHERE is_available = TRUE;
UPDATE accessory_sets SET is_available = FALSE WHERE is_available = TRUE;
UPDATE tea_products   SET is_available = FALSE WHERE is_available = TRUE;
UPDATE tea_sets       SET is_available = FALSE WHERE is_available = TRUE;
UPDATE sets           SET is_available = FALSE WHERE is_available = TRUE;

-- The pickup zone carries the old shop's name in the checkout picker.
-- Rebrand the row in place; the Phangan zone names themselves (Thong Sala,
-- Haad Rin, …) are the other shop's delivery map and stay until the owner
-- defines TurboBaby's Phuket zones — flagged to him, not decided here.
UPDATE delivery_zones
   SET name = 'Самовывоз (TurboBaby)', name_en = 'Pickup at TurboBaby'
 WHERE name LIKE '%Woody%' OR name_en LIKE '%Woody%';
