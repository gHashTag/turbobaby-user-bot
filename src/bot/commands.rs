use std::sync::Arc;
use teloxide::{
    prelude::*,
    types::{InlineKeyboardButton, InlineKeyboardMarkup, WebAppInfo},
    utils::command::BotCommands,
};
use tracing::error;

use crate::{config::Config, db::Database, locales::*, ai::{AiClient, get_random_joke_prompt, get_random_fact_prompt}};

#[derive(BotCommands, Clone)]
#[command(rename_rule = "lowercase", description = "Woody Bot commands:")]
pub enum Command {
    #[command(description = "Start")]
    Start(String),
    #[command(description = "Menu")]
    Menu,
    #[command(description = "Sets")]
    Sets,
    #[command(description = "Sommelier")]
    Sommelier,
    #[command(description = "Joke")]
    Joke,
    #[command(description = "Fact")]
    Fact,
    #[command(description = "Help")]
    Help,
    #[command(description = "Language")]
    Lang,
    #[command(description = "Engage stats (admin)")]
    Engage,
    #[command(description = "Post fact to group (admin)")]
    Factpost,
}

fn web_app_btn(text: &str, url: &str) -> InlineKeyboardButton {
    InlineKeyboardButton::web_app(text, WebAppInfo { url: url.parse().unwrap() })
}

fn callback_btn(text: &str, data: &str) -> InlineKeyboardButton {
    InlineKeyboardButton::callback(text, data)
}

fn url_btn(text: &str, url: &str) -> InlineKeyboardButton {
    InlineKeyboardButton::url(text, url.parse().unwrap())
}

pub fn build_app_url(base_url: &str, lang: &str, page: Option<&str>) -> String {
    let mut url = format!("{}?lang={}", base_url, lang);
    if let Some(p) = page { url.push_str(&format!("&page={}", p)); }
    url
}

