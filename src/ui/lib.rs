// Dioxus UI Library for woody-weed-bot
//
// This module contains the Telegram Mini App frontend built with Dioxus.
// It integrates with the existing Axum backend without API changes.

pub use app::App;
pub use app_simple::SimpleApp;
pub use routes::router;

pub mod app;
pub mod app_simple;
pub mod routes;
pub mod state;
pub mod telegram;

pub mod api;
pub mod components;
pub mod layouts;
pub mod pages;
pub mod game;
pub mod i18n;
pub mod theme;
pub mod test_simple;

// Re-export commonly used Dioxus types
pub use dioxus::prelude::*;
