// Bin-vs-lib asymmetry: see src/ai.rs's note.

use std::sync::Arc;
use teloxide::{
    prelude::*,
    types::{InlineKeyboardButton, InlineKeyboardMarkup, MaybeInaccessibleMessage},
};

use crate::bot::commands::rental_menu;
// Cycle #76: button helpers consolidated to bot/mod.rs.
// Cycle #129: AI_RATE_LIMIT replaced by `ai_rate_limit_allow` helper.
use crate::bot::{ai_rate_limit_allow, callback_btn, notify, tg_fire_and_forget};
use crate::{
    ai::{get_random_fact_prompt, get_random_joke_prompt},
    config::Config,
    db::Database,
    locales::*,
};

use crate::util::html_escape;

pub(crate) fn is_callback_data_valid(data: &str) -> bool {
    data.len() <= 200
}

/// What a button press asks the bot to do.
///
/// Extracted from `handle_callback`'s dispatch so the routing table can be
/// tested. Previously the mapping lived inside an 850-line async function that
/// needs a live Telegram connection to run at all, which meant a prefix arm
/// shadowing another — or an admin gate attached to the wrong action — could
/// only be caught in production.
///
/// Ordering matters and is preserved from the original match: the exact-match
/// arms are tried before the prefix arms, and the prefix arms in their original
/// sequence. `strip_prefix` also keeps payloads intact, so an order id that
/// itself begins with another keyword (`confirm_complete_7`) still routes to
/// `ConfirmOrder("complete_7")` rather than being re-parsed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CallbackAction {
    /// Tell a joke (initial button or "another one").
    Joke,
    /// Share a fact (initial button or "another one").
    Fact,
    /// Show the language picker.
    ShowLanguagePicker,
    /// Switch the user's locale to this language code.
    SetLanguage(String),
    /// A press on a retired strain-of-day button; see `route_callback`.
    RetiredCarouselPage,
    /// Admin-only: confirm an order.
    ConfirmOrder(String),
    /// Admin-only: mark an order completed.
    CompleteOrder(String),
    /// Admin-only: reject an order and refund any bonus.
    RejectOrder(String),
    /// Anything unrecognised — acknowledged and logged, never acted on.
    Unknown,
}

impl CallbackAction {
    /// Whether this action may only be performed by a configured admin.
    ///
    /// Stated once here rather than repeated inside each arm, so "which
    /// buttons are privileged" is a fact a test can assert instead of a
    /// property spread across three copy-pasted guards.
    pub(crate) fn requires_admin(&self) -> bool {
        matches!(
            self,
            Self::ConfirmOrder(_) | Self::CompleteOrder(_) | Self::RejectOrder(_)
        )
    }
}

/// Map raw `callback_data` to the action it requests.
pub(crate) fn route_callback(data: &str) -> CallbackAction {
    match data {
        "start_joke" | "more_joke" => return CallbackAction::Joke,
        "start_fact" | "more_fact" => return CallbackAction::Fact,
        "show_lang" => return CallbackAction::ShowLanguagePicker,
        _ => {}
    }
    if let Some(code) = data.strip_prefix("set_lang_") {
        return CallbackAction::SetLanguage(code.to_string());
    }
    // The retired strain-of-day carousel (`sotd_next_*` / `sotd_prev_*`). Its
    // buttons still sit under old messages in customers' chats. Until
    // 2026-09-25 a press paged to a strain card (a name, a THC line, a per-gram
    // price); the owner ruled that day that nothing cannabis-related may appear
    // anywhere, so a press now answers with the rental menu
    // (`commands::rental_menu`) and the page index is no longer read. Both
    // prefixes keep their own arm, so a press is still recognised rather than
    // falling through to `Unknown`, which answers and does nothing.
    if data.starts_with("sotd_next_") {
        return CallbackAction::RetiredCarouselPage;
    }
    if data.starts_with("sotd_prev_") {
        return CallbackAction::RetiredCarouselPage;
    }
    if let Some(id) = data.strip_prefix("confirm_") {
        return CallbackAction::ConfirmOrder(id.to_string());
    }
    if let Some(id) = data.strip_prefix("complete_") {
        return CallbackAction::CompleteOrder(id.to_string());
    }
    if let Some(id) = data.strip_prefix("reject_") {
        return CallbackAction::RejectOrder(id.to_string());
    }
    CallbackAction::Unknown
}

// `parse_pagination_index` stood here and went with the carousel's pages. This
// comment keeps its three lines, so every line below stays where the contracts
// under specs/ cite it.

pub(crate) fn can_confirm_order(status: &str) -> bool {
    status == "pending" || status == "confirmed"
}

