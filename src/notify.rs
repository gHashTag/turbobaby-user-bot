use teloxide::Bot;
use teloxide::prelude::*;
use crate::config::Config;

pub async fn notify_admins(bot: &Bot, config: &Config, text: &str) {
    for admin_id in &config.admin_ids {
        if let Err(e) = bot.send_message(teloxide::types::ChatId(*admin_id), text).await {
            tracing::warn!("notify_admins failed for admin_id={}: {}", admin_id, e);
        }
    }
}
