// Bin-vs-lib asymmetry: see src/ai.rs's note.

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
// snake_case so `PromoOn` parses as `/promo_on` — the name the mute button
// has been promising since it shipped. Every other variant here is a single
// lowercase word, so for them the rule changes nothing.
#[command(rename_rule = "snake_case", description = "TurboBaby commands:")]
pub(crate) enum Command {
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
    #[command(description = "Recent app errors (admin)")]
    Errors,
    #[command(description = "Promo sales report (admin)")]
    Promo,
    #[command(description = "Turn promo messages back on")]
    PromoOn,
}

use crate::util::html_escape;

pub(crate) fn build_app_url(base_url: &str, lang: &str, page: Option<&str>) -> String {
    build_app_url_with_start(base_url, lang, page, None)
}

/// Same as [`build_app_url`], plus an optional `startapp` payload.
///
/// Used when the bot answers a `t.me/<bot>?start=<payload>` deep link: the
/// Mini App reads `startapp` from its own URL and opens the shared card
/// instead of the home screen.
pub(crate) fn build_app_url_with_start(
    base_url: &str,
    lang: &str,
    page: Option<&str>,
    start_param: Option<&str>,
) -> String {
    // Split the base URL at the fragment first, then the query string, so we
    // can insert the page path component BEFORE them. Appending the page after
    // `?cache=180` produced URLs like `/?cache=180/sets&lang=ru&v=4`, which
    // Telegram parsed as the root path with a malformed query and silently
    // opened the home screen instead of Sets/Sommelier/etc.
    let fragment_start = base_url.find('#');
    let before_fragment = if let Some(pos) = fragment_start {
        &base_url[..pos]
    } else {
        base_url
    };
    let fragment = fragment_start.map(|pos| &base_url[pos..]);

    let query_start = before_fragment.find('?');
    let path_part = &before_fragment[..query_start.unwrap_or(before_fragment.len())];
    let query = query_start.map(|pos| &before_fragment[pos..]);

    let path_part = path_part.trim_end_matches('/');
    let path = match page {
        Some(p) => format!("{}/{}", path_part, p),
        None => path_part.to_string(),
    };

    let lang_params = match start_param {
        Some(sp) => format!("lang={}&v=4&startapp={}", lang, urlencoding::encode(sp)),
        None => format!("lang={}&v=4", lang),
    };
    match (query, fragment) {
        (Some(q), Some(f)) => format!("{}{}&{}{}", path, q, lang_params, f),
        (Some(q), None) => format!("{}{}&{}", path, q, lang_params),
        (None, Some(f)) => format!("{}?{}{}", path, lang_params, f),
        (None, None) => format!("{}?{}", path, lang_params),
    }
}

fn build_admin_url(base_url: &str) -> String {
    // Insert `/admin` as the path component. If the configured URL carries a
    // cache-bust query (e.g. ?cache=180), append `/admin` BEFORE the query so
    // the path is correct and Telegram's WebApp fragment (#tgWebAppData=...)
    // remains intact. Also strip any trailing slash on the path part to avoid
    // double slashes.
    let query_or_fragment_start = base_url.find('?').or_else(|| base_url.find('#'));
    if let Some(pos) = query_or_fragment_start {
        let path_part = base_url[..pos].trim_end_matches('/');
        format!("{}/admin{}", path_part, &base_url[pos..])
    } else {
        format!("{}/admin", base_url.trim_end_matches('/'))
    }
}

/// Pure price calculator: apply a percentage discount and round to the nearest integer.
/// Result is never negative.
pub(crate) fn calculate_discounted_price(price_per_gram: f64, discount_percent: f64) -> f64 {
    (price_per_gram * (1.0 - discount_percent / 100.0))
        .max(0.0)
        .round()
}