pub async fn handle_command(
    bot: Bot,
    msg: Message,
    cmd: Command,
    db: Arc<Database>,
    config: Arc<Config>,
) -> Result<(), teloxide::RequestError> {
    let user_id = msg.from().map(|u| u.id.0 as i64).unwrap_or(0);
    let lang = db.get_user_lang(user_id).await
        .unwrap_or_else(|| map_telegram_lang(msg.from().and_then(|u| u.language_code.as_ref().map(|s| s.as_str()))));
    let locale = get_locale(&lang);
    let base = &config.web_app_url;

    match cmd {
        Command::Start(args) => {
            // Handle referral
            if args.starts_with("ref_") {
                // TODO: process referral
            }
            if args == "channel" {
                bot.send_message(msg.chat.id,
                    format!("🪵 <b>Добро пожаловать из канала!</b>\n\n{}\n\n👇 <b>Нажми кнопку ниже:</b>", locale.open_menu)
                )
                .parse_mode(teloxide::types::ParseMode::Html)
                .reply_markup(InlineKeyboardMarkup::new(vec![
                    vec![web_app_btn(&format!("🛒 {}", locale.open_menu), &build_app_url(base, &lang, None))]
                ]))
                .await?;
                return Ok(());
            }

            // Ensure profile
            // db.get_or_create_loyalty_profile(user_id).await.ok();

            let welcome = format!(
                "🪵 <b>{}</b>\n━━━━━━━━━━━━━━━━\n{}\n\n🌿 {}\n🍷 {}\n🌱 {}\n🗺️ {}\n🛍️ {}\n💰 {}\n😜 {}\n\n👇 <i>{}</i>",
                locale.welcome, locale.start_description,
                locale.welcome_feature1, locale.welcome_feature2, locale.welcome_feature3,
                locale.welcome_feature4, locale.welcome_feature5, locale.welcome_feature6,
                locale.welcome_feature7, locale.open_menu
            );

            bot.send_message(msg.chat.id, welcome)
                .parse_mode(teloxide::types::ParseMode::Html)
                .reply_markup(InlineKeyboardMarkup::new(vec![
                    vec![web_app_btn(&format!("🛒 {}", locale.open_menu), &build_app_url(base, &lang, None))],
                    vec![web_app_btn(&format!("🎁 {}", locale.view_sets), &build_app_url(base, &lang, Some("sets")))],
                    vec![web_app_btn(&format!("🍷 {}", locale.sommelier), &build_app_url(base, &lang, Some("sommelier")))],
                    vec![web_app_btn(&format!("🌱 {}", locale.garden), &build_app_url(base, &lang, Some("garden")))],
                    vec![web_app_btn(&format!("🛍️ {}", locale.accessories), &build_app_url(base, &lang, Some("accessories")))],
                    vec![web_app_btn(&format!("🗺️ {}", locale.quest), &build_app_url(base, &lang, Some("quest")))],
                    vec![web_app_btn(&format!("👤 {}", locale.profile), &build_app_url(base, &lang, Some("profile")))],
                    vec![callback_btn(&format!("😜 {}", locale.joke), "start_joke")],
                    vec![callback_btn(&format!("🧠 {}", locale.fact), "start_fact")],
                    vec![callback_btn(&format!("🌐 {}", locale.help), "show_lang")],
                ]))
                .await?;
        }

        Command::Menu => {
            let strains = db.get_strains_of_day().await.unwrap_or_default();
            if !strains.is_empty() {
                let s = &strains[0];
                let discount = s.strain_of_day_discount;
                let discounted = (s.price_per_gram * (1.0 - discount / 100.0)).round();
                let text = format!(
                    "🔥 <b>{}</b> (1/{})\n━━━━━━━━━━━━━━━━\n\n🌿 <b>{}</b>\n{}{}\n💰 <s>{} ฿/г</s> → <b>{} ฿/г</b>\n🔥 Скидка: -{}%",
                    locale.strain_of_day, strains.len(), s.name,
                    s.thc_percent.map(|t| format!("⚡ THC: {}%\n", t)).unwrap_or_default(),
                    s.category.as_ref().map(|c| format!("📁 {}\n", c)).unwrap_or_default(),
                    s.price_per_gram, discounted, discount
                );
                let mut btns: Vec<Vec<InlineKeyboardButton>> = vec![];
                if strains.len() > 1 { btns.push(vec![callback_btn(&locale.next_strain, "sotd_next_0")]); }
                btns.push(vec![web_app_btn(&format!("🛒 {}", locale.open_menu), &build_app_url(base, &lang, None))]);
                bot.send_message(msg.chat.id, text).parse_mode(teloxide::types::ParseMode::Html)
                    .reply_markup(InlineKeyboardMarkup::new(btns)).await?;
            } else {
                bot.send_message(msg.chat.id, format!("🛒 <b>{}</b>\n━━━━━━━━━━━━━━━━", locale.menu))
                    .parse_mode(teloxide::types::ParseMode::Html)
                    .reply_markup(InlineKeyboardMarkup::new(vec![
                        vec![web_app_btn(&format!("🛒 {}", locale.open_menu), &build_app_url(base, &lang, None))],
                        vec![web_app_btn(&format!("🎁 {}", locale.view_sets), &build_app_url(base, &lang, Some("sets")))],
                        vec![web_app_btn(&format!("🍷 {}", locale.sommelier), &build_app_url(base, &lang, Some("sommelier")))],
                        vec![web_app_btn(&format!("🌱 {}", locale.garden), &build_app_url(base, &lang, Some("garden")))],
                        vec![web_app_btn(&format!("🛍️ {}", locale.accessories), &build_app_url(base, &lang, Some("accessories")))],
                        vec![web_app_btn(&format!("📦 {}", locale.my_orders), &build_app_url(base, &lang, Some("orders")))],
                    ]))
                    .await?;
            }
        }

        Command::Sets => {
            bot.send_message(msg.chat.id, format!("🎁 <b>{}</b>\n━━━━━━━━━━━━━━━━\n\n{}", locale.sets_for_beginners, locale.sets_description))
                .parse_mode(teloxide::types::ParseMode::Html)
                .reply_markup(InlineKeyboardMarkup::new(vec![
                    vec![web_app_btn(&format!("🎁 {}", locale.view_sets), &build_app_url(base, &lang, Some("sets")))],
                    vec![web_app_btn(&format!("🛒 {}", locale.open_menu), &build_app_url(base, &lang, None))],
                ])).await?;
        }

        Command::Sommelier => {
            bot.send_message(msg.chat.id, format!("🍷 <b>{}</b>\n━━━━━━━━━━━━━━━━\n\n{}", locale.sommelier, locale.sommelier_description))
                .parse_mode(teloxide::types::ParseMode::Html)
                .reply_markup(InlineKeyboardMarkup::new(vec![
                    vec![web_app_btn(&format!("🍷 {}", locale.start_sommelier), &build_app_url(base, &lang, Some("sommelier")))],
                    vec![web_app_btn(&format!("🛒 {}", locale.open_menu), &build_app_url(base, &lang, None))],
                ])).await?;
        }

        Command::Joke => {
            let thinking = bot.send_message(msg.chat.id, &locale.joke_thinking).await?;
            let ai = AiClient::new(config.grok_api_key.clone(), config.glm_api_key.clone());
            let prompt = get_random_joke_prompt(&locale.joke_prompt, None);
            let joke = ai.ask_grok(&prompt, "Joker", &locale.lang_instruction).await;
            bot.delete_message(msg.chat.id, thinking.id).await.ok();
            if let Some(j) = joke {
                bot.send_message(msg.chat.id, format!("😜 {}", j))
                    .reply_markup(InlineKeyboardMarkup::new(vec![
                        vec![callback_btn(&format!("🔄 {}", locale.more_joke), "more_joke")]
                    ])).await?;
            } else {
                bot.send_message(msg.chat.id, &locale.joke_fail_fallback).await?;
            }
        }

        Command::Fact => {
            let thinking = bot.send_message(msg.chat.id, &locale.fact_thinking).await?;
            let ai = AiClient::new(config.grok_api_key.clone(), config.glm_api_key.clone());
            let prompt = get_random_fact_prompt(&locale.fact_prompt);
            let fact = ai.ask_grok(&prompt, "Professor", &locale.lang_instruction).await;
            bot.delete_message(msg.chat.id, thinking.id).await.ok();
            if let Some(f) = fact {
                bot.send_message(msg.chat.id, format!("🧠 {}", f))
                    .reply_markup(InlineKeyboardMarkup::new(vec![
                        vec![callback_btn(&format!("🔄 {}", locale.interesting_fact), "more_fact")]
                    ])).await?;
            } else {
                bot.send_message(msg.chat.id, &locale.fact_fail_fallback).await?;
            }
        }

        Command::Help => {
            bot.send_message(msg.chat.id, format!("🌐 <b>{}</b>\n━━━━━━━━━━━━━━━━\n\n{}", locale.help, locale.help_commands))
                .parse_mode(teloxide::types::ParseMode::Html)
                .reply_markup(InlineKeyboardMarkup::new(vec![
                    vec![web_app_btn(&format!("🛒 {}", locale.open_menu), &build_app_url(base, &lang, None))],
                    vec![callback_btn(&format!("🌐 {}", locale.choose_lang), "show_lang")],
                ])).await?;
        }

        Command::Lang => {
            let btns: Vec<Vec<InlineKeyboardButton>> = supported_langs().iter().map(|code| {
                let l = get_locale(code);
                vec![callback_btn(&format!("{} {}", l.flag, l.name), &format!("set_lang_{}", code))]
            }).collect();
            bot.send_message(msg.chat.id, &locale.choose_lang)
                .reply_markup(InlineKeyboardMarkup::new(btns)).await?;
        }

        Command::Engage => {
            if !config.admin_ids.contains(&user_id) { return Ok(()); }
            bot.send_message(msg.chat.id, "📬 Engage stats: (TODO)").await?;
        }

        Command::Factpost => {
            if !config.admin_ids.contains(&user_id) { return Ok(()); }
            bot.send_message(msg.chat.id, "🌿 Posting fact to group... (TODO)").await?;
        }
    }

    Ok(())
}