pub(crate) fn should_refund_bonus(status: &str) -> bool {
    status != "rejected" && status != "completed"
}

pub(crate) async fn handle_callback(
    bot: Bot,
    q: CallbackQuery,
    db: Arc<Database>,
    config: Arc<Config>,
    ai_client: Arc<crate::ai::AiClient>,
) -> Result<(), teloxide::RequestError> {
    // A button tap is the other authentic sighting of a person. See
    // `crate::bot::remember_who` for why this is not done at the Mini App
    // boundary instead.
    crate::bot::remember_who(&db, &q.from).await;

    // The promoter's Publish button. Handled before the general routing below
    // because it is the one callback that posts publicly, and it must not fall
    // through to a menu action if the routing table ever changes.
    if let Some(key) = q
        .data
        .as_deref()
        .and_then(|d| d.strip_prefix(crate::promo::PUBLISH_CALLBACK))
    {
        let answer = crate::promo::publish(
            db.clone(),
            bot.clone(),
            config.clone(),
            key,
            q.from.id.0 as i64,
        )
        .await;
        bot.answer_callback_query(q.id)
            .text(answer)
            .show_alert(true)
            .await?;
        return Ok(());
    }

    // The promoter's mute button, beside its Publish button and for the same
    // reason: it is in the message, so stopping the drafts is one tap and not
    // a hunt through settings.
    if q.data.as_deref() == Some(crate::promo::MUTE_CALLBACK) {
        let answer = crate::promo::mute(db.clone(), q.from.id.0 as i64).await;
        bot.answer_callback_query(q.id)
            .text(answer)
            .show_alert(true)
            .await?;
        return Ok(());
    }

    let data = match q.data.as_deref() {
        Some(d) => {
            if !is_callback_data_valid(d) {
                tracing::warn!(
                    "callback_query: data too long ({}) from user_id={}",
                    d.len(),
                    q.from.id.0
                );
                bot.answer_callback_query(q.id).await?;
                return Ok(());
            }
            d.to_string()
        }
        None => {
            tracing::warn!("callback_query: empty data from user_id={}", q.from.id.0);
            bot.answer_callback_query(q.id).await?;
            return Ok(());
        }
    };

    let user_id = q.from.id.0 as i64;

    // Blocked-user guard
    if db.is_user_blocked(user_id).await.unwrap_or(false) {
        return Ok(());
    }

    tracing::debug!(
        "callback_query received: data='{}' user_id={} username={:?} message_id={:?}",
        data,
        user_id,
        q.from.username,
        q.message.as_ref().map(|m| match m {
            MaybeInaccessibleMessage::Regular(msg) => msg.id.0,
            MaybeInaccessibleMessage::Inaccessible(_) => 0,
        })
    );

    let lang = db
        .get_user_lang(user_id)
        .await
        .unwrap_or_else(|| map_telegram_lang(q.from.language_code.as_deref()));
    let locale = get_locale(&lang);
    let base = &config.web_app_url;
    let too_fast_msg = if lang == "ru" {
        "⏳ Слишком быстро! Подождите несколько секунд."
    } else {
        "⏳ Too fast! Wait a few seconds."
    };

    let action = route_callback(&data);

    // One admin gate for every privileged action, instead of the same guard
    // copy-pasted into three arms. `requires_admin` is the single statement of
    // which buttons are privileged, and it is asserted by tests — a new
    // admin-only action can no longer ship without its check.
    if action.requires_admin() && !config.admin_ids.contains(&user_id) {
        tracing::warn!(
            "callback: {:?} rejected for non-admin user_id={}",
            action,
            user_id
        );
        bot.answer_callback_query(q.id)
            .text("⛔ Admin only")
            .await?;
        return Ok(());
    }

    match action {
        CallbackAction::Joke => {
            bot.answer_callback_query(q.id).await?;
            let prompt = get_random_joke_prompt(&locale.joke_prompt, None);
            if let Some(msg) = q.message.as_ref().and_then(|m| match m {
                MaybeInaccessibleMessage::Regular(msg) => Some(msg),
                MaybeInaccessibleMessage::Inaccessible(_) => None,
            }) {
                // AI rate-limit: only check inside the AI callback arms so a
                // language-switch press (or a press on a retired carousel
                // button) does not silently consume the user's joke/fact quota.
                if !ai_rate_limit_allow(user_id) {
                    bot.edit_message_text(msg.chat.id, msg.id, too_fast_msg)
                        .await
                        .ok();
                } else {
                    bot.edit_message_text(msg.chat.id, msg.id, &locale.joke_thinking)
                        .await
                        .ok();
                    let joke = ai_client
                        .ask_grok(&prompt, "Joker", &locale.lang_instruction)
                        .await;
                    let text = joke
                        .map(|j| format!("😜 {}", html_escape(&j)))
                        .unwrap_or(locale.joke_fail_fallback.clone());
                    // Cycle #150: parse_mode=Html so html_escape'd `j`
                    // renders correctly (was literal `&lt;` etc.).
                    tg_fire_and_forget(
                        bot.edit_message_text(msg.chat.id, msg.id, &text)
                            .parse_mode(teloxide::types::ParseMode::Html)
                            .reply_markup(InlineKeyboardMarkup::new(vec![vec![callback_btn(
                                &format!("🔄 {}", locale.more_joke),
                                "more_joke",
                            )]]))
                            .send(),
                        "edit_message_text:joke_final",
                    )
                    .await;
                }
            }
        }

        CallbackAction::Fact => {
            bot.answer_callback_query(q.id).await?;
            let prompt = get_random_fact_prompt(&locale.fact_prompt);
            if let Some(msg) = q.message.as_ref().and_then(|m| match m {
                MaybeInaccessibleMessage::Regular(msg) => Some(msg),
                MaybeInaccessibleMessage::Inaccessible(_) => None,
            }) {
                if !ai_rate_limit_allow(user_id) {
                    bot.edit_message_text(msg.chat.id, msg.id, too_fast_msg)
                        .await
                        .ok();
                } else {
                    bot.edit_message_text(msg.chat.id, msg.id, &locale.fact_thinking)
                        .await
                        .ok();
                    let fact = ai_client
                        .ask_grok(&prompt, "FactMaster", &locale.lang_instruction)
                        .await;
                    let text = fact
                        .map(|f| format!("🧠 {}", html_escape(&f)))
                        .unwrap_or(locale.fact_fail_fallback.clone());
                    // Cycle #150: parse_mode=Html — same fix as more_joke.
                    tg_fire_and_forget(
                        bot.edit_message_text(msg.chat.id, msg.id, &text)
                            .parse_mode(teloxide::types::ParseMode::Html)
                            .reply_markup(InlineKeyboardMarkup::new(vec![vec![callback_btn(
                                &format!("🔄 {}", locale.interesting_fact),
                                "more_fact",
                            )]]))
                            .send(),
                        "edit_message_text:fact_final",
                    )
                    .await;
                }
            }
        }

        CallbackAction::ShowLanguagePicker => {
            bot.answer_callback_query(q.id).await?;
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
            if let Some(msg) = q.message.as_ref().and_then(|m| match m {
                MaybeInaccessibleMessage::Regular(msg) => Some(msg),
                MaybeInaccessibleMessage::Inaccessible(_) => None,
            }) {
                tg_fire_and_forget(
                    bot.send_message(msg.chat.id, &locale.choose_lang)
                        .reply_markup(InlineKeyboardMarkup::new(btns))
                        .send(),
                    "send_message:choose_lang",
                )
                .await;
            }
        }

        CallbackAction::SetLanguage(new_lang) => {
            let new_lang = new_lang.as_str();
            // Cycle #76: was `.ok()` — user toggled language and UI said
            // "changed" even when the DB write failed silently, leaving
            // the user with their old locale on next session. We still
            // don't surface the error to the user (UI flow assumes
            // success), but ops needs visibility.
            if let Err(e) = db.set_user_lang(user_id, new_lang).await {
                tracing::warn!(
                    "callbacks: set_user_lang failed for user_id={} new_lang={}: {}",
                    user_id,
                    new_lang,
                    e
                );
            }
            let new_locale = get_locale(new_lang);
            bot.answer_callback_query(q.id)
                .text(&new_locale.lang_changed)
                .await?;
            if let Some(msg) = q.message.as_ref().and_then(|m| match m {
                MaybeInaccessibleMessage::Regular(msg) => Some(msg),
                MaybeInaccessibleMessage::Inaccessible(_) => None,
            }) {
                bot.edit_message_text(
                    msg.chat.id,
                    msg.id,
                    format!("{} {}", new_locale.flag, new_locale.lang_changed),
                )
                .await
                .ok();
            }
        }

        CallbackAction::RetiredCarouselPage => {
            bot.answer_callback_query(q.id).await?;
            // The rental menu, in place of the strain card the button used to
            // page to. Written over the old card when it can be edited, so the
            // chat stops showing it; sent as a new message when Telegram no
            // longer lets the bot edit it. No table is read: the carousel's
            // reader (`Database::get_strains_of_day`) queried `strains`, which
            // 083_drop_cannabis_catalog dropped.
            let (text, markup) = rental_menu(&locale, base, &lang);
            match q.message.as_ref() {
                Some(MaybeInaccessibleMessage::Regular(msg)) => {
                    bot.edit_message_text(msg.chat.id, msg.id, text)
                        .parse_mode(teloxide::types::ParseMode::Html)
                        .reply_markup(markup)
                        .await
                        .ok();
                }
                Some(MaybeInaccessibleMessage::Inaccessible(old)) => {
                    bot.send_message(old.chat.id, text)
                        .parse_mode(teloxide::types::ParseMode::Html)
                        .reply_markup(markup)
                        .await?;
                }
                None => {}
            }
        }

        CallbackAction::ConfirmOrder(order_id) => {
            let order_id = order_id.as_str();
            // Cycle #96: SeaORM tx. FOR UPDATE row lock + conditional
            // status flip + commit. Drop = auto-rollback on early-return.
            let mut customer_telegram_id: Option<i64> = None;
            let db_ok = {
                use sea_orm::{ConnectionTrait, DbBackend, Statement, TransactionTrait};
                match db.orm.begin().await {
                    Ok(tx) => {
                        let ok = match tx
                            .query_one(Statement::from_sql_and_values(
                                DbBackend::Postgres,
                                "SELECT status, telegram_id FROM orders WHERE id = $1 FOR UPDATE",
                                [order_id.into()],
                            ))
                            .await
                        {
                            Ok(Some(row)) => {
                                customer_telegram_id =
                                    row.try_get("", "telegram_id").ok().flatten();
                                // Read the gating status loudly. `can_confirm_order("")`
                                // is already false (safe no-op), but surface a corrupt
                                // read instead of swallowing it as "not confirmable".
                                let status: String = match row.try_get::<String>("", "status") {
                                    Ok(s) => s,
                                    Err(e) => {
                                        tracing::error!(
                                            "callback: confirm corrupt status read order_id={} err={}",
                                            order_id,
                                            e
                                        );
                                        String::new()
                                    }
                                };
                                if can_confirm_order(&status) {
                                    match tx
                                        .execute(Statement::from_sql_and_values(
                                            DbBackend::Postgres,
                                            "UPDATE orders SET status = 'confirmed' WHERE id = $1",
                                            [order_id.into()],
                                        ))
                                        .await
                                    {
                                        Ok(res) => {
                                            tracing::info!(
                                                "callback: confirm order_id={} updated {} rows",
                                                order_id,
                                                res.rows_affected()
                                            );
                                            true
                                        }
                                        Err(e) => {
                                            tracing::error!(
                                                "callback: confirm order_id={} UPDATE error: {}",
                                                order_id,
                                                e
                                            );
                                            false
                                        }
                                    }
                                } else {
                                    tracing::warn!(
                                        "callback: confirm order_id={} skipped, current status={}",
                                        order_id,
                                        status
                                    );
                                    false
                                }
                            }
                            Ok(None) => {
                                tracing::warn!("callback: confirm order_id={} not found", order_id);
                                false
                            }
                            Err(e) => {
                                tracing::error!(
                                    "callback: confirm order_id={} SELECT error: {}",
                                    order_id,
                                    e
                                );
                                false
                            }
                        };
                        if ok {
                            if let Err(e) = tx.commit().await {
                                tracing::error!(
                                    "callback: confirm commit error order_id={} err={}",
                                    order_id,
                                    e
                                );
                                false
                            } else {
                                true
                            }
                        } else {
                            // tx drops → auto-rollback.
                            false
                        }
                    }
                    Err(e) => {
                        tracing::error!("callback: confirm order_id={} tx error: {}", order_id, e);
                        false
                    }
                }
            };
            if db_ok {
                bot.answer_callback_query(q.id)
                    .text(format!("✅ {}", locale.order_confirmed))
                    .await?;
                if let Some(msg) = q.message.as_ref().and_then(|m| match m {
                    MaybeInaccessibleMessage::Regular(msg) => Some(msg),
                    MaybeInaccessibleMessage::Inaccessible(_) => None,
                }) {
                    bot.edit_message_reply_markup(msg.chat.id, msg.id)
                        .reply_markup(InlineKeyboardMarkup::new(vec![vec![callback_btn(
                            &format!("📦 {}", locale.order_complete_btn),
                            &format!("complete_{}", order_id),
                        )]]))
                        .await
                        .ok();
                }
                // Cycle #79: notify the customer their order was confirmed.
                if let Some(cid) = customer_telegram_id {
                    notify::notify_order_status(
                        &bot,
                        &db,
                        &config,
                        cid,
                        order_id,
                        "confirmed",
                        None,
                    )
                    .await;
                }
            } else if let Some(msg) = q.message.as_ref().and_then(|m| match m {
                MaybeInaccessibleMessage::Regular(msg) => Some(msg),
                MaybeInaccessibleMessage::Inaccessible(_) => None,
            }) {
                bot.answer_callback_query(q.id)
                    .text("❌ DB error — check logs")
                    .await
                    .ok();
                bot.send_message(
                    msg.chat.id,
                    format!("❌ Failed to confirm order #{}", order_id),
                )
                .await
                .ok();
            }
        }

        CallbackAction::CompleteOrder(order_id) => {
            let order_id = order_id.as_str();
            tracing::info!("callback: complete order_id={}", order_id);
            bot.answer_callback_query(q.id)
                .text("📦 Completed!")
                .await?;

            // Atomically complete order, update loyalty profile and
            // recalculate tier; the referral edge is confirmed inside
            // complete_order_and_update_loyalty (cycle #171), and since
            // 2026-09-26 (R3) that confirmation pays nobody — this callsite
            // only owns the user-facing Telegram notification.
            let completion =
                match crate::db::orders::complete_order_and_update_loyalty(&db.orm, order_id).await
                {
                    Ok(Some(c)) => c,
                    Ok(None) => {
                        // No customer attribution or already completed — nothing more to do.
                        if let Some(msg) = q.message.as_ref().and_then(|m| match m {
                            teloxide::types::MaybeInaccessibleMessage::Regular(msg) => Some(msg),
                            _ => None,
                        }) {
                            bot.edit_message_reply_markup(msg.chat.id, msg.id)
                                .reply_markup(InlineKeyboardMarkup::new::<
                                    Vec<Vec<InlineKeyboardButton>>,
                                >(vec![]))
                                .await
                                .ok();
                        }
                        return Ok(());
                    }
                    Err(e) => {
                        tracing::error!("callback: complete_order_and_update_loyalty error: {}", e);
                        return Ok(());
                    }
                };

            // The referrer's "🎉 +N ฿" message stood here, sent when the
            // completion credited a referral bonus. The owner stopped that
            // bonus on 2026-09-26 (R3: «Убрать, только скидка 10%»), so there
            // is nothing to announce: the referral credit is recorded by a
            // manager per rental (`crate::db::referral_credit`).

            if let Some(msg) = q.message.as_ref().and_then(|m| match m {
                MaybeInaccessibleMessage::Regular(msg) => Some(msg),
                MaybeInaccessibleMessage::Inaccessible(_) => None,
            }) {
                bot.edit_message_reply_markup(msg.chat.id, msg.id)
                    .reply_markup(InlineKeyboardMarkup::new::<Vec<Vec<InlineKeyboardButton>>>(
                        vec![],
                    ))
                    .await
                    .ok();
                bot.send_message(
                    msg.chat.id,
                    format!(
                        "📦 Completed #{} ✅",
                        &order_id[order_id.len().saturating_sub(6)..]
                    ),
                )
                .await
                .ok();
            }
            // Cycle #79: notify the customer their order was completed.
            notify::notify_order_status(
                &bot,
                &db,
                &config,
                completion.customer_telegram_id,
                order_id,
                "completed",
                completion.cashback_credited.map(|(_, amount)| amount),
            )
            .await;
        }

        CallbackAction::RejectOrder(order_id) => {
            let _order_id = order_id.as_str();
            tracing::info!("callback: reject order_id={}", _order_id);
            bot.answer_callback_query(q.id)
                .text(format!("❌ {}", locale.order_rejected))
                .await?;
            // Cycle #96: SeaORM tx for the reject + bonus-refund path.
            // Drop = auto-rollback on every error branch.
            let mut customer_telegram_id: Option<i64> = None;
            let mut rejected = false;
            {
                use sea_orm::{ConnectionTrait, DbBackend, Statement, TransactionTrait};
                match db.orm.begin().await {
                    Ok(tx) => {
                        // Refund bonus atomically if this is the first rejection.
                        let mut refund_ok = true;
                        if let Ok(Some(r)) = tx.query_one(Statement::from_sql_and_values(
                            DbBackend::Postgres,
                            "SELECT telegram_id, bonus_used::float8 AS bonus_used, stars_used, status FROM orders WHERE id = $1 FOR UPDATE",
                            [_order_id.into()],
                        )).await {
                            // Fail loud: `should_refund_bonus("")` is TRUE, so a
                            // silent `.unwrap_or_default()` on a corrupt status read
                            // would fire a bonus refund (and flip to rejected) on an
                            // UNKNOWN order state — risking a double refund on an
                            // already-rejected order. On a read error, refuse: don't
                            // refund and don't flip (refund_ok=false aborts the tx).
                            let current_status: String = match r.try_get::<String>("", "status") {
                                Ok(s) => s,
                                Err(e) => {
                                    tracing::error!(
                                        "callback: reject corrupt status read order_id={} err={}",
                                        _order_id,
                                        e
                                    );
                                    refund_ok = false;
                                    String::new()
                                }
                            };
                            customer_telegram_id = r.try_get("", "telegram_id").ok().flatten();
                            if refund_ok && should_refund_bonus(&current_status) {
                                let bonus_raw: f64 = r.try_get::<f64>("", "bonus_used").unwrap_or(0.0);
                                let bonus = if bonus_raw.is_finite() { bonus_raw.max(0.0) } else { 0.0 };
                                let stars_raw: i64 = r.try_get::<i64>("", "stars_used").unwrap_or(0);
                                let stars = stars_raw.max(0);
                                let tid = customer_telegram_id;
                                if bonus > 0.0 {
                                    if let Some(tid) = tid {
                                        if let Err(e) = tx.execute(Statement::from_sql_and_values(
                                            DbBackend::Postgres,
                                            "INSERT INTO loyalty_profiles (telegram_id, bonus_balance, total_spent) VALUES ($1, 0, 0) ON CONFLICT (telegram_id) DO NOTHING",
                                            [tid.into()],
                                        )).await {
                                            tracing::error!("callback: reject bonus upsert error: {}", e);
                                            refund_ok = false;
                                        }
                                        if refund_ok {
                                            if let Err(e) = tx.execute(Statement::from_sql_and_values(
                                                DbBackend::Postgres,
                                                "UPDATE loyalty_profiles SET bonus_balance = bonus_balance + $1 WHERE telegram_id = $2",
                                                [bonus.into(), tid.into()],
                                            )).await {
                                                tracing::error!("callback: reject bonus refund error: {}", e);
                                                refund_ok = false;
                                            }
                                        }
                                    }
                                }
                                if refund_ok && stars > 0 {
                                    if let Some(tid) = tid {
                                        if let Err(e) = tx.execute(Statement::from_sql_and_values(
                                            DbBackend::Postgres,
                                            "INSERT INTO user_stars (telegram_id, balance, updated_at) VALUES ($1, 0, NOW()) ON CONFLICT (telegram_id) DO NOTHING",
                                            [tid.into()],
                                        )).await {
                                            tracing::error!("callback: reject stars seed error: {}", e);
                                            refund_ok = false;
                                        }
                                        if refund_ok {
                                            if let Err(e) = tx.execute(Statement::from_sql_and_values(
                                                DbBackend::Postgres,
                                                "UPDATE user_stars SET balance = balance + $1, updated_at = NOW() WHERE telegram_id = $2",
                                                [stars.into(), tid.into()],
                                            )).await {
                                                tracing::error!("callback: reject stars refund error: {}", e);
                                                refund_ok = false;
                                            }
                                        }
                                        if refund_ok {
                                            let tx_id = uuid::Uuid::new_v4().to_string();
                                            if let Err(e) = tx.execute(Statement::from_sql_and_values(
                                                DbBackend::Postgres,
                                                "INSERT INTO stars_transactions (id, telegram_id, amount, balance_after, source, reason, related_order_id) VALUES ($1, $2, $3, COALESCE((SELECT balance FROM user_stars WHERE telegram_id = $2), 0), 'plot', 'refund', $4)",
                                                [tx_id.into(), tid.into(), stars.into(), _order_id.into()],
                                            )).await {
                                                tracing::error!("callback: reject stars transaction error: {}", e);
                                                refund_ok = false;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        if refund_ok {
                            match tx
                                .execute(Statement::from_sql_and_values(
                                    DbBackend::Postgres,
                                    "UPDATE orders SET status = 'rejected' WHERE id = $1",
                                    [_order_id.into()],
                                ))
                                .await
                            {
                                Ok(res) => {
                                    if let Err(e) = tx.commit().await {
                                        tracing::error!(
                                            "callback: reject commit error order_id={} err={}",
                                            _order_id,
                                            e
                                        );
                                    } else {
                                        rejected = true;
                                        tracing::info!(
                                            "callback: reject order_id={} updated {} rows",
                                            _order_id,
                                            res.rows_affected()
                                        );
                                    }
                                }
                                Err(e) => {
                                    // tx drops → auto-rollback.
                                    tracing::error!(
                                        "callback: reject order_id={} DB error: {}",
                                        _order_id,
                                        e
                                    );
                                }
                            }
                        } else {
                            // tx drops → auto-rollback.
                            tracing::error!("callback: reject order_id={} rolled back due to bonus refund failure", _order_id);
                        }
                    }
                    Err(e) => {
                        tracing::error!("callback: reject order_id={} tx error: {}", _order_id, e)
                    }
                }
            }
            // Cycle #79: notify the customer their order was rejected.
            if rejected {
                if let Some(tid) = customer_telegram_id {
                    notify::notify_order_status(
                        &bot, &db, &config, tid, _order_id, "rejected", None,
                    )
                    .await;
                }
            }
            if let Some(msg) = q.message.as_ref().and_then(|m| match m {
                MaybeInaccessibleMessage::Regular(msg) => Some(msg),
                MaybeInaccessibleMessage::Inaccessible(_) => None,
            }) {
                bot.edit_message_reply_markup(msg.chat.id, msg.id)
                    .reply_markup(InlineKeyboardMarkup::new::<Vec<Vec<InlineKeyboardButton>>>(
                        vec![],
                    ))
                    .await
                    .ok();
            }
        }

        CallbackAction::Unknown => {
            tracing::warn!("callback: unknown data='{}' from user_id={}", data, user_id);
            bot.answer_callback_query(q.id).await?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        can_confirm_order, is_callback_data_valid, route_callback, should_refund_bonus,
        CallbackAction,
    };

    // ---- Routing table --------------------------------------------------
    //
    // These pin the mapping that used to live inside an 850-line async
    // function requiring a live Telegram connection to exercise at all.

    #[test]
    fn every_button_the_bot_builds_routes_somewhere() {
        // The failure this prevents: a button is emitted with a payload the
        // dispatcher has no arm for, so pressing it silently does nothing.
        // Every literal here is a payload constructed somewhere in src/ —
        // see the `callback_btn` / `format!` call sites. The retired carousel's
        // `sotd_*` payloads are no longer constructed anywhere; the buttons
        // already in chats are pinned by
        // `every_retired_carousel_button_routes_to_the_rental_menu`.
        for data in [
            "start_joke",
            "start_fact",
            "show_lang",
            "set_lang_ru",
            "confirm_7f3a",
            "complete_7f3a",
            "reject_7f3a",
        ] {
            assert_ne!(
                route_callback(data),
                CallbackAction::Unknown,
                "{data} is emitted as a button but routes nowhere"
            );
        }
    }

    #[test]
    fn exact_match_buttons_route_to_their_action() {
        assert_eq!(route_callback("start_joke"), CallbackAction::Joke);
        assert_eq!(route_callback("more_joke"), CallbackAction::Joke);
        assert_eq!(route_callback("start_fact"), CallbackAction::Fact);
        assert_eq!(route_callback("more_fact"), CallbackAction::Fact);
        assert_eq!(
            route_callback("show_lang"),
            CallbackAction::ShowLanguagePicker
        );
    }

    #[test]
    fn language_code_is_carried_verbatim() {
        assert_eq!(
            route_callback("set_lang_ru"),
            CallbackAction::SetLanguage("ru".into())
        );
        assert_eq!(
            route_callback("set_lang_en"),
            CallbackAction::SetLanguage("en".into())
        );
    }

    /// The retired carousel's buttons still route somewhere: both prefixes,
    /// any index, a malformed one included, land on the one action that
    /// answers with the rental menu (owner, 2026-09-25). None of them may fall
    /// through to `Unknown`, which answers the press and does nothing, and
    /// leaves the old strain card standing in the chat.
    #[test]
    fn every_retired_carousel_button_routes_to_the_rental_menu() {
        for data in [
            "sotd_next_0",
            "sotd_next_3",
            "sotd_prev_2",
            "sotd_next_",
            "sotd_next_abc",
            "sotd_prev_-1",
            "sotd_next_99999999999999999999",
        ] {
            assert_eq!(
                route_callback(data),
                CallbackAction::RetiredCarouselPage,
                "{data} must answer with the rental menu"
            );
        }
    }

    #[test]
    fn order_actions_carry_the_order_id() {
        assert_eq!(
            route_callback("confirm_7f3a"),
            CallbackAction::ConfirmOrder("7f3a".into())
        );
        assert_eq!(
            route_callback("complete_7f3a"),
            CallbackAction::CompleteOrder("7f3a".into())
        );
        assert_eq!(
            route_callback("reject_7f3a"),
            CallbackAction::RejectOrder("7f3a".into())
        );
    }

    #[test]
    fn prefixes_do_not_shadow_one_another() {
        // `confirm_` is tried before `complete_` and `reject_`. An order id
        // that happens to start with another keyword must stay part of the id
        // rather than being re-parsed into a different action.
        assert_eq!(
            route_callback("confirm_complete_7"),
            CallbackAction::ConfirmOrder("complete_7".into())
        );
        assert_eq!(
            route_callback("confirm_reject_7"),
            CallbackAction::ConfirmOrder("reject_7".into())
        );
        assert_eq!(
            route_callback("complete_confirm_7"),
            CallbackAction::CompleteOrder("confirm_7".into())
        );
    }

    #[test]
    fn an_id_containing_underscores_survives_intact() {
        // UUID-ish and slug ids both occur in this database.
        assert_eq!(
            route_callback("confirm_7f3a-1b2c_4d5e"),
            CallbackAction::ConfirmOrder("7f3a-1b2c_4d5e".into())
        );
    }

    #[test]
    fn an_empty_payload_still_routes_to_its_action() {
        // The arms handle an empty id themselves (the DB lookup misses);
        // routing must not silently reclassify it as Unknown.
        assert_eq!(
            route_callback("confirm_"),
            CallbackAction::ConfirmOrder(String::new())
        );
        assert_eq!(
            route_callback("set_lang_"),
            CallbackAction::SetLanguage(String::new())
        );
    }

    #[test]
    fn unrecognised_data_is_unknown() {
        for data in [
            "",
            "confirm",
            "CONFIRM_7",
            "delete_everything",
            "set_language_ru",
            "sotd_",
        ] {
            assert_eq!(
                route_callback(data),
                CallbackAction::Unknown,
                "{data:?} must not route to a real action"
            );
        }
    }

    #[test]
    fn exactly_the_order_actions_are_admin_only() {
        // The three order buttons carry an admin gate; nothing else may, and
        // none of them may lose it. Previously this was three copy-pasted
        // guards with nothing asserting the set.
        for admin in [
            CallbackAction::ConfirmOrder("x".into()),
            CallbackAction::CompleteOrder("x".into()),
            CallbackAction::RejectOrder("x".into()),
        ] {
            assert!(admin.requires_admin(), "{admin:?} must be admin-only");
        }
        for public in [
            CallbackAction::Joke,
            CallbackAction::Fact,
            CallbackAction::ShowLanguagePicker,
            CallbackAction::SetLanguage("ru".into()),
            CallbackAction::RetiredCarouselPage,
            CallbackAction::Unknown,
        ] {
            assert!(
                !public.requires_admin(),
                "{public:?} must not require admin"
            );
        }
    }

    #[test]
    fn routing_never_panics_on_hostile_input() {
        // callback_data comes straight off the wire. It is length-capped by
        // `is_callback_data_valid`, but nothing else sanitises it.
        for data in [
            "\u{0}\u{0}\u{0}",
            "confirm_\u{1f600}",
            "set_lang_\u{4f60}\u{597d}",
            "sotd_next_99999999999999999999",
            &"x".repeat(200),
        ] {
            let _ = route_callback(data);
        }
    }

    #[test]
    fn test_is_callback_data_valid_ok() {
        assert!(is_callback_data_valid("confirm_abc123"));
    }

    #[test]
    fn test_is_callback_data_valid_too_long() {
        let data = "a".repeat(201);
        assert!(!is_callback_data_valid(&data));
    }

    #[test]
    fn test_is_callback_data_valid_exactly_200() {
        let data = "a".repeat(200);
        assert!(is_callback_data_valid(&data));
    }

    #[test]
    fn test_can_confirm_order_pending() {
        assert!(can_confirm_order("pending"));
    }

    #[test]
    fn test_can_confirm_order_confirmed() {
        assert!(can_confirm_order("confirmed"));
    }

    #[test]
    fn test_can_confirm_order_rejected() {
        assert!(!can_confirm_order("rejected"));
    }

    #[test]
    fn test_should_refund_bonus_pending() {
        assert!(should_refund_bonus("pending"));
    }

    #[test]
    fn test_should_refund_bonus_confirmed() {
        assert!(should_refund_bonus("confirmed"));
    }

    #[test]
    fn test_should_refund_bonus_rejected() {
        assert!(!should_refund_bonus("rejected"));
    }

    #[test]
    fn test_should_refund_bonus_completed() {
        assert!(!should_refund_bonus("completed"));
    }

    /// Hazard guard: `should_refund_bonus` returns TRUE for an empty/unknown
    /// status (it only excludes the two terminal states). That is exactly why
    /// the reject handler must read the order status FAIL-LOUD — a silent
    /// `.unwrap_or_default()` here would fire a refund (and a status flip) on an
    /// unknown order state, risking a double refund on an already-rejected order.
    #[test]
    fn should_refund_bonus_is_dangerously_true_for_unknown_status() {
        assert!(should_refund_bonus(""));
        assert!(should_refund_bonus("pending"));
        // The only safe-by-default values are the terminal ones:
        assert!(!should_refund_bonus("rejected"));
        assert!(!should_refund_bonus("completed"));
    }
}
