//! Trios ecosystem - business logic modules

pub mod api_errors;
pub mod attendees;
pub mod calendar;
pub mod checkout_errors;
pub mod core;
pub mod deeplink;
pub mod drink_categories;
pub mod garden;
pub mod health;
pub mod i18n;
pub mod js_errors;
pub mod packs;
pub mod person;
pub mod pricing;
pub mod quest;
pub mod referrals;
pub mod stars_cap;
pub mod store;
pub mod validation;

// Re-export commonly used types
pub use core::{Error, Lang, QrToken, Result, TelegramId, Timestamp};
