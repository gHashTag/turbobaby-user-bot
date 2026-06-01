pub mod callbacks;
pub mod commands;
pub mod handlers;

use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

/// Shared AI rate-limit map across all bot entrypoints (text, commands, callbacks).
pub static AI_RATE_LIMIT: std::sync::LazyLock<Mutex<HashMap<i64, Instant>>> =
    std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));
pub const AI_COOLDOWN: Duration = Duration::from_secs(5);

use teloxide::dispatching::UpdateHandler;
use teloxide::prelude::*;
use teloxide::types::{InlineKeyboardButton, WebAppInfo};

pub(crate) fn web_app_btn(text: &str, url: &str) -> InlineKeyboardButton {
    match url.parse() {
        Ok(u) => InlineKeyboardButton::web_app(text, WebAppInfo { url: u }),
        Err(e) => {
            tracing::error!("Invalid web_app URL '{}': {}", url, e);
            InlineKeyboardButton::url(
                text,
                "https://t.me".parse().expect("static URL is always valid"),
            )
        }
    }
}

pub(crate) fn callback_btn(text: &str, data: &str) -> InlineKeyboardButton {
    InlineKeyboardButton::callback(text, data)
}

pub(crate) fn url_btn(text: &str, url: &str) -> InlineKeyboardButton {
    match url.parse() {
        Ok(u) => InlineKeyboardButton::url(text, u),
        Err(e) => {
            tracing::error!("Invalid URL '{}': {}", url, e);
            InlineKeyboardButton::url(
                text,
                "https://t.me".parse().expect("static URL is always valid"),
            )
        }
    }
}

/// Wrap a Telegram API call that's allowed to fail silently. Rate-limit
/// (HTTP 429) and "message too old to edit" failures are routine — those
/// are dropped without noise. Everything else gets a `debug!` log so ops
/// can see real connectivity / permissions issues that previously got
/// swallowed by `.await.ok()`.
///
/// Cycle #78: was 15+ bare `.ok()` callsites on `edit_message_text` /
/// `delete_message` etc. Replacing every one is invasive; this helper
/// targets the highest-signal ones where the user sees the failure (e.g.
/// "Joke fail" stuck on screen because the final edit was rejected).
pub(crate) async fn tg_fire_and_forget<T, E: std::fmt::Display>(
    fut: impl std::future::Future<Output = std::result::Result<T, E>>,
    ctx: &'static str,
) {
    if let Err(e) = fut.await {
        let s = e.to_string();
        let is_expected = s.contains("Too Many Requests")
            || s.contains("retry_after")
            || s.contains("message is not modified")
            || s.contains("message to edit not found")
            || s.contains("message to delete not found");
        if !is_expected {
            tracing::debug!("tg: {} failed: {}", ctx, s);
        }
    }
}

pub fn create_handler() -> UpdateHandler<teloxide::RequestError> {
    dptree::entry()
        .branch(
            Update::filter_message()
                .branch(
                    dptree::entry()
                        .filter_command::<commands::Command>()
                        .endpoint(commands::handle_command),
                )
                .branch(Message::filter_text().endpoint(handlers::handle_text))
                .branch(
                    dptree::filter(|msg: Message| msg.web_app_data().is_some())
                        .endpoint(handlers::handle_web_app_data),
                ),
        )
        .branch(Update::filter_callback_query().endpoint(callbacks::handle_callback))
}
