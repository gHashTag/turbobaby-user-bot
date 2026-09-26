//! Trios ecosystem - business logic modules

pub mod api_errors;
pub mod attendees;
pub mod calendar;
pub mod checkout_errors;
pub mod core;
pub mod deeplink;
pub mod drink_categories;
pub mod health;
pub mod i18n;
pub mod js_errors;
pub mod loyalty;
pub mod market;
pub mod order_status_view;
pub mod packs;
pub mod person;
pub mod pricing;
pub mod promo;
pub mod quest;
pub mod referrals;
pub mod stars_cap;
pub mod store;
pub mod validation;

// Re-export commonly used types
pub use core::{Error, Lang, QrToken, Result, TelegramId, Timestamp};

// What a customer is shown of the content the previous shop left in storage
// (owner, 2026-09-25, answer 3). Declared after the re-exports, in a group of
// its own, so that no line of this file moves.
pub mod legacy_view;

// The name a bike line is printed under on the customer's order screens
// (2026-09-26). Appended, like the group above, so that no line of this file moves.
pub mod order_line;

// The referral credit of 2026-09-26 (R3). Appended so that no line of this file moves.
pub mod referral_credit;
