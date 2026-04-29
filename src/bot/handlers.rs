use std::sync::Arc;
use teloxide::{
    prelude::*,
    types::{InlineKeyboardButton, InlineKeyboardMarkup, WebAppInfo},
};
use tracing::error;

use crate::{config::Config, db::Database, locales::*, ai::{AiClient}};
use crate::bot::commands::build_app_url;

fn web_app_btn(text: &str, url: &str) -> InlineKeyboardButton {
    InlineKeyboardButton::web_app(text, WebAppInfo { url: url.parse().unwrap() })
}
fn url_btn(text: &str, url: &str) -> InlineKeyboardButton {
    InlineKeyboardButton::url(text, url.parse().unwrap())
}

pub async fn handle_text(
    bot: Bot,
    msg: Message,
    db: Arc<Database>,
    config: Arc<Config>,
) -> Result<(), teloxide::RequestError> {
    let text = match msg.text() {
        Some(t) => t.to_string(),
        None => return Ok(()),
    };

    // Skip commands
    if text.starts_with('/') { return Ok(()); }
    // Skip emoji keyboard presses
    if text.starts_with(['🎁', '🍷', '😜', '🧠', '🌐', '⚙', '🛒']) { return Ok(()); }

    let is_group = matches!(msg.chat.kind, teloxide::types::ChatKind::Public(_));
    let user_id = msg.from().map(|u| u.id.0 as i64).unwrap_or(0);

    let lang = detect_language(&text);
    let locale = get_locale(lang);

    // Mark user unblocked
    db.mark_user_unblocked(user_id).await.ok();

    let ai = AiClient::new(config.grok_api_key.clone(), config.glm_api_key.clone());
    let user = msg.from();
    let name = user
        .and_then(|u| if !u.first_name.is_empty() { Some(&u.first_name) } else { u.username.as_ref() })
        .map(|s| s.as_str())
        .unwrap_or("friend");
    let ai_response = ai.ask_grok(&text, name, &locale.lang_instruction).await;

    if let Some(response) = ai_response {
        if is_group {
            bot.send_message(msg.chat.id, response)
                .reply_markup(InlineKeyboardMarkup::new(vec![
                    vec![url_btn(&format!("🛒 {}", locale.open_menu),
                        &format!("https://t.me/{}?start=channel", config.bot_username))]
                ])).await?;
        } else {
            bot.send_message(msg.chat.id, response).await?;
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
    bot: Bot,
    msg: Message,
    db: Arc<Database>,
    config: Arc<Config>,
) -> Result<(), teloxide::RequestError> {
    if let Some(data) = msg.web_app_data() {
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
