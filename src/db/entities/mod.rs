// SeaORM entities. Минимальный набор: подключаем по мере перевода эндпоинтов.
// Each entity module is `pub mod` so integration tests reach
// `woody_weed_bot::db::entities::*`. Each individual entity file
// has its own `#![allow(unreachable_pub)]` (SeaORM macros generate
// pub items unconditionally).
#[allow(unreachable_pub)]
pub mod accessory;
pub mod block_history;
#[allow(unreachable_pub)]
pub mod cart;
#[allow(unreachable_pub)]
pub mod cart_item;

#[allow(unreachable_pub)]
pub mod bonus_transaction;
#[allow(unreachable_pub)]
pub mod delivery_zone;
#[allow(unreachable_pub)]
pub mod event;
#[allow(unreachable_pub)]
pub mod event_photo;
#[allow(unreachable_pub)]
pub mod garden_config;
#[allow(unreachable_pub)]
pub mod lab_certificate;
#[allow(unreachable_pub)]
pub mod loyalty_config;
#[allow(unreachable_pub)]
pub mod loyalty_idempotency_key;
#[allow(unreachable_pub)]
pub mod loyalty_profile;
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
