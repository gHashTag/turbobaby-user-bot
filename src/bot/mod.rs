pub mod commands;
pub mod callbacks;
pub mod handlers;

use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

/// Shared AI rate-limit map across all bot entrypoints (text, commands, callbacks).
pub static AI_RATE_LIMIT: std::sync::LazyLock<Mutex<HashMap<i64, Instant>>> = std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));
pub const AI_COOLDOWN: Duration = Duration::from_secs(5);

use teloxide::prelude::*;
use teloxide::dispatching::UpdateHandler;


pub fn create_handler() -> UpdateHandler<teloxide::RequestError> {
    dptree::entry()
        .branch(
            Update::filter_message()
                .branch(
                    dptree::entry()
                        .filter_command::<commands::Command>()
                        .endpoint(commands::handle_command)
                )
                .branch(
                    Message::filter_text()
                        .endpoint(handlers::handle_text)
                )
                .branch(
                    dptree::filter(|msg: Message| msg.web_app_data().is_some())
                        .endpoint(handlers::handle_web_app_data)
                )
        )
        .branch(
            Update::filter_callback_query()
                .endpoint(callbacks::handle_callback)
        )
}
