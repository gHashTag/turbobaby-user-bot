//! Trios ecosystem - business logic modules

pub mod core;
pub mod garden;
pub mod quest;
pub mod store;
pub mod i18n;
pub mod validation;

// Re-export commonly used types
pub use core::{Error, Result, Lang, TelegramId, QrToken, Timestamp};
