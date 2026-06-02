use std::sync::Arc;
use teloxide::{
    prelude::*,
    types::{InlineKeyboardButton, InlineKeyboardMarkup},
    utils::command::BotCommands,
};

// Cycle #76: button helpers consolidated to bot/mod.rs.
// Cycle #129: AI_RATE_LIMIT replaced by `ai_rate_limit_allow` helper.
use crate::bot::{ai_rate_limit_allow, callback_btn, web_app_btn};
use crate::{
    ai::{get_random_fact_prompt, get_random_joke_prompt},
    config::Config,
    db::referrals as ref_db,
    db::Database,
    locales::*,
    notify::notify_admins,
};

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
    #[command(description = "Get your referral invite link")]
    Invite,
    #[command(description = "Show referral statistics")]
    Refstats,
    #[command(description = "Admin panel (owners only)")]
    Admin,
    #[command(description = "Unblock user by telegram_id (admin)")]
    Unblock(String),
    #[command(description = "List currently blocked users (admin)")]
    Blocks,
}

use crate::util::html_escape;

pub fn build_app_url(base_url: &str, lang: &str, page: Option<&str>) -> String {
    let mut url = format!("{}?lang={}", base_url, lang);
    if let Some(p) = page {
        url.push_str(&format!("&page={}", p));
    }
    url
}

fn build_admin_url(base_url: &str) -> String {
    // Strip any trailing slash, then append /admin so the Telegram WebApp
    // opens directly on the admin route.
    let trimmed = base_url.trim_end_matches('/');
    format!("{}/admin", trimmed)
}

/// Pure price calculator: apply a percentage discount and round to the nearest integer.
/// Result is never negative.
pub(crate) fn calculate_discounted_price(price_per_gram: f64, discount_percent: f64) -> f64 {
    (price_per_gram * (1.0 - discount_percent / 100.0))
        .max(0.0)
        .round()
}

