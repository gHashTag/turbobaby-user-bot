use std::sync::Arc;
use std::time::{Duration, Instant};
use teloxide::{
    prelude::*,
    types::{InlineKeyboardButton, InlineKeyboardMarkup, MaybeInaccessibleMessage},
};

use crate::bot::commands::{build_app_url, calculate_discounted_price};
// Cycle #76: button helpers consolidated to bot/mod.rs.
use crate::bot::{callback_btn, tg_fire_and_forget, web_app_btn, AI_COOLDOWN, AI_RATE_LIMIT};
use crate::db::referrals as ref_db;
use crate::{
    ai::{get_random_fact_prompt, get_random_joke_prompt},
    config::Config,
    db::Database,
    locales::*,
};

use crate::util::html_escape;

pub fn is_callback_data_valid(data: &str) -> bool {
    data.len() <= 200
}

pub fn parse_pagination_index(data: &str) -> Option<usize> {
    data.split('_').next_back().and_then(|s| s.parse().ok())
}

pub fn can_confirm_order(status: &str) -> bool {
    status == "pending" || status == "confirmed"
}

pub fn should_refund_bonus(status: &str) -> bool {
    status != "rejected" && status != "completed"
}

pub async fn handle_callback(
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

    // AI rate-limit for callbacks
    let mut ai_allowed = true;
    {
        let now = Instant::now();
        let mut map = AI_RATE_LIMIT.lock().await;
        map.retain(|_, last| now.saturating_duration_since(*last) < Duration::from_secs(300));
        if let Some(last) = map.get(&user_id) {
            if now.saturating_duration_since(*last) < AI_COOLDOWN {
                ai_allowed = false;
            }
        }
        if ai_allowed {
            map.insert(user_id, now);
        }
    }

    match data.as_str() {
        "start_joke" | "more_joke" => {
            bot.answer_callback_query(q.id).await?;
            let prompt = get_random_joke_prompt(&locale.joke_prompt, None);
            if let Some(msg) = q.message.as_ref().and_then(|m| match m {
                MaybeInaccessibleMessage::Regular(msg) => Some(msg),
                MaybeInaccessibleMessage::Inaccessible(_) => None,
            }) {
                if !ai_allowed {
                    bot.edit_message_text(msg.chat.id, msg.id, "⏳ Too fast! Wait a few seconds.")
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
                    tg_fire_and_forget(
                        bot.edit_message_text(msg.chat.id, msg.id, &text)
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
                if !ai_allowed {
                    bot.edit_message_text(msg.chat.id, msg.id, "⏳ Too fast! Wait a few seconds.")
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
                    tg_fire_and_forget(
                        bot.edit_message_text(msg.chat.id, msg.id, &text)
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
            let db_ok = match db.pool.get().await {
                Ok(mut client) => {
                    match client.transaction().await {
                        Ok(tx) => {
                            let ok = match tx
                                .query_opt(
                                    "SELECT status FROM orders WHERE id = $1 FOR UPDATE",
                                    &[&order_id],
                                )
                                .await
                            {
                                Ok(Some(row)) => {
                                    let status: String = row.try_get("status").unwrap_or_default();
                                    if can_confirm_order(&status) {
                                        match tx.execute("UPDATE orders SET status = 'confirmed' WHERE id = $1", &[&order_id]).await {
                                            Ok(rows) => {
                                                tracing::info!("callback: confirm order_id={} updated {} rows", order_id, rows);
                                                true
                                            }
                                            Err(e) => {
                                                tracing::error!("callback: confirm order_id={} UPDATE error: {}", order_id, e);
                                                false
                                            }
                                        }
                                    } else {
                                        tracing::warn!("callback: confirm order_id={} skipped, current status={}", order_id, status);
                                        false
                                    }
                                }
                                Ok(None) => {
                                    tracing::warn!(
                                        "callback: confirm order_id={} not found",
                                        order_id
                                    );
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
                                if let Err(e) = tx.rollback().await {
                                    tracing::error!(
                                        "callback: confirm rollback error order_id={} err={}",
                                        order_id,
                                        e
                                    );
                                }
                                false
                            }
                        }
                        Err(e) => {
                            tracing::error!(
                                "callback: confirm order_id={} tx error: {}",
                                order_id,
                                e
                            );
                            false
                        }
                    }
                }
                Err(e) => {
                    tracing::error!("callback: confirm order_id={} pool error: {}", order_id, e);
                    false
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

            // Atomically complete order, update loyalty profile, and recalculate tier
            let (is_first, customer_telegram_id) =
                match crate::db::orders::complete_order_and_update_loyalty(&db.pool, order_id).await
                {
                    Ok(Some((cid, first))) => (first, Some(cid)),
                    Ok(None) => (false, None),
                    Err(e) => {
                        tracing::error!("callback: complete_order_and_update_loyalty error: {}", e);
                        return Ok(());
                    }
                };

            // If first order, confirm referral and notify referrer
            if is_first {
                if let Some(cid) = customer_telegram_id {
                    let pool = &db.pool;
                    // Cycle #76: was `.ok()` — pool exhaustion / DB down
                    // would silently skip the bonus credit. Customer pays,
                    // referrer never gets paid, no log.
                    let db_client = match pool.get().await {
                        Ok(c) => Some(c),
                        Err(e) => {
                            tracing::warn!(
                                "callbacks: pool.get failed during referral confirm for cid={}: {}",
                                cid,
                                e
                            );
                            None
                        }
                    };
                    if let Some(db_client) = db_client {
                        // Get referral bonus amount from loyalty_config
                        let bonus_row = match db_client.query_opt(
                            "SELECT config->>'referral_bonus' AS bonus FROM loyalty_config WHERE id = 1",
                            &[],
                        ).await {
                            Ok(row) => row,
                            Err(e) => {
                                tracing::warn!(
                                    "callbacks: loyalty_config bonus read failed: {}",
                                    e
                                );
                                None
                            }
                        };
                        let bonus: f64 = bonus_row
                            .and_then(|r| r.try_get::<_, Option<String>>("bonus").ok().flatten())
                            .and_then(|s| s.parse().ok())
                            .unwrap_or(200.0);

                        if let Err(e) = ref_db::confirm_referral(pool, cid, bonus).await {
                            tracing::error!(
                                "callback: confirm_referral failed for referred_id={}: {}",
                                cid,
                                e
                            );
                        } else {
                            // Notify referrer only after successful bonus credit
                            // Cycle #76: differentiate "no row" from "query failed".
                            let event_row = match db_client.query_opt(
                                "SELECT referrer_id FROM referral_events WHERE referred_id = $1",
                                &[&cid],
                            ).await {
                                Ok(row) => row,
                                Err(e) => {
                                    tracing::warn!(
                                        "callbacks: referral_events lookup failed for cid={}: {}",
                                        cid,
                                        e
                                    );
                                    None
                                }
                            };
                            if let Some(ev) = event_row {
                                let referrer_id: i64 = ev.try_get("referrer_id").unwrap_or(0);
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
            match db.pool.get().await {
                Ok(mut client) => {
                    match client.transaction().await {
                        Ok(tx) => {
                            // Refund bonus atomically if this is the first rejection.
                            let mut refund_ok = true;
                            if let Ok(Some(r)) = tx.query_opt("SELECT telegram_id, bonus_used::float8, status FROM orders WHERE id = $1 FOR UPDATE", &[&_order_id]).await {
                                let current_status: String = r.try_get("status").unwrap_or_default();
                                if should_refund_bonus(&current_status) {
                                    let bonus_raw: f64 = r.try_get::<_, f64>("bonus_used").unwrap_or(0.0);
                                    let bonus = if bonus_raw.is_finite() { bonus_raw.max(0.0) } else { 0.0 };
                                    let tid: Option<i64> = r.try_get("telegram_id").ok().flatten();
                                    if bonus > 0.0 {
                                        if let Some(tid) = tid {
                                            if let Err(e) = tx.execute(
                                                "INSERT INTO loyalty_profiles (telegram_id, bonus_balance, total_spent) VALUES ($1, 0, 0) ON CONFLICT (telegram_id) DO NOTHING",
                                                &[&tid],
                                            ).await {
                                                tracing::error!("callback: reject bonus upsert error: {}", e);
                                                refund_ok = false;
                                            }
                                            if refund_ok {
                                                if let Err(e) = tx.execute(
                                                    "UPDATE loyalty_profiles SET bonus_balance = bonus_balance + $1 WHERE telegram_id = $2",
                                                    &[&bonus, &tid],
                                                ).await {
                                                    tracing::error!("callback: reject bonus refund error: {}", e);
                                                    refund_ok = false;
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            if refund_ok {
                                match tx
                                    .execute(
                                        "UPDATE orders SET status = 'rejected' WHERE id = $1",
                                        &[&_order_id],
                                    )
                                    .await
                                {
                                    Ok(rows) => {
                                        if let Err(e) = tx.commit().await {
                                            tracing::error!(
                                                "callback: reject commit error order_id={} err={}",
                                                _order_id,
                                                e
                                            );
                                        } else {
                                            tracing::info!(
                                                "callback: reject order_id={} updated {} rows",
                                                _order_id,
                                                rows
                                            );
                                        }
                                    }
                                    Err(e) => {
                                        if let Err(rollback_err) = tx.rollback().await {
                                            tracing::error!("callback: reject rollback error order_id={} err={}", _order_id, rollback_err);
                                        }
                                        tracing::error!(
                                            "callback: reject order_id={} DB error: {}",
                                            _order_id,
                                            e
                                        );
                                    }
                                }
                            } else {
                                if let Err(e) = tx.rollback().await {
                                    tracing::error!(
                                        "callback: reject rollback error order_id={} err={}",
                                        _order_id,
                                        e
                                    );
                                }
                                tracing::error!("callback: reject order_id={} rolled back due to bonus refund failure", _order_id);
                            }
                        }
                        Err(e) => tracing::error!(
                            "callback: reject order_id={} tx error: {}",
                            _order_id,
                            e
                        ),
                    }
                }
                Err(e) => {
                    tracing::error!("callback: reject order_id={} pool error: {}", _order_id, e)
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
}
