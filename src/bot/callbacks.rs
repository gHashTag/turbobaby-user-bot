use std::sync::Arc;
use std::time::{Duration, Instant};
use teloxide::{
    prelude::*,
    types::{InlineKeyboardButton, InlineKeyboardMarkup, WebAppInfo, MaybeInaccessibleMessage},
};

use crate::{config::Config, db::Database, locales::*, ai::{get_random_joke_prompt, get_random_fact_prompt}};
use crate::bot::commands::build_app_url;
use crate::db::referrals as ref_db;
use crate::bot::{AI_RATE_LIMIT, AI_COOLDOWN};

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
fn callback_btn(text: &str, data: &str) -> InlineKeyboardButton {
    InlineKeyboardButton::callback(text, data)
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
            if d.len() > 200 {
                tracing::warn!("callback_query: data too long ({}) from user_id={}", d.len(), q.from.id.0);
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
        data, user_id, q.from.username, q.message.as_ref().map(|m| match m {
            MaybeInaccessibleMessage::Regular(msg) => msg.id.0,
            MaybeInaccessibleMessage::Inaccessible(_) => 0,
        })
    );

    let lang = db.get_user_lang(user_id).await
        .unwrap_or_else(|| map_telegram_lang(q.from.language_code.as_deref()));
    let locale = get_locale(&lang);
    let base = &config.web_app_url;

    // AI rate-limit for callbacks
    let mut ai_allowed = true;
    {
        let now = Instant::now();
        let mut map = AI_RATE_LIMIT.lock().await;
        map.retain(|_, last| now.duration_since(*last) < Duration::from_secs(300));
        if let Some(last) = map.get(&user_id) {
            if now.duration_since(*last) < AI_COOLDOWN {
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
                    bot.edit_message_text(msg.chat.id, msg.id, "⏳ Too fast! Wait a few seconds.").await.ok();
                } else {
                    bot.edit_message_text(msg.chat.id, msg.id, &locale.joke_thinking).await.ok();
                    let joke = ai_client.ask_grok(&prompt, "Joker", &locale.lang_instruction).await;
                    let text = joke.map(|j| format!("😜 {}", j)).unwrap_or(locale.joke_fail_fallback.clone());
                    bot.edit_message_text(msg.chat.id, msg.id, &text)
                        .reply_markup(InlineKeyboardMarkup::new(vec![
                            vec![callback_btn(&format!("🔄 {}", locale.more_joke), "more_joke")]
                        ])).await.ok();
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
                    bot.edit_message_text(msg.chat.id, msg.id, "⏳ Too fast! Wait a few seconds.").await.ok();
                } else {
                    bot.edit_message_text(msg.chat.id, msg.id, &locale.fact_thinking).await.ok();
                    let fact = ai_client.ask_grok(&prompt, "FactMaster", &locale.lang_instruction).await;
                    let text = fact.map(|f| format!("🧠 {}", f)).unwrap_or(locale.fact_fail_fallback.clone());
                    bot.edit_message_text(msg.chat.id, msg.id, &text)
                        .reply_markup(InlineKeyboardMarkup::new(vec![
                            vec![callback_btn(&format!("🔄 {}", locale.interesting_fact), "more_fact")]
                        ])).await.ok();
                }
            }
        }

        "show_lang" => {
            bot.answer_callback_query(q.id).await?;
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
            bot.answer_callback_query(q.id).text(&new_locale.lang_changed).await?;
            if let Some(msg) = q.message.as_ref().and_then(|m| match m {
    MaybeInaccessibleMessage::Regular(msg) => Some(msg),
    MaybeInaccessibleMessage::Inaccessible(_) => None,
}) {
                bot.edit_message_text(msg.chat.id, msg.id,
                    format!("{} {}", new_locale.flag, new_locale.lang_changed)).await.ok();
            }
        }

        d if d.starts_with("sotd_next_") || d.starts_with("sotd_prev_") => {
            bot.answer_callback_query(q.id).await?;
            let is_next = d.starts_with("sotd_next_");
            let current: usize = d.split('_').next_back().and_then(|s| s.parse().ok()).unwrap_or(0);
            let new_idx = if is_next { current + 1 } else { current.saturating_sub(1) };
            let strains = db.get_strains_of_day().await.unwrap_or_default();
            if let Some(s) = strains.get(new_idx) {
                if let Some(msg) = q.message.as_ref().and_then(|m| match m {
    MaybeInaccessibleMessage::Regular(msg) => Some(msg),
    MaybeInaccessibleMessage::Inaccessible(_) => None,
}) {
                    let discount = s.strain_of_day_discount;
                    let discounted = (s.price_per_gram * (1.0 - discount / 100.0)).max(0.0).round();
                    let text = format!(
                        "🔥 <b>{}</b> ({}/{})\n━━━━━━━━━━━━━━━━\n🌿 <b>{}</b>\n{}💰 <s>{} ฿/г</s> → <b>{} ฿/г</b>\n🔥 -{:.0}%",
                        locale.strain_of_day, new_idx + 1, strains.len(), html_escape(&s.name),
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
            if !config.admin_ids.contains(&user_id) {
                tracing::warn!("callback: confirm rejected for non-admin user_id={}", user_id);
                bot.answer_callback_query(q.id).text("⛔ Admin only").await?;
                return Ok(());
            }
            let order_id = &d["confirm_".len()..];
            let db_ok = match db.pool.get().await {
                Ok(mut client) => {
                    match client.transaction().await {
                        Ok(tx) => {
                            let ok = match tx.query_opt("SELECT status FROM orders WHERE id = $1 FOR UPDATE", &[&order_id]).await {
                                Ok(Some(row)) => {
                                    let status: String = row.try_get("status").unwrap_or_default();
                                    if status == "pending" || status == "confirmed" {
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
                                    tracing::warn!("callback: confirm order_id={} not found", order_id);
                                    false
                                }
                                Err(e) => {
                                    tracing::error!("callback: confirm order_id={} SELECT error: {}", order_id, e);
                                    false
                                }
                            };
                            if ok {
                                if let Err(e) = tx.commit().await {
                                    tracing::error!("callback: confirm commit error order_id={} err={}", order_id, e);
                                    false
                                } else {
                                    true
                                }
                            } else {
                                if let Err(e) = tx.rollback().await {
                                    tracing::error!("callback: confirm rollback error order_id={} err={}", order_id, e);
                                }
                                false
                            }
                        }
                        Err(e) => {
                            tracing::error!("callback: confirm order_id={} tx error: {}", order_id, e);
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
                bot.answer_callback_query(q.id).text(format!("✅ {}", locale.order_confirmed)).await?;
                if let Some(msg) = q.message.as_ref().and_then(|m| match m {
    MaybeInaccessibleMessage::Regular(msg) => Some(msg),
    MaybeInaccessibleMessage::Inaccessible(_) => None,
}) {
                    bot.edit_message_reply_markup(msg.chat.id, msg.id)
                        .reply_markup(InlineKeyboardMarkup::new(vec![
                            vec![callback_btn(&format!("📦 {}", locale.order_complete_btn), &format!("complete_{}", order_id))]
                        ])).await.ok();
                }
            } else if let Some(msg) = q.message.as_ref().and_then(|m| match m {
    MaybeInaccessibleMessage::Regular(msg) => Some(msg),
    MaybeInaccessibleMessage::Inaccessible(_) => None,
}) {
                bot.answer_callback_query(q.id).text("❌ DB error — check logs").await.ok();
                bot.send_message(msg.chat.id, format!("❌ Failed to confirm order #{}", order_id)).await.ok();
            }
        }

        d if d.starts_with("complete_") => {
            if !config.admin_ids.contains(&user_id) {
                tracing::warn!("callback: complete rejected for non-admin user_id={}", user_id);
                bot.answer_callback_query(q.id).text("⛔ Admin only").await?;
                return Ok(());
            }
            let order_id = &d["complete_".len()..];
            tracing::info!("callback: complete order_id={}", order_id);
            bot.answer_callback_query(q.id).text("📦 Completed!").await?;

            // Atomically complete order, update loyalty profile, and recalculate tier
            let (is_first, customer_telegram_id) = match crate::db::orders::complete_order_and_update_loyalty(&db.pool, order_id).await {
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
                    let db_client = pool.get().await.ok();
                    if let Some(db_client) = db_client {
                        // Get referral bonus amount from loyalty_config
                        let bonus_row = db_client.query_opt(
                            "SELECT config->>'referral_bonus' AS bonus FROM loyalty_config WHERE id = 1",
                            &[],
                        ).await.ok().flatten();
                        let bonus: f64 = bonus_row
                            .and_then(|r| r.try_get::<_, Option<String>>("bonus").ok().flatten())
                            .and_then(|s| s.parse().ok())
                            .unwrap_or(200.0);

                        if let Err(e) = ref_db::confirm_referral(pool, cid, bonus).await {
                            tracing::error!("callback: confirm_referral failed for referred_id={}: {}", cid, e);
                        } else {
                            // Notify referrer only after successful bonus credit
                            let event_row = db_client.query_opt(
                                "SELECT referrer_id FROM referral_events WHERE referred_id = $1",
                                &[&cid],
                            ).await.ok().flatten();
                            if let Some(ev) = event_row {
                                let referrer_id: i64 = ev.try_get("referrer_id").unwrap_or(0);
                                if referrer_id != 0 {
                                    if let Err(e) = bot.send_message(
                                        teloxide::types::ChatId(referrer_id),
                                        format!("🎉 {} +{:.0} ฿", locale.referral_bonus, bonus),
                                    ).await {
                                        tracing::warn!("referral bonus notify failed for referrer_id={}: {}", referrer_id, e);
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
                    .reply_markup(InlineKeyboardMarkup::new::<Vec<Vec<InlineKeyboardButton>>>(vec![])).await.ok();
                bot.send_message(msg.chat.id, format!("📦 Completed #{} ✅", &order_id[order_id.len().saturating_sub(6)..])).await.ok();
            }
        }

        d if d.starts_with("reject_") => {
            if !config.admin_ids.contains(&user_id) {
                tracing::warn!("callback: reject rejected for non-admin user_id={}", user_id);
                bot.answer_callback_query(q.id).text("⛔ Admin only").await?;
                return Ok(());
            }
            let _order_id = &d["reject_".len()..];
            tracing::info!("callback: reject order_id={}", _order_id);
            bot.answer_callback_query(q.id).text(format!("❌ {}", locale.order_rejected)).await?;
            match db.pool.get().await {
                Ok(mut client) => {
                    match client.transaction().await {
                        Ok(tx) => {
                            // Refund bonus atomically if this is the first rejection.
                            if let Ok(Some(r)) = tx.query_opt("SELECT telegram_id, bonus_used::float8, status FROM orders WHERE id = $1 FOR UPDATE", &[&_order_id]).await {
                                let current_status: String = r.try_get("status").unwrap_or_default();
                                if current_status != "rejected" && current_status != "completed" {
                                    let bonus: f64 = r.try_get::<_, f64>("bonus_used").unwrap_or(0.0);
                                    let tid: Option<i64> = r.try_get("telegram_id").ok().flatten();
                                    if bonus > 0.0 {
                                        if let Some(tid) = tid {
                                            if let Err(e) = tx.execute(
                                                "INSERT INTO loyalty_profiles (telegram_id, bonus_balance, total_spent) VALUES ($1, 0, 0) ON CONFLICT (telegram_id) DO NOTHING",
                                                &[&tid],
                                            ).await {
                                                tracing::error!("callback: reject bonus upsert error: {}", e);
                                            }
                                            if let Err(e) = tx.execute(
                                                "UPDATE loyalty_profiles SET bonus_balance = bonus_balance + $1 WHERE telegram_id = $2",
                                                &[&bonus, &tid],
                                            ).await {
                                                tracing::error!("callback: reject bonus refund error: {}", e);
                                            }
                                        }
                                    }
                                }
                            }
                            match tx.execute("UPDATE orders SET status = 'rejected' WHERE id = $1", &[&_order_id]).await {
                                Ok(rows) => {
                                    if let Err(e) = tx.commit().await {
                                        tracing::error!("callback: reject commit error order_id={} err={}", _order_id, e);
                                    } else {
                                        tracing::info!("callback: reject order_id={} updated {} rows", _order_id, rows);
                                    }
                                }
                                Err(e) => {
                                    if let Err(rollback_err) = tx.rollback().await {
                                        tracing::error!("callback: reject rollback error order_id={} err={}", _order_id, rollback_err);
                                    }
                                    tracing::error!("callback: reject order_id={} DB error: {}", _order_id, e);
                                }
                            }
                        }
                        Err(e) => tracing::error!("callback: reject order_id={} tx error: {}", _order_id, e),
                    }
                }
                Err(e) => tracing::error!("callback: reject order_id={} pool error: {}", _order_id, e),
            }
            if let Some(msg) = q.message.as_ref().and_then(|m| match m {
    MaybeInaccessibleMessage::Regular(msg) => Some(msg),
    MaybeInaccessibleMessage::Inaccessible(_) => None,
}) {
                bot.edit_message_reply_markup(msg.chat.id, msg.id)
                    .reply_markup(InlineKeyboardMarkup::new::<Vec<Vec<InlineKeyboardButton>>>(vec![])).await.ok();
            }
        }

        _ => {
            tracing::warn!("callback: unknown data='{}' from user_id={}", data, user_id);
            bot.answer_callback_query(q.id).await?;
        }
    }

    Ok(())
}
