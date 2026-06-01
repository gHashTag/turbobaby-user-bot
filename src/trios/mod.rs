//! Trios ecosystem - business logic modules

pub mod checkout_errors;
pub mod core;
pub mod garden;
pub mod i18n;
pub mod pricing;
pub mod quest;
pub mod store;
pub mod validation;

// Re-export commonly used types
pub use core::{Error, Lang, QrToken, Result, TelegramId, Timestamp};