pub(crate) async fn handle_command(
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

    // Here rather than inside `/start`, because it is the one place every
    // command passes through — and a referral arrives as `/start ref_<code>`,
    // so the friend whose name the panel was missing is named at the exact
    // moment they follow the link. See `crate::bot::remember_who`.
    if let Some(u) = msg.from.as_ref() {
        crate::bot::remember_who(&db, u).await;
    }

    let existing_lang = db.get_user_lang(user_id).await;
    let is_new_user = existing_lang.is_none();
    let lang = existing_lang.unwrap_or_else(|| {
        map_telegram_lang(msg.from.as_ref().and_then(|u| u.language_code.as_deref()))
    });
    let locale = get_locale(&lang);
    let base = &config.web_app_url;
    let too_fast_msg = if lang == "ru" {
        "⏳ Слишком быстро! Подождите несколько секунд."
    } else {
        "⏳ Too fast! Wait a few seconds."
    };

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
                    format!("🏍 <b>Добро пожаловать из канала!</b>\n\n{}\n\n👇 <b>Нажми кнопку ниже:</b>", locale.open_menu)
                )
                .parse_mode(teloxide::types::ParseMode::Html)
                .reply_markup(InlineKeyboardMarkup::new(vec![
                    vec![web_app_btn(&format!("🛒 {}", locale.open_menu), &build_app_url(base, &lang, None))]
                ]))
                .await?;
                return Ok(());
            }

            // Shared card / order / cart deep link: `t.me/<bot>?start=<payload>`.
            // Answer with a Mini App button whose URL carries `startapp`, so the
            // app opens the exact card instead of the home screen. See
            // `crate::bot::is_miniapp_start_payload` for why `?start=` is used.
            if crate::bot::is_miniapp_start_payload(&args) {
                let _ = ref_db::get_or_create_referral_code(&db.orm, user_id).await;
                let (title, btn) = if lang == "ru" {
                    (
                        "🔗 <b>Ссылка получена</b>\n━━━━━━━━━━━━━━━━\nНажмите кнопку ниже — откроется именно та карточка, которой с вами поделились.",
                        "👀 Открыть карточку",
                    )
                } else {
                    (
                        "🔗 <b>Shared link</b>\n━━━━━━━━━━━━━━━━\nTap the button below to open the exact card that was shared with you.",
                        "👀 Open the card",
                    )
                };
                bot.send_message(msg.chat.id, title)
                    .parse_mode(teloxide::types::ParseMode::Html)
                    .reply_markup(InlineKeyboardMarkup::new(vec![vec![web_app_btn(
                        btn,
                        &build_app_url_with_start(base, &lang, None, Some(&args)),
                    )]]))
                    .await?;
                return Ok(());
            }

            // Ensure profile + referral code exist (idempotent).
            // Cycle #169F: loyalty_profile rows without a referral_code caused
            // the WebApp profile screen to show the stale placeholder
            // "WOODY-DEMO". Eager seeding on /start closes the gap.
            let _ = ref_db::get_or_create_referral_code(&db.orm, user_id).await;

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
                "🏍 <b>{}</b>\n━━━━━━━━━━━━━━━━\n{}\n\n🏍 {}\n💰 {}\n🗺️ {}\n🛡️ {}\n📲 {}\n🎁 {}\n😜 {}\n\n👇 <i>{}</i>",
                locale.welcome, locale.start_description,
                locale.welcome_feature1, locale.welcome_feature2, locale.welcome_feature3,
                locale.welcome_feature4, locale.welcome_feature5, locale.welcome_feature6,
                locale.welcome_feature7, locale.open_menu
            );

            bot.send_message(msg.chat.id, welcome)
                .parse_mode(teloxide::types::ParseMode::Html)
                .reply_markup(InlineKeyboardMarkup::new(vec![
                    vec![web_app_btn(
                        &format!("🏍 {}", locale.open_menu),
                        &build_app_url(base, &lang, None),
                    )],
                    vec![web_app_btn(
                        &format!("📦 {}", locale.my_orders),
                        &build_app_url(base, &lang, Some("orders")),
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
            // The strain-of-day card that used to live here read the `strains`
            // table, which 083_drop_cannabis_catalog removed. Every /menu was a
            // query that could only error into the default, so the catalog menu
            // below is no longer an else-branch — it is the whole command.
            bot.send_message(
                msg.chat.id,
                format!("🏍 <b>{}</b>\n━━━━━━━━━━━━━━━━", locale.menu),
            )
                .parse_mode(teloxide::types::ParseMode::Html)
                .reply_markup(InlineKeyboardMarkup::new(vec![
                    vec![web_app_btn(
                        &format!("🏍 {}", locale.open_menu),
                        &build_app_url(base, &lang, None),
                    )],
                    vec![web_app_btn(
                        &format!("📦 {}", locale.my_orders),
                        &build_app_url(base, &lang, Some("orders")),
                    )],
                    vec![web_app_btn(
                        &format!("👤 {}", locale.profile),
                        &build_app_url(base, &lang, Some("profile")),
                    )],
                ]))
                .await?;
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
                bot.send_message(msg.chat.id, too_fast_msg).await?;
                return Ok(());
            }
            let thinking = bot.send_message(msg.chat.id, &locale.joke_thinking).await?;
            let prompt = get_random_joke_prompt(&locale.joke_prompt, None);
            let joke = ai_client
                .ask_grok(&prompt, "Joker", &locale.lang_instruction)
                .await;
            bot.delete_message(msg.chat.id, thinking.id).await.ok();
            if let Some(j) = joke {
                // Cycle #150: parse_mode=Html so the html_escape'd `j`
                // renders `&lt;` as `<` rather than literal `&lt;` text.
                bot.send_message(msg.chat.id, format!("😜 {}", html_escape(&j)))
                    .parse_mode(teloxide::types::ParseMode::Html)
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
                bot.send_message(msg.chat.id, too_fast_msg).await?;
                return Ok(());
            }
            let thinking = bot.send_message(msg.chat.id, &locale.fact_thinking).await?;
            let prompt = get_random_fact_prompt(&locale.fact_prompt);
            let fact = ai_client
                .ask_grok(&prompt, "Professor", &locale.lang_instruction)
                .await;
            bot.delete_message(msg.chat.id, thinking.id).await.ok();
            if let Some(f) = fact {
                // Cycle #150: parse_mode=Html — same fix as /joke above.
                bot.send_message(msg.chat.id, format!("🧠 {}", html_escape(&f)))
                    .parse_mode(teloxide::types::ParseMode::Html)
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
            // Cycle #next: referral codes are opaque, so fetch/create the
            // persisted code instead of deriving it from telegram_id.
            let code = ref_db::get_or_create_referral_code(&db.orm, user_id)
                .await
                .unwrap_or_else(|e| {
                    tracing::error!("get_or_create_referral_code failed for invite: {}", e);
                    // Graceful degrade: no code means no invite link this turn.
                    String::new()
                });
            if code.is_empty() {
                bot.send_message(
                    msg.chat.id,
                    "⚠️ Не удалось получить реферальный код. Попробуйте позже.",
                )
                .await?;
            } else {
                let invite_link =
                    format!("https://t.me/{}?start=ref_{}", config.bot_username, code);
                let text = format!(
                    "🎁 <b>{}</b>\n\n🔗 <code>{}</code>\n\n{}",
                    locale.referral_title, invite_link, locale.referral_share_hint,
                );
                // Share button via switch_inline_query so Telegram shows "Share" UX
                let share_btn = InlineKeyboardButton::switch_inline_query(
                    format!("📤 {}", locale.referral_share_button),
                    format!("🏍 TurboBaby — {invite_link}"),
                );
                bot.send_message(msg.chat.id, text)
                    .parse_mode(teloxide::types::ParseMode::Html)
                    .reply_markup(InlineKeyboardMarkup::new(vec![vec![share_btn]]))
                    .await?;
            }
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
            // Cycle #171: the WebApp admin screen now gates on ADMIN_PASSWORD,
            // not on Telegram initData / admin_ids.  The bot button is just a
            // convenient entry point; showing it to everyone is safe because the
            // WebApp still requires the shared admin password.
            bot.send_message(
                msg.chat.id,
                "🔧 <b>Admin Panel</b>\n━━━━━━━━━━━━━━━━\nУправление товарами:\nStrains • Gear • Tea • Sets • Acc.Sets • Tea Sets",
            )
            .parse_mode(teloxide::types::ParseMode::Html)
            .reply_markup(InlineKeyboardMarkup::new(vec![vec![web_app_btn(
                "🔧 Открыть админку",
                &build_admin_url(base),
            )]]))
            .await?;
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

        Command::Errors => {
            // Front-end failures used to be readable only with direct database
            // access, which the hosting provider redacts — so during an outage
            // the one record of what broke was unreachable. This puts it where
            // the owner already is.
            if !config.admin_ids.contains(&user_id) {
                return Ok(());
            }
            const WINDOW_HOURS: i64 = 24;
            let text = match db.recent_client_errors(WINDOW_HOURS, 50).await {
                Ok(groups) => crate::trios::health::format_health_digest(WINDOW_HOURS, &groups),
                Err(e) => {
                    tracing::error!("/errors DB error: {}", e);
                    "❌ Не удалось прочитать журнал ошибок".to_string()
                }
            };
            bot.send_message(msg.chat.id, text)
                .parse_mode(teloxide::types::ParseMode::Html)
                .await?;
        }

        Command::Promo => {
            // The sales report used to be readable only by curl against the
            // admin API, while the owner presses Publish in Telegram — the
            // answer to "did it sell" lived in a different room than the
            // question. Same 30-day window as the API default, so the two
            // never disagree.
            if !config.admin_ids.contains(&user_id) {
                return Ok(());
            }
            const DAYS: i64 = 30;
            let text = match crate::promo::report(&db, DAYS).await {
                Ok(rows) => crate::trios::promo::format_promo_digest(
                    DAYS,
                    &rows.into_iter().map(Into::into).collect::<Vec<_>>(),
                ),
                Err(e) => {
                    tracing::error!("/promo report query failed: {e}");
                    "❌ Не удалось прочитать отчёт промо".to_string()
                }
            };
            bot.send_message(msg.chat.id, text)
                .parse_mode(teloxide::types::ParseMode::Html)
                .await?;
        }

        Command::PromoOn => {
            // The mute button has answered "Включить обратно: /promo_on" since
            // it shipped, and until now that command did not exist — a promise
            // with no way to keep it. Not admin-gated: a customer who muted the
            // mailing is exactly who this is for.
            if crate::promo::unmute(&db, user_id).await {
                bot.send_message(msg.chat.id, "Промо-сообщения снова включены ✅")
                    .await?;
            } else {
                bot.send_message(msg.chat.id, "Не получилось, попробуйте ещё раз")
                    .await?;
            }
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
    use super::{
        build_admin_url, build_app_url, build_app_url_with_start, calculate_discounted_price,
    };

    #[test]
    fn test_build_app_url_basic() {
        assert_eq!(
            build_app_url("https://app.com", "en", None),
            "https://app.com?lang=en&v=4"
        );
    }

    #[test]
    fn test_build_app_url_with_page() {
        assert_eq!(
            build_app_url("https://app.com", "ru", Some("admin")),
            "https://app.com/admin?lang=ru&v=4"
        );
    }

    #[test]
    fn test_build_app_url_trailing_slash() {
        assert_eq!(
            build_app_url("https://app.com/", "th", None),
            "https://app.com?lang=th&v=4"
        );
    }

    #[test]
    fn test_build_app_url_with_query() {
        assert_eq!(
            build_app_url("https://app.com/?cache=180", "ru", Some("sets")),
            "https://app.com/sets?cache=180&lang=ru&v=4"
        );
    }

    #[test]
    fn test_build_app_url_with_fragment() {
        assert_eq!(
            build_app_url("https://app.com/#tgWebAppData=xyz", "en", Some("menu")),
            "https://app.com/menu?lang=en&v=4#tgWebAppData=xyz"
        );
    }

    #[test]
    fn test_build_app_url_with_query_and_fragment() {
        assert_eq!(
            build_app_url(
                "https://app.com/?cache=180#tgWebAppData=xyz",
                "ru",
                Some("profile")
            ),
            "https://app.com/profile?cache=180&lang=ru&v=4#tgWebAppData=xyz"
        );
    }

    #[test]
    fn test_build_app_url_with_start_param() {
        // The shared-card payload must survive into the Mini App URL so the
        // app can open the exact card instead of the home screen.
        assert_eq!(
            build_app_url_with_start("https://app.com", "ru", None, Some("p_strain_abc123")),
            "https://app.com?lang=ru&v=4&startapp=p_strain_abc123"
        );
    }

    #[test]
    fn test_build_app_url_with_start_param_keeps_fragment_last() {
        assert_eq!(
            build_app_url_with_start(
                "https://app.com/?cache=180#tgWebAppData=xyz",
                "en",
                None,
                Some("o_42")
            ),
            "https://app.com?cache=180&lang=en&v=4&startapp=o_42#tgWebAppData=xyz"
        );
    }

    #[test]
    fn test_miniapp_start_payloads_are_recognised() {
        for good in [
            "p_strain_abc",
            "p_acc_1",
            "p_set_x",
            "p_tea_t42",
            "p_event_e1",
            "o_7f3a",
            "reorder__7f3a",
            "garden__1234",
            "cart",
        ] {
            assert!(
                crate::bot::is_miniapp_start_payload(good),
                "{good} should be treated as a Mini App deep link"
            );
        }
        for bad in [
            "",
            "channel",
            "ref_WOODY123",
            "p_strain_abc def",
            "p_strain_\u{43e}\u{43f}",
        ] {
            assert!(
                !crate::bot::is_miniapp_start_payload(bad),
                "{bad:?} should NOT be treated as a Mini App deep link"
            );
        }
        // Over the 64-char Telegram limit.
        assert!(!crate::bot::is_miniapp_start_payload(&format!(
            "p_strain_{}",
            "a".repeat(64)
        )));
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
    fn test_build_admin_url_with_query() {
        assert_eq!(
            build_admin_url("https://app.com/?cache=180"),
            "https://app.com/admin?cache=180"
        );
    }

    #[test]
    fn test_build_admin_url_with_fragment() {
        assert_eq!(
            build_admin_url("https://app.com/#tgWebAppData=xyz"),
            "https://app.com/admin#tgWebAppData=xyz"
        );
    }

    #[test]
    fn test_build_admin_url_with_query_and_fragment() {
        assert_eq!(
            build_admin_url("https://app.com/?cache=180#tgWebAppData=xyz"),
            "https://app.com/admin?cache=180#tgWebAppData=xyz"
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

#[cfg(test)]
mod deep_link_button_tests {
    use super::build_app_url_with_start;

    /// The URL the bot actually puts on the Mini App button must parse.
    ///
    /// `web_app_btn` falls back to a plain button pointing at `https://t.me`
    /// when `url.parse()` fails — Telegram's own homepage, which is a tap that
    /// goes nowhere and reads exactly like "the link does not work". The
    /// fallback logs, but nothing asserted that the real production base ever
    /// reaches it, so this pins the shapes that matter: the live
    /// `WEB_APP_URL` (which carries `?cache=NNN`), a base with no query at all,
    /// and a base arriving with Telegram's own fragment already attached.
    #[test]
    fn the_button_url_for_a_shared_card_parses() {
        let payload = "p_set_fe346171-aa5b-4f88-93ed-8be0ec38aa6c";
        for base in [
            "https://woody-weed-bot-production-370f.up.railway.app/?cache=181",
            "https://woody-weed-bot-production-370f.up.railway.app/",
            "https://woody-weed-bot-production-370f.up.railway.app",
            "https://woody-weed-bot-production-370f.up.railway.app/?cache=181#tgWebAppData=xyz",
        ] {
            let built = build_app_url_with_start(base, "ru", None, Some(payload));
            let parsed = built.parse::<url::Url>();
            assert!(
                parsed.is_ok(),
                "the bot would have sent a button to https://t.me instead of the \
                 card: base {base} produced {built} ({:?})",
                parsed.err()
            );
            let parsed = parsed.expect("checked above");
            assert_eq!(parsed.scheme(), "https", "built {built}");
            assert!(
                built.contains(&format!("startapp={payload}")),
                "the payload has to survive into the button URL, or the app opens \
                 the home screen: {built}"
            );
        }
    }
}
