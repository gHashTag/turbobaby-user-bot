//! Background worker that drains `notification_queue` and sends referrer-facing
//! Telegram messages.
//!
//! Loop #21: referral lifecycle pushes (friend joined, watered, ordered, milestone)
//! are decoupled from the request path. The worker polls every 30 seconds, sends
//! up to 50 queued messages per tick, and marks rows `processed_at` on success.
//! After 3 failed attempts a row is abandoned to avoid infinite retries.

use std::sync::Arc;
use teloxide::prelude::*;
use teloxide::types::{ChatId, InlineKeyboardMarkup};

use crate::bot::{miniapp_deep_link, url_btn};
use crate::config::Config;
use crate::db::Database;
use crate::locales::get_locale;

const POLL_INTERVAL_SECS: u64 = 30;
const BATCH_SIZE: usize = 50;
const MAX_ATTEMPTS: i32 = 3;

/// Spawn the notification worker loop.
pub fn spawn_notification_worker(
    orm: sea_orm::DatabaseConnection,
    bot: Arc<Bot>,
    config: Arc<Config>,
) {
    tokio::spawn(async move {
        let db = Arc::new(Database::from_conn(orm.clone()));
        let mut interval =
            tokio::time::interval(std::time::Duration::from_secs(POLL_INTERVAL_SECS));
        interval.tick().await; // discard cold-start tick
        loop {
            interval.tick().await;
            match process_batch(&orm, &bot, &config, &db, BATCH_SIZE).await {
                // silent-tick: deliberate, not an omission -- ticks every 30s (POLL_INTERVAL_SECS) = 2880 lines/day, and Ok(0) here means zero messages DELIVERED, not zero due.
                // Reporting "nothing due" here would announce health where there may be
                // total delivery failure, which is worse than the silence it replaced.
                Ok(0) => {}
                Ok(n) => tracing::info!("notification worker: delivered {} message(s)", n),
                Err(e) => tracing::warn!("notification worker batch failed: {}", e),
            }
        }
    });
}

async fn process_batch(
    orm: &sea_orm::DatabaseConnection,
    bot: &Bot,
    config: &Config,
    db: &Arc<Database>,
    limit: usize,
) -> Result<usize, anyhow::Error> {
    let rows = crate::db::notifications::pending_notifications(orm, limit).await?;
    let mut delivered = 0usize;

    for row in rows {
        let telegram_id = row.telegram_id;
        let kind = row.kind.clone();
        let payload = row.payload.clone();

        if telegram_id == 0 {
            crate::db::notifications::mark_delivered(orm, row.id)
                .await
                .ok();
            continue;
        }

        let lang = db
            .get_user_lang(telegram_id)
            .await
            .unwrap_or_else(|| "en".to_string());
        let locale = get_locale(&lang);

        let text = build_message(&kind, &payload, &locale, &config.bot_username);
        // This button carried `startapp=garden` until D5 removed the screen.
        // Every notification this worker sends is about a friend — joined,
        // ordered — so `referrals` is not a substitute destination, it is the
        // one the message was always about; the garden merely hosted the panel.
        // The deep link is the reason `Target::Referrals` exists in
        // `trios::deeplink`: the bot answers `/start referrals` with a button,
        // the app parses it to `/referrals`, and the server serves that path.
        let markup = InlineKeyboardMarkup::new(vec![vec![url_btn(
            &locale.referrals_open_app,
            &miniapp_deep_link(&config.bot_username, "referrals"),
        )]]);

        match bot
            .send_message(ChatId(telegram_id), text)
            .reply_markup(markup)
            .await
        {
            Ok(_) => {
                if let Err(e) = crate::db::notifications::mark_delivered(orm, row.id).await {
                    tracing::warn!(
                        "notification worker: mark_delivered failed for {}: {}",
                        row.id,
                        e
                    );
                } else {
                    crate::metrics::friend_activity_pushed(&kind);
                    delivered += 1;
                }
            }
            Err(e) => {
                tracing::warn!(
                    "notification worker: send failed for telegram_id={} kind={}: {}",
                    telegram_id,
                    kind,
                    e
                );
                if row.attempts + 1 >= MAX_ATTEMPTS {
                    // Give up: mark as processed so we don't retry forever.
                    let _ = crate::db::notifications::mark_delivered(orm, row.id).await;
                } else {
                    let _ = crate::db::notifications::increment_attempts(orm, row.id).await;
                }
            }
        }
    }

    Ok(delivered)
}

fn build_message(
    kind: &str,
    payload: &serde_json::Value,
    locale: &crate::locales::Locale,
    bot_username: &str,
) -> String {
    let name = payload
        .get("referred_name")
        .and_then(|v| v.as_str())
        .unwrap_or("Friend");

    match kind {
        "friend_joined" => format!(
            "{}\n\n{}",
            locale.referral_friend_joined.replace("{name}", name),
            locale.referral_invite_progress_hint
        ),
        // No code writes this kind any more — `enqueue_friend_watered` was
        // deleted with the garden mechanic (D5). The arm stays because rows
        // queued before that removal may still be unsent in a deployed
        // database, and this worker is the only thing that can deliver them.
        "friend_watered" => {
            let streak = payload.get("streak").and_then(|v| v.as_i64()).unwrap_or(0);
            let body = locale
                .garden_friend_watered_legacy
                .replace("{name}", name)
                .replace("{streak}", &streak.to_string());
            format!("{}\n\n{}", body, locale.referral_invite_progress_hint)
        }
        "friend_ordered" => {
            let bonus = payload.get("bonus").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let body = locale
                .referral_friend_ordered
                .replace("{name}", name)
                .replace("{bonus}", &format!("{:.0}", bonus));
            format!("{}\n\n{}", body, locale.referral_invite_progress_hint)
        }
        "milestone" => {
            let milestone = payload
                .get("milestone")
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            let bonus = payload
                .get("bonus_amount")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);
            locale
                .referral_milestone_bonus
                .replace("{milestone}", &milestone.to_string())
                .replace("{bonus}", &format!("{:.0}", bonus))
                .replace("{bot}", bot_username)
        }
        _ => format!("{}: {}", locale.referral_bonus, kind),
    }
}
