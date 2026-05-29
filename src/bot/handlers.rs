use std::sync::Arc;
use std::time::{Duration, Instant};
use teloxide::{
    prelude::*,
    types::{InlineKeyboardButton, InlineKeyboardMarkup, WebAppInfo},
};

use crate::{config::Config, db::Database, locales::*};
use crate::bot::{AI_RATE_LIMIT, AI_COOLDOWN};
use crate::bot::commands::build_app_url;
use crate::util::html_escape;

fn web_app_btn(text: &str, url: &str) -> InlineKeyboardButton {
    match url.parse() {
        Ok(u) => InlineKeyboardButton::web_app(text, WebAppInfo { url: u }),
        Err(e) => {
            tracing::error!("Invalid web_app URL '{}': {}", url, e);
            InlineKeyboardButton::url(text, "https://t.me".parse().expect("static URL is always valid"))
        }
    }
}
fn url_btn(text: &str, url: &str) -> InlineKeyboardButton {
    match url.parse() {
        Ok(u) => InlineKeyboardButton::url(text, u),
        Err(e) => {
            tracing::error!("Invalid URL '{}': {}", url, e);
            InlineKeyboardButton::url(text, "https://t.me".parse().expect("static URL is always valid"))
        }
    }
}

pub async fn handle_text(
    bot: Bot,
    msg: Message,
    db: Arc<Database>,
    config: Arc<Config>,
    ai_client: Arc<crate::ai::AiClient>,
) -> Result<(), teloxide::RequestError> {
    let text = match msg.text() {
        Some(t) => t.to_string(),
        None => return Ok(()),
    };

    // Cap length to prevent AI context-window abuse and API cost spikes
    let text = if text.len() > 1500 {
        text.chars().take(1500).collect::<String>()
    } else {
        text
    };

    // Skip commands
    if text.starts_with('/') { return Ok(()); }
    // Skip emoji keyboard presses
    if text.starts_with(['🎁', '🍷', '😜', '🧠', '🌐', '⚙', '🛒']) { return Ok(()); }

    let is_group = matches!(msg.chat.kind, teloxide::types::ChatKind::Public(_));
    let user_id = msg.from.as_ref().map(|u| u.id.0 as i64).unwrap_or(0);

    // Blocked-user guard
    if db.is_user_blocked(user_id).await.unwrap_or(false) {
        return Ok(());
    }

    let lang = detect_language(&text);
    let locale = get_locale(lang);

    // AI rate-limit: 1 request per 5 seconds per user
    {
        let now = Instant::now();
        let mut map = AI_RATE_LIMIT.lock().await;
        map.retain(|_, last| now.duration_since(*last) < Duration::from_secs(300));
        if let Some(last) = map.get(&user_id) {
            if now.duration_since(*last) < AI_COOLDOWN {
                tracing::warn!("AI rate limit hit for user_id={}", user_id);
                return Ok(());
            }
        }
        map.insert(user_id, now);
    }

    let user = msg.from.as_ref();
    let name = user
        .and_then(|u| if !u.first_name.is_empty() { Some(&u.first_name) } else { u.username.as_ref() })
        .map(|s| s.as_str())
        .unwrap_or("friend");
    let ai_response = ai_client.ask_grok(&text, name, &locale.lang_instruction).await;

    if let Some(response) = ai_response {
        let safe = html_escape(&response);
        if is_group {
            bot.send_message(msg.chat.id, safe)
                .parse_mode(teloxide::types::ParseMode::Html)
                .reply_markup(InlineKeyboardMarkup::new(vec![
                    vec![url_btn(&format!("🛒 {}", locale.open_menu),
                        &format!("https://t.me/{}?start=channel", config.bot_username))]
                ])).await?;
        } else {
            bot.send_message(msg.chat.id, safe)
                .parse_mode(teloxide::types::ParseMode::Html)
                .await?;
        }
    } else {
        let user_lang = db.get_user_lang(user_id).await.unwrap_or_else(|| lang.to_string());
        let user_locale = get_locale(&user_lang);
        let base = &config.web_app_url;
        if is_group {
            bot.send_message(msg.chat.id, &user_locale.menu)
                .reply_markup(InlineKeyboardMarkup::new(vec![
                    vec![url_btn(&user_locale.open_menu,
                        &format!("https://t.me/{}?start=channel", config.bot_username))]
                ])).await?;
        } else {
            bot.send_message(msg.chat.id, &user_locale.menu)
                .reply_markup(InlineKeyboardMarkup::new(vec![
                    vec![web_app_btn(&user_locale.open_menu, &build_app_url(base, &user_lang, None))]
                ])).await?;
        }
    }

    Ok(())
}

pub async fn handle_web_app_data(
    _bot: Bot,
    msg: Message,
    db: Arc<Database>,
    _config: Arc<Config>,
) -> Result<(), teloxide::RequestError> {
    let user_id = msg.from.as_ref().map(|u| u.id.0 as i64).unwrap_or(0);

    // Blocked-user guard
    if db.is_user_blocked(user_id).await.unwrap_or(false) {
        return Ok(());
    }

    if let Some(data) = msg.web_app_data() {
        if data.data.len() > 100_000 {
            tracing::warn!("web_app_data too large from user_id={}", user_id);
            return Ok(());
        }
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&data.data) {
            if json["type"] == "order" {
                // notify admins about order from web_app_data
                tracing::info!("📱 web_app_data order: {:?}", json["order"]["id"]);
                // TODO: call notify_admins_about_order
            }
        }
    }
    Ok(())
}
