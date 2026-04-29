use std::sync::Arc;
use teloxide::{
    prelude::*,
    types::{InlineKeyboardButton, InlineKeyboardMarkup, Message, WebAppInfo, MaybeInaccessibleMessage},
};

use crate::{config::Config, db::Database, locales::*, ai::{AiClient, get_random_joke_prompt, get_random_fact_prompt}};
use crate::bot::commands::build_app_url;

fn web_app_btn(text: &str, url: &str) -> InlineKeyboardButton {
    InlineKeyboardButton::web_app(text, WebAppInfo { url: url.parse().unwrap() })
}
fn callback_btn(text: &str, data: &str) -> InlineKeyboardButton {
    InlineKeyboardButton::callback(text, data)
}

pub async fn handle_callback(
    bot: Bot,
    q: CallbackQuery,
    db: Arc<Database>,
    config: Arc<Config>,
) -> Result<(), teloxide::RequestError> {
    let data = match q.data.as_ref().map(|s| s.as_str()) {
        Some(d) => d.to_string(),
        None => { bot.answer_callback_query(&q.id).await?; return Ok(()); }
    };

    let user_id = q.from.id.0 as i64;
    let lang = db.get_user_lang(user_id).await
        .unwrap_or_else(|| map_telegram_lang(q.from.language_code.as_ref().map(|s| s.as_str())));
    let locale = get_locale(&lang);
    let base = &config.web_app_url;
    let ai = AiClient::new(config.grok_api_key.clone(), config.glm_api_key.clone());

    match data.as_str() {
        "start_joke" | "more_joke" => {
            bot.answer_callback_query(&q.id).await?;
            let prompt = get_random_joke_prompt(&locale.joke_prompt, None);
            if let Some(msg) = q.message.as_ref().and_then(|m| match m {
    MaybeInaccessibleMessage::Regular(msg) => Some(msg),
    MaybeInaccessibleMessage::Inaccessible(_) => None,
}) {
                bot.edit_message_text(msg.chat.id, msg.id, &locale.joke_thinking).await.ok();
                let joke = ai.ask_grok(&prompt, "Joker", &locale.lang_instruction).await;
                let text = joke.map(|j| format!("😜 {}", j)).unwrap_or(locale.joke_fail_fallback.clone());
                bot.edit_message_text(msg.chat.id, msg.id, &text)
                    .reply_markup(InlineKeyboardMarkup::new(vec![
                        vec![callback_btn(&format!("🔄 {}", locale.more_joke), "more_joke")]
                    ])).await.ok();
            }
        }

        "start_fact" | "more_fact" => {
            bot.answer_callback_query(&q.id).await?;
            let prompt = get_random_fact_prompt(&locale.fact_prompt);
            if let Some(msg) = q.message.as_ref().and_then(|m| match m {
    MaybeInaccessibleMessage::Regular(msg) => Some(msg),
    MaybeInaccessibleMessage::Inaccessible(_) => None,
}) {
                bot.edit_message_text(msg.chat.id, msg.id, &locale.fact_thinking).await.ok();
                let fact = ai.ask_grok(&prompt, "Professor", &locale.lang_instruction).await;
                let text = fact.map(|f| format!("🧠 {}", f)).unwrap_or(locale.fact_fail_fallback.clone());
                bot.edit_message_text(msg.chat.id, msg.id, &text)
                    .reply_markup(InlineKeyboardMarkup::new(vec![
                        vec![callback_btn(&format!("🔄 {}", locale.interesting_fact), "more_fact")]
                    ])).await.ok();
            }
        }

        "show_lang" => {
            bot.answer_callback_query(&q.id).await?;
            let btns: Vec<Vec<InlineKeyboardButton>> = supported_langs().iter().map(|code| {
                let l = get_locale(code);
                vec![callback_btn(&format!("{} {}", l.flag, l.name), &format!("set_lang_{}", code))]
            }).collect();
            if let Some(msg) = q.message.as_ref().and_then(|m| match m {
    MaybeInaccessibleMessage::Regular(msg) => Some(msg),
    MaybeInaccessibleMessage::Inaccessible(_) => None,
}) {
                bot.send_message(msg.chat.id, &locale.choose_lang)
                    .reply_markup(InlineKeyboardMarkup::new(btns)).await.ok();
            }
        }

        d if d.starts_with("set_lang_") => {
            let new_lang = &d["set_lang_".len()..];
            db.set_user_lang(user_id, new_lang).await.ok();
            let new_locale = get_locale(new_lang);
            bot.answer_callback_query(&q.id).text(&new_locale.lang_changed).await?;
            if let Some(msg) = q.message.as_ref().and_then(|m| match m {
    MaybeInaccessibleMessage::Regular(msg) => Some(msg),
    MaybeInaccessibleMessage::Inaccessible(_) => None,
}) {
                bot.edit_message_text(msg.chat.id, msg.id,
                    format!("{} {}", new_locale.flag, new_locale.lang_changed)).await.ok();
            }
        }

        d if d.starts_with("sotd_next_") || d.starts_with("sotd_prev_") => {
            bot.answer_callback_query(&q.id).await?;
            let is_next = d.starts_with("sotd_next_");
            let current: usize = d.split('_').last().and_then(|s| s.parse().ok()).unwrap_or(0);
            let new_idx = if is_next { current + 1 } else { current.saturating_sub(1) };
            let strains = db.get_strains_of_day().await.unwrap_or_default();
            if let Some(s) = strains.get(new_idx) {
                if let Some(msg) = q.message.as_ref().and_then(|m| match m {
    MaybeInaccessibleMessage::Regular(msg) => Some(msg),
    MaybeInaccessibleMessage::Inaccessible(_) => None,
}) {
                    let discount = s.strain_of_day_discount;
                    let discounted = (s.price_per_gram * (1.0 - discount / 100.0)).round();
                    let text = format!(
                        "🔥 <b>{}</b> ({}/{})\n━━━━━━━━━━━━━━━━\n🌿 <b>{}</b>\n{}💰 <s>{} ฿/г</s> → <b>{} ฿/г</b>\n🔥 -{:.0}%",
                        locale.strain_of_day, new_idx + 1, strains.len(), s.name,
                        s.thc_percent.map(|t| format!("⚡ THC: {}%\n", t)).unwrap_or_default(),
                        s.price_per_gram, discounted, discount
                    );
                    let mut btns: Vec<Vec<InlineKeyboardButton>> = vec![];
                    let mut nav = vec![];
                    if new_idx > 0 { nav.push(callback_btn(&locale.prev_strain, &format!("sotd_prev_{}", new_idx))); }
                    if new_idx < strains.len() - 1 { nav.push(callback_btn(&locale.next_strain, &format!("sotd_next_{}", new_idx))); }
                    if !nav.is_empty() { btns.push(nav); }
                    btns.push(vec![web_app_btn(&format!("🛒 {}", locale.open_menu), &build_app_url(base, &lang, None))]);
                    bot.edit_message_text(msg.chat.id, msg.id, &text)
                        .parse_mode(teloxide::types::ParseMode::Html)
                        .reply_markup(InlineKeyboardMarkup::new(btns)).await.ok();
                }
            }
        }

        d if d.starts_with("confirm_") => {
            let order_id = &d["confirm_".len()..];
            bot.answer_callback_query(&q.id).text(&format!("✅ {}", locale.order_confirmed)).await?;
            // db.update_order_status(order_id, "confirmed").await.ok();
            if let Some(msg) = q.message.as_ref().and_then(|m| match m {
    MaybeInaccessibleMessage::Regular(msg) => Some(msg),
    MaybeInaccessibleMessage::Inaccessible(_) => None,
}) {
                bot.edit_message_reply_markup(msg.chat.id, msg.id)
                    .reply_markup(InlineKeyboardMarkup::new(vec![
                        vec![callback_btn(&format!("📦 {}", locale.order_complete_btn), &format!("complete_{}", order_id))]
                    ])).await.ok();
            }
        }

        d if d.starts_with("complete_") => {
            let order_id = &d["complete_".len()..];
            bot.answer_callback_query(&q.id).text("📦 Completed!").await?;
            // db.update_order_status(order_id, "completed").await.ok();
            // process_loyalty_on_complete(order_id, &db, &bot, &config).await.ok();
            if let Some(msg) = q.message.as_ref().and_then(|m| match m {
    MaybeInaccessibleMessage::Regular(msg) => Some(msg),
    MaybeInaccessibleMessage::Inaccessible(_) => None,
}) {
                bot.edit_message_reply_markup(msg.chat.id, msg.id)
                    .reply_markup(InlineKeyboardMarkup::new::<Vec<Vec<InlineKeyboardButton>>>(vec![])).await.ok();
                bot.send_message(msg.chat.id, format!("📦 Completed #{} ✅", &order_id[order_id.len().saturating_sub(6)..])).await.ok();
            }
        }

        d if d.starts_with("reject_") => {
            let order_id = &d["reject_".len()..];
            bot.answer_callback_query(&q.id).text(&format!("❌ {}", locale.order_rejected)).await?;
            // db.update_order_status(order_id, "rejected").await.ok();
            if let Some(msg) = q.message.as_ref().and_then(|m| match m {
    MaybeInaccessibleMessage::Regular(msg) => Some(msg),
    MaybeInaccessibleMessage::Inaccessible(_) => None,
}) {
                bot.edit_message_reply_markup(msg.chat.id, msg.id)
                    .reply_markup(InlineKeyboardMarkup::new::<Vec<Vec<InlineKeyboardButton>>>(vec![])).await.ok();
            }
        }

        _ => { bot.answer_callback_query(&q.id).await?; }
    }

    Ok(())
}