pub async fn handle_command(
    bot: Bot,
    msg: Message,
    cmd: Command,
    db: Arc<Database>,
    config: Arc<Config>,
    ai_client: Arc<crate::ai::AiClient>,
) -> Result<(), teloxide::RequestError> {
    let user_id = msg.from.as_ref().map(|u| u.id.0 as i64).unwrap_or(0);

    // Blocked-user guard
    if db.is_user_blocked(user_id).await.unwrap_or(false) {
        return Ok(());
    }

    let existing_lang = db.get_user_lang(user_id).await;
    let is_new_user = existing_lang.is_none();
    let lang = existing_lang.unwrap_or_else(|| {
        map_telegram_lang(msg.from.as_ref().and_then(|u| u.language_code.as_deref()))
    });
    let locale = get_locale(&lang);
    let base = &config.web_app_url;

    match cmd {
        Command::Start(args) => {
            // Handle referral: /start ref_<code>
            if args.starts_with("ref_") {
                let code = args.trim_start_matches("ref_");
                if code.len() > 200 {
                    tracing::warn!("referral code too long from user_id={}", user_id);
                } else {
                    // Only process if this is NOT the same user who owns the code
                    let referrer = ref_db::find_referrer_by_code(&db.orm, code)
                        .await
                        .ok()
                        .flatten();
                    if let Some(referrer_id) = referrer {
                        if referrer_id != user_id {
                            // Record pending referral event (idempotent)
                            if let Err(e) = ref_db::record_referral(
                                &db.orm,
                                referrer_id,
                                user_id,
                                code,
                                Some("telegram_start"),
                            )
                            .await
                            {
                                tracing::error!("record_referral failed: {}", e);
                            }
                        }
                    }
                }
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

            // Notify admins about new users
            if is_new_user {
                crate::metrics::user_registered();
                let first_name = msg
                    .from
                    .as_ref()
                    .map(|u| u.first_name.clone())
                    .unwrap_or_default();
                let username = msg
                    .from
                    .as_ref()
                    .and_then(|u| u.username.clone())
                    .unwrap_or_else(|| "unknown".to_string());
                let language_code = msg
                    .from
                    .as_ref()
                    .and_then(|u| u.language_code.clone())
                    .unwrap_or_else(|| "unknown".to_string());
                let notify_bot = bot.clone();
                let notify_config = config.clone();
                let notify_text = format!(
                    "\u{1F195} \u{041D}\u{043E}\u{0432}\u{044B}\u{0439} \u{043F}\u{043E}\u{043B}\u{044C}\u{0437}\u{043E}\u{0432}\u{0430}\u{0442}\u{0435}\u{043B}\u{044C}\n\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\n\u{1F464} {} (@{})\n\u{1F194} {}\n\u{1F30D} {}",
                    html_escape(&first_name), html_escape(&username), user_id, language_code
                );
                tokio::spawn(async move {
                    notify_admins(&notify_bot, &notify_config, &notify_text).await;
                });
            }

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
                    vec![web_app_btn(
                        &format!("🛒 {}", locale.open_menu),
                        &build_app_url(base, &lang, None),
                    )],
                    vec![web_app_btn(
                        &format!("🎁 {}", locale.view_sets),
                        &build_app_url(base, &lang, Some("sets")),
                    )],
                    vec![web_app_btn(
                        &format!("🍷 {}", locale.sommelier),
                        &build_app_url(base, &lang, Some("sommelier")),
                    )],
                    vec![web_app_btn(
                        &format!("🌱 {}", locale.garden),
                        &build_app_url(base, &lang, Some("garden")),
                    )],
                    vec![web_app_btn(
                        &format!("🛍️ {}", locale.accessories),
                        &build_app_url(base, &lang, Some("accessories")),
                    )],
                    vec![web_app_btn(
                        &format!("🗺️ {}", locale.quest),
                        &build_app_url(base, &lang, Some("quest")),
                    )],
                    vec![web_app_btn(
                        &format!("👤 {}", locale.profile),
                        &build_app_url(base, &lang, Some("profile")),
                    )],
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
                let discounted = calculate_discounted_price(s.price_per_gram, discount);
                let text = format!(
                    "🔥 <b>{}</b> (1/{})\n━━━━━━━━━━━━━━━━\n\n🌿 <b>{}</b>\n{}{}\n💰 <s>{} ฿/г</s> → <b>{} ฿/г</b>\n🔥 Скидка: -{}%",
                    locale.strain_of_day, strains.len(), html_escape(&s.name),
                    s.thc_percent.map(|t| format!("⚡ THC: {}%\n", t)).unwrap_or_default(),
                    s.category.as_ref().map(|c| format!("📁 {}\n", html_escape(c))).unwrap_or_default(),
                    s.price_per_gram, discounted, discount
                );
                let mut btns: Vec<Vec<InlineKeyboardButton>> = vec![];
                if strains.len() > 1 {
                    btns.push(vec![callback_btn(&locale.next_strain, "sotd_next_0")]);
                }
                btns.push(vec![web_app_btn(
                    &format!("🛒 {}", locale.open_menu),
                    &build_app_url(base, &lang, None),
                )]);
                bot.send_message(msg.chat.id, text)
                    .parse_mode(teloxide::types::ParseMode::Html)
                    .reply_markup(InlineKeyboardMarkup::new(btns))
                    .await?;
            } else {
                bot.send_message(
                    msg.chat.id,
                    format!("🛒 <b>{}</b>\n━━━━━━━━━━━━━━━━", locale.menu),
                )
                .parse_mode(teloxide::types::ParseMode::Html)
                .reply_markup(InlineKeyboardMarkup::new(vec![
                    vec![web_app_btn(
                        &format!("🛒 {}", locale.open_menu),
                        &build_app_url(base, &lang, None),
                    )],
                    vec![web_app_btn(
                        &format!("🎁 {}", locale.view_sets),
                        &build_app_url(base, &lang, Some("sets")),
                    )],
                    vec![web_app_btn(
                        &format!("🍷 {}", locale.sommelier),
                        &build_app_url(base, &lang, Some("sommelier")),
                    )],
                    vec![web_app_btn(
                        &format!("🌱 {}", locale.garden),
                        &build_app_url(base, &lang, Some("garden")),
                    )],
                    vec![web_app_btn(
                        &format!("🛍️ {}", locale.accessories),
                        &build_app_url(base, &lang, Some("accessories")),
                    )],
                    vec![web_app_btn(
                        &format!("📦 {}", locale.my_orders),
                        &build_app_url(base, &lang, Some("orders")),
                    )],
                ]))
                .await?;
            }
        }

        Command::Sets => {
            bot.send_message(
                msg.chat.id,
                format!(
                    "🎁 <b>{}</b>\n━━━━━━━━━━━━━━━━\n\n{}",
                    locale.sets_for_beginners, locale.sets_description
                ),
            )
            .parse_mode(teloxide::types::ParseMode::Html)
            .reply_markup(InlineKeyboardMarkup::new(vec![
                vec![web_app_btn(
                    &format!("🎁 {}", locale.view_sets),
                    &build_app_url(base, &lang, Some("sets")),
                )],
                vec![web_app_btn(
                    &format!("🛒 {}", locale.open_menu),
                    &build_app_url(base, &lang, None),
                )],
            ]))
            .await?;
        }

        Command::Sommelier => {
            bot.send_message(
                msg.chat.id,
                format!(
                    "🍷 <b>{}</b>\n━━━━━━━━━━━━━━━━\n\n{}",
                    locale.sommelier, locale.sommelier_description
                ),
            )
            .parse_mode(teloxide::types::ParseMode::Html)
            .reply_markup(InlineKeyboardMarkup::new(vec![
                vec![web_app_btn(
                    &format!("🍷 {}", locale.start_sommelier),
                    &build_app_url(base, &lang, Some("sommelier")),
                )],
                vec![web_app_btn(
                    &format!("🛒 {}", locale.open_menu),
                    &build_app_url(base, &lang, None),
                )],
            ]))
            .await?;
        }

        Command::Joke => {
            if !ai_rate_limit_allow(user_id) {
                return Ok(());
            }
            let thinking = bot.send_message(msg.chat.id, &locale.joke_thinking).await?;
            let prompt = get_random_joke_prompt(&locale.joke_prompt, None);
            let joke = ai_client
                .ask_grok(&prompt, "Joker", &locale.lang_instruction)
                .await;
            bot.delete_message(msg.chat.id, thinking.id).await.ok();
            if let Some(j) = joke {
                bot.send_message(msg.chat.id, format!("😜 {}", html_escape(&j)))
                    .reply_markup(InlineKeyboardMarkup::new(vec![vec![callback_btn(
                        &format!("🔄 {}", locale.more_joke),
                        "more_joke",
                    )]]))
                    .await?;
            } else {
                bot.send_message(msg.chat.id, &locale.joke_fail_fallback)
                    .await?;
            }
        }

        Command::Fact => {
            if !ai_rate_limit_allow(user_id) {
                return Ok(());
            }
            let thinking = bot.send_message(msg.chat.id, &locale.fact_thinking).await?;
            let prompt = get_random_fact_prompt(&locale.fact_prompt);
            let fact = ai_client
                .ask_grok(&prompt, "Professor", &locale.lang_instruction)
                .await;
            bot.delete_message(msg.chat.id, thinking.id).await.ok();
            if let Some(f) = fact {
                bot.send_message(msg.chat.id, format!("🧠 {}", html_escape(&f)))
                    .reply_markup(InlineKeyboardMarkup::new(vec![vec![callback_btn(
                        &format!("🔄 {}", locale.interesting_fact),
                        "more_fact",
                    )]]))
                    .await?;
            } else {
                bot.send_message(msg.chat.id, &locale.fact_fail_fallback)
                    .await?;
            }
        }

        Command::Help => {
            bot.send_message(
                msg.chat.id,
                format!(
                    "🌐 <b>{}</b>\n━━━━━━━━━━━━━━━━\n\n{}",
                    locale.help, locale.help_commands
                ),
            )
            .parse_mode(teloxide::types::ParseMode::Html)
            .reply_markup(InlineKeyboardMarkup::new(vec![
                vec![web_app_btn(
                    &format!("🛒 {}", locale.open_menu),
                    &build_app_url(base, &lang, None),
                )],
                vec![callback_btn(
                    &format!("🌐 {}", locale.choose_lang),
                    "show_lang",
                )],
            ]))
            .await?;
        }

        Command::Lang => {
            let btns: Vec<Vec<InlineKeyboardButton>> = supported_langs()
                .iter()
                .map(|code| {
                    let l = get_locale(code);
                    vec![callback_btn(
                        &format!("{} {}", l.flag, l.name),
                        &format!("set_lang_{}", code),
                    )]
                })
                .collect();
            bot.send_message(msg.chat.id, &locale.choose_lang)
                .reply_markup(InlineKeyboardMarkup::new(btns))
                .await?;
        }

        Command::Engage => {
            // Cycle #59 added the fraud panel; cycle #63 / B added orders /
            // revenue / pending above it so admin sees both signal and load
            // in one message. Both queries run in parallel via tokio::join!
            // because they hit independent tables (no contention).
            if !config.admin_ids.contains(&user_id) {
                return Ok(());
            }
            let (order_res, fraud_res, block_res) = tokio::join!(
                crate::db::orders::order_stats_24h(&db.orm),
                crate::db::orders::fraud_stats_24h(&db.orm),
                crate::db::orders::block_stats_24h(&db.orm),
            );
            let order_block = match order_res {
                Ok(o) => crate::db::orders::format_order_stats(&o),
                Err(e) => {
                    tracing::error!("engage order_stats query failed: {}", e);
                    "<b>📦 Orders (24h)</b>\n<i>failed to query DB</i>".to_string()
                }
            };
            let fraud_block = match fraud_res {
                Ok(s) => {
                    let total = s.subtotal_mismatch + s.unknown_item + s.unavailable + s.malformed;
                    if total == 0 {
                        String::from("<b>🛡 Fraud signals (24h)</b>\n✅ <i>none</i>")
                    } else {
                        let top = s
                            .top_offender
                            .as_deref()
                            .map(|t| {
                                format!(
                                    "<code>{}</code> ({} events)",
                                    html_escape(t),
                                    s.top_offender_count
                                )
                            })
                            .unwrap_or_else(|| "—".to_string());
                        format!(
                            "<b>🛡 Fraud signals (24h)</b>\n\
                             🚨 Subtotal mismatch: <b>{}</b>\n\
                             🚨 Unknown item: <b>{}</b>\n\
                             ⚠️ Unavailable item: <b>{}</b>\n\
                             ⚠️ Malformed payload: <b>{}</b>\n\
                             👤 Top offender: {}",
                            s.subtotal_mismatch, s.unknown_item, s.unavailable, s.malformed, top,
                        )
                    }
                }
                Err(e) => {
                    tracing::error!("engage fraud_stats query failed: {}", e);
                    "<b>🛡 Fraud signals (24h)</b>\n<i>failed to query DB</i>".to_string()
                }
            };
            // Cycle #68: third panel — `block_history` activity. Symmetric
            // with the fraud panel: empty window renders ✅ none so admin
            // doesn't see a wall of zeros on quiet days.
            let block_block = match block_res {
                Ok(b) => crate::db::orders::format_block_stats(&b),
                Err(e) => {
                    tracing::error!("engage block_stats query failed: {}", e);
                    "<b>🚫 Blocks (24h)</b>\n<i>failed to query DB</i>".to_string()
                }
            };
            let text = format!(
                "📬 <b>Engage — last 24h</b>\n━━━━━━━━━━━━━━━━\n{}\n\n{}\n\n{}",
                order_block, fraud_block, block_block,
            );
            bot.send_message(msg.chat.id, text)
                .parse_mode(teloxide::types::ParseMode::Html)
                .await?;
        }

        Command::Factpost => {
            if !config.admin_ids.contains(&user_id) {
                return Ok(());
            }
            bot.send_message(
                msg.chat.id,
                "🌿 Posting fact to group... feature not yet implemented",
            )
            .await?;
        }

        Command::Invite => {
            let code = ref_db::get_or_create_referral_code(&db.orm, user_id)
                .await
                .unwrap_or_else(|_| "error".into());
            let invite_link = format!("https://t.me/{}?start=ref_{}", config.bot_username, code);
            let text = format!(
                "🎁 <b>{}</b>\n\n🔗 <code>{}</code>\n\n{}",
                locale.referral_title, invite_link, locale.referral_share_hint,
            );
            // Share button via switch_inline_query so Telegram shows "Share" UX
            let share_btn = InlineKeyboardButton::switch_inline_query(
                format!("📤 {}", locale.referral_share_button),
                format!("🪵 Woody Weed — {invite_link}"),
            );
            bot.send_message(msg.chat.id, text)
                .parse_mode(teloxide::types::ParseMode::Html)
                .reply_markup(InlineKeyboardMarkup::new(vec![vec![share_btn]]))
                .await?;
        }

        Command::Refstats => {
            let stats = ref_db::get_referrer_stats(&db.orm, user_id)
                .await
                .unwrap_or(crate::db::referrals::ReferrerStats {
                    total_invited: 0,
                    confirmed: 0,
                    pending: 0,
                    total_bonus_earned: 0.0,
                });
            let text = format!(
                "📊 <b>{}</b>\n━━━━━━━━━━━━━━━━\n👥 {}: <b>{}</b>\n✅ {}: <b>{}</b>\n⏳ {}: <b>{}</b>\n💰 {}: <b>{:.0} ฿</b>",
                locale.referral_title,
                locale.referral_invited_count, stats.total_invited,
                locale.referral_confirmed,    stats.confirmed,
                locale.referral_pending,      stats.pending,
                locale.referral_bonus_earned, stats.total_bonus_earned,
            );
            bot.send_message(msg.chat.id, text)
                .parse_mode(teloxide::types::ParseMode::Html)
                .reply_markup(InlineKeyboardMarkup::new(vec![vec![web_app_btn(
                    &format!("📈 {}", locale.referral_leaderboard),
                    &build_app_url(base, &lang, Some("referrals")),
                )]]))
                .await?;
        }

        Command::Admin => {
            if config.admin_ids.contains(&user_id) {
                bot.send_message(
                    msg.chat.id,
                    "🔧 <b>Admin Panel</b>\n━━━━━━━━━━━━━━━━\nУправление товарами: Strains / Gear / Tea",
                )
                .parse_mode(teloxide::types::ParseMode::Html)
                .reply_markup(InlineKeyboardMarkup::new(vec![vec![web_app_btn(
                    "🔧 Открыть админку",
                    &build_admin_url(base),
                )]]))
                .await?;
            } else {
                bot.send_message(
                    msg.chat.id,
                    format!(
                        "🔒 Доступ закрыт\n\nВаш Telegram ID: <code>{}</code>\nПередайте его владельцу шопа, чтобы получить доступ.",
                        user_id
                    ),
                )
                .parse_mode(teloxide::types::ParseMode::Html)
                .await?;
            }
        }

        Command::Blocks => {
            // Cycle #62: visibility companion to /unblock. Without it admin
            // has to grep stdout for the warn! line from auto_block_for_fraud
            // to find out who is currently blocked.
            if !config.admin_ids.contains(&user_id) {
                return Ok(());
            }
            let limit = crate::db::orders::BLOCKED_USERS_LIST_LIMIT;
            let text = match crate::db::orders::query_blocked_users(&db.orm, limit).await {
                Ok(rows) => crate::db::orders::format_blocks_message(&rows, rows.len()),
                Err(e) => {
                    tracing::error!("/blocks DB error: {}", e);
                    "❌ DB error querying blocked users".to_string()
                }
            };
            bot.send_message(msg.chat.id, text)
                .parse_mode(teloxide::types::ParseMode::Html)
                .await?;
        }

        Command::Unblock(arg) => {
            // Cycle #61: counterpart to the auto-block from cycle #60.
            // Admin-only. Parses a positive telegram_id, flips
            // loyalty_profiles.is_blocked to false, replies with a
            // distinguishable message for flipped vs. was-not-blocked
            // (so admin doesn't think the command was a no-op for typos).
            if !config.admin_ids.contains(&user_id) {
                return Ok(());
            }
            let target = match crate::db::orders::parse_unblock_arg(&arg) {
                Some(tid) => tid,
                None => {
                    bot.send_message(
                        msg.chat.id,
                        "❌ Usage: /unblock &lt;telegram_id&gt; (positive integer)",
                    )
                    .parse_mode(teloxide::types::ParseMode::Html)
                    .await?;
                    return Ok(());
                }
            };
            match crate::db::orders::manual_unblock(&db.orm, target).await {
                Ok(true) => {
                    tracing::info!(
                        admin = user_id,
                        target = target,
                        "/unblock: manual override applied"
                    );
                    // Cycle #64: persist the action so the admin who actually
                    // ran it is accountable on review. Best-effort: a failure
                    // here logs a warn but doesn't block the reply.
                    if let Err(e) = crate::db::orders::record_block_history(
                        &db.orm,
                        target,
                        crate::db::orders::BLOCK_ACTION_UNBLOCK,
                        Some("admin_manual"),
                        Some(user_id),
                    )
                    .await
                    {
                        tracing::warn!("/unblock: block_history audit insert failed: {}", e);
                    }
                    bot.send_message(
                        msg.chat.id,
                        format!("✅ User <code>{}</code> unblocked", target),
                    )
                    .parse_mode(teloxide::types::ParseMode::Html)
                    .await?;
                }
                Ok(false) => {
                    bot.send_message(
                        msg.chat.id,
                        format!(
                            "ℹ️ User <code>{}</code> was not blocked (or has no loyalty profile)",
                            target
                        ),
                    )
                    .parse_mode(teloxide::types::ParseMode::Html)
                    .await?;
                }
                Err(e) => {
                    tracing::error!("/unblock DB error for target={}: {}", target, e);
                    bot.send_message(
                        msg.chat.id,
                        format!("❌ DB error while unblocking <code>{}</code>", target),
                    )
                    .parse_mode(teloxide::types::ParseMode::Html)
                    .await?;
                }
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{build_admin_url, build_app_url, calculate_discounted_price};

    #[test]
    fn test_build_app_url_basic() {
        assert_eq!(
            build_app_url("https://app.com", "en", None),
            "https://app.com?lang=en"
        );
    }

    #[test]
    fn test_build_app_url_with_page() {
        assert_eq!(
            build_app_url("https://app.com", "ru", Some("admin")),
            "https://app.com?lang=ru&page=admin"
        );
    }

    #[test]
    fn test_build_app_url_trailing_slash() {
        assert_eq!(
            build_app_url("https://app.com/", "th", None),
            "https://app.com/?lang=th"
        );
    }

    #[test]
    fn test_build_admin_url_basic() {
        assert_eq!(build_admin_url("https://app.com"), "https://app.com/admin");
    }

    #[test]
    fn test_build_admin_url_trailing_slash() {
        assert_eq!(build_admin_url("https://app.com/"), "https://app.com/admin");
    }

    #[test]
    fn test_build_admin_url_multiple_slashes() {
        assert_eq!(
            build_admin_url("https://app.com//"),
            "https://app.com/admin"
        );
    }

    #[test]
    fn test_calculate_discounted_price_basic() {
        assert_eq!(calculate_discounted_price(100.0, 10.0), 90.0);
    }

    #[test]
    fn test_calculate_discounted_price_no_discount() {
        assert_eq!(calculate_discounted_price(350.0, 0.0), 350.0);
    }

    #[test]
    fn test_calculate_discounted_price_full_discount() {
        assert_eq!(calculate_discounted_price(100.0, 100.0), 0.0);
    }

    #[test]
    fn test_calculate_discounted_price_over_discount() {
        assert_eq!(calculate_discounted_price(100.0, 150.0), 0.0);
    }

    #[test]
    fn test_calculate_discounted_price_rounds() {
        assert_eq!(calculate_discounted_price(99.0, 33.33), 66.0);
    }
}
