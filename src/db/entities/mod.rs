// SeaORM entities. Минимальный набор: подключаем по мере перевода эндпоинтов.
// Each entity module is `pub mod` so integration tests reach
// `turbobaby_bot::db::entities::*`. Each individual entity file
// has its own `#![allow(unreachable_pub)]` (SeaORM macros generate
// pub items unconditionally).
#[allow(unreachable_pub)]
pub mod accessory;
// The bike domain. `bike` is the family a customer chooses, `bike_unit` the
// machine the shop hands over, and `class_discount` / `rental_term` the two
// published discount tables the client price is computed from.
#[allow(unreachable_pub)]
pub mod bike;
#[allow(unreachable_pub)]
pub mod bike_service_record;
#[allow(unreachable_pub)]
pub mod bike_unit;
pub mod block_history;
#[allow(unreachable_pub)]
pub mod cart;
#[allow(unreachable_pub)]
pub mod cart_item;
#[allow(unreachable_pub)]
pub mod class_discount;
#[allow(unreachable_pub)]
pub mod rental_term;

#[allow(unreachable_pub)]
pub mod bonus_transaction;
#[allow(unreachable_pub)]
pub mod delivery_zone;
#[allow(unreachable_pub)]
pub mod event;
#[allow(unreachable_pub)]
pub mod event_photo;
// `garden_config` stood here. It was the only remaining reference to the
// `garden_config` table, so deleting the mechanic (D5) left a SeaORM entity
// mapping a table nothing else in the tree mentions. The table itself stays —
// see `ALLOWED_ORPHANS` in src/db/mod.rs for why it is not dropped.
#[allow(unreachable_pub)]
pub mod lab_certificate;
#[allow(unreachable_pub)]
pub mod loyalty_config;
#[allow(unreachable_pub)]
pub mod loyalty_idempotency_key;
#[allow(unreachable_pub)]
pub mod loyalty_profile;
#[allow(unreachable_pub)]
pub mod notification_queue;
#[allow(unreachable_pub)]
pub mod order;
#[allow(unreachable_pub)]
pub mod order_fraud_event;
#[allow(unreachable_pub)]
pub mod order_idempotency_key;
#[allow(unreachable_pub)]
pub mod quest_place;
#[allow(unreachable_pub)]
pub mod referral_event;
#[allow(unreachable_pub)]
pub mod referral_milestone;
#[allow(unreachable_pub)]
pub mod stars_idempotency_key;
#[allow(unreachable_pub)]
pub mod stars_transaction;
#[allow(unreachable_pub)]
pub mod strain;
#[allow(unreachable_pub)]
pub mod strain_review;
#[allow(unreachable_pub)]
pub mod tea_product;
#[allow(unreachable_pub)]
pub mod treasure_hunt;
#[allow(unreachable_pub)]
pub mod user;
#[allow(unreachable_pub)]
pub mod user_stars;
