use teloxide::Bot;
use teloxide::prelude::*;
use crate::config::Config;

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

pub async fn notify_admins(bot: &Bot, config: &Config, text: &str) {
    let safe = html_escape(text);
    for admin_id in &config.admin_ids {
        if let Err(e) = bot.send_message(teloxide::types::ChatId(*admin_id), &safe).await {
            tracing::warn!("notify_admins failed for admin_id={}: {}", admin_id, e);
        }
    }
}
