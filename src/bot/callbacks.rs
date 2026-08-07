// Bin-vs-lib asymmetry: see src/ai.rs's note.

use std::sync::Arc;
use teloxide::{
    prelude::*,
    types::{InlineKeyboardButton, InlineKeyboardMarkup, MaybeInaccessibleMessage},
};

use crate::bot::commands::{build_app_url, calculate_discounted_price};
// Cycle #76: button helpers consolidated to bot/mod.rs.
// Cycle #129: AI_RATE_LIMIT replaced by `ai_rate_limit_allow` helper.
use crate::bot::{ai_rate_limit_allow, callback_btn, notify, tg_fire_and_forget, web_app_btn};
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

pub(crate) fn parse_pagination_index(data: &str) -> Option<usize> {
    data.split('_').next_back().and_then(|s| s.parse().ok())
}

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

    match data.as_str() {
        "start_joke" | "more_joke" => {
            bot.answer_callback_query(q.id).await?;
            let prompt = get_random_joke_prompt(&locale.joke_prompt, None);
            if let Some(msg) = q.message.as_ref().and_then(|m| match m {
                MaybeInaccessibleMessage::Regular(msg) => Some(msg),
                MaybeInaccessibleMessage::Inaccessible(_) => None,
            }) {
                // AI rate-limit: only check inside the AI callback arms so a
                // language-switch or strain-of-day pagination press does not
                // silently consume the user's joke/fact quota.
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

        "start_fact" | "more_fact" => {
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

        "show_lang" => {
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

        d if d.starts_with("set_lang_") => {
            let new_lang = &d["set_lang_".len()..];
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

        d if d.starts_with("sotd_next_") || d.starts_with("sotd_prev_") => {
            bot.answer_callback_query(q.id).await?;
            let is_next = d.starts_with("sotd_next_");
            let current = parse_pagination_index(d).unwrap_or(0);
            let new_idx = if is_next {
                current + 1
            } else {
                current.saturating_sub(1)
            };
            let strains = db.get_strains_of_day().await.unwrap_or_default();
            if let Some(s) = strains.get(new_idx) {
                if let Some(msg) = q.message.as_ref().and_then(|m| match m {
                    MaybeInaccessibleMessage::Regular(msg) => Some(msg),
                    MaybeInaccessibleMessage::Inaccessible(_) => None,
                }) {
                    let discount = s.strain_of_day_discount;
                    let discounted = calculate_discounted_price(s.price_per_gram, discount);
                    let text = format!(
                        "🔥 <b>{}</b> ({}/{})\n━━━━━━━━━━━━━━━━\n🌿 <b>{}</b>\n{}💰 <s>{} ฿/г</s> → <b>{} ฿/г</b>\n🔥 -{:.0}%",
                        locale.strain_of_day, new_idx + 1, strains.len(), html_escape(&s.name),
                        s.thc_percent.map(|t| format!("⚡ THC: {}%\n", t)).unwrap_or_default(),
                        s.price_per_gram, discounted, discount
                    );
                    let mut btns: Vec<Vec<InlineKeyboardButton>> = vec![];
                    let mut nav = vec![];
                    if new_idx > 0 {
                        nav.push(callback_btn(
                            &locale.prev_strain,
                            &format!("sotd_prev_{}", new_idx),
                        ));
                    }
                    if new_idx < strains.len() - 1 {
                        nav.push(callback_btn(
                            &locale.next_strain,
                            &format!("sotd_next_{}", new_idx),
                        ));
                    }
                    if !nav.is_empty() {
                        btns.push(nav);
                    }
                    btns.push(vec![web_app_btn(
                        &format!("🛒 {}", locale.open_menu),
                        &build_app_url(base, &lang, None),
                    )]);
                    bot.edit_message_text(msg.chat.id, msg.id, &text)
                        .parse_mode(teloxide::types::ParseMode::Html)
                        .reply_markup(InlineKeyboardMarkup::new(btns))
                        .await
                        .ok();
                }
            }
        }

        d if d.starts_with("confirm_") => {
            if !config.admin_ids.contains(&user_id) {
                tracing::warn!(
                    "callback: confirm rejected for non-admin user_id={}",
                    user_id
                );
                bot.answer_callback_query(q.id)
                    .text("⛔ Admin only")
                    .await?;
                return Ok(());
            }
            let order_id = &d["confirm_".len()..];
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

        d if d.starts_with("complete_") => {
            if !config.admin_ids.contains(&user_id) {
                tracing::warn!(
                    "callback: complete rejected for non-admin user_id={}",
                    user_id
                );
                bot.answer_callback_query(q.id)
                    .text("⛔ Admin only")
                    .await?;
                return Ok(());
            }
            let order_id = &d["complete_".len()..];
            tracing::info!("callback: complete order_id={}", order_id);
            bot.answer_callback_query(q.id)
                .text("📦 Completed!")
                .await?;

            // Atomically complete order, update loyalty profile,
            // recalculate tier, plant garden seed, and credit
            // referral bonus (cycle #171 lifted the referral credit
            // INTO complete_order_and_update_loyalty — this callsite
            // only owns the user-facing Telegram notification now).
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

            // Notify referrer if the bonus was credited inside the
            // completion function. Cycle #171: the lookup + credit
            // happens in db/orders.rs; this branch only fires when
            // the credit succeeded (Some(bonus)).
            if let Some(bonus) = completion.referral_bonus_credited {
                use sea_orm::{ConnectionTrait, DbBackend, Statement};
                // Look up referrer_id from referral_events for the
                // notification target. Cycle #76: differentiate
                // "no row" from "query failed".
                let event_row = match db
                    .orm
                    .query_one(Statement::from_sql_and_values(
                        DbBackend::Postgres,
                        "SELECT referrer_id FROM referral_events WHERE referred_id = $1",
                        [completion.customer_telegram_id.into()],
                    ))
                    .await
                {
                    Ok(row) => row,
                    Err(e) => {
                        tracing::warn!(
                            "callbacks: referral_events lookup failed for cid={}: {}",
                            completion.customer_telegram_id,
                            e
                        );
                        None
                    }
                };
                if let Some(ev) = event_row {
                    let referrer_id: i64 = ev.try_get("", "referrer_id").unwrap_or(0);
                    if referrer_id != 0 {
                        if let Err(e) = bot
                            .send_message(
                                teloxide::types::ChatId(referrer_id),
                                format!("🎉 {} +{:.0} ฿", locale.referral_bonus, bonus),
                            )
                            .await
                        {
                            tracing::warn!(
                                "referral bonus notify failed for referrer_id={}: {}",
                                referrer_id,
                                e
                            );
                        }
                    }
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

        d if d.starts_with("reject_") => {
            if !config.admin_ids.contains(&user_id) {
                tracing::warn!(
                    "callback: reject rejected for non-admin user_id={}",
                    user_id
                );
                bot.answer_callback_query(q.id)
                    .text("⛔ Admin only")
                    .await?;
                return Ok(());
            }
            let _order_id = &d["reject_".len()..];
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

        _ => {
            tracing::warn!("callback: unknown data='{}' from user_id={}", data, user_id);
            bot.answer_callback_query(q.id).await?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        can_confirm_order, is_callback_data_valid, parse_pagination_index, should_refund_bonus,
    };

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
    fn test_parse_pagination_index_next() {
        assert_eq!(parse_pagination_index("sotd_next_5"), Some(5));
    }

    #[test]
    fn test_parse_pagination_index_prev() {
        assert_eq!(parse_pagination_index("sotd_prev_3"), Some(3));
    }

    #[test]
    fn test_parse_pagination_index_invalid() {
        assert_eq!(parse_pagination_index("sotd_next_abc"), None);
    }

    #[test]
    fn test_parse_pagination_index_no_underscore() {
        assert_eq!(parse_pagination_index("sotd"), None);
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
