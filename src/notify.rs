use teloxide::Bot;
use teloxide::prelude::*;
use crate::config::Config;

pub async fn notify_admins(bot: &Bot, config: &Config, text: &str) {
    for admin_id in &config.admin_ids {
        let _ = bot.send_message(teloxide::types::ChatId(*admin_id), text).await;
    }
}
