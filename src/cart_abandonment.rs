// Loop #12: abandoned-cart recovery.
//
// A lightweight background sweep looks for server-side carts that still have
// items but haven't been touched for a while. The customer gets one friendly
// Telegram reminder with a deep link straight back into the cart so they can
// finish checkout in one tap.

use sea_orm::{ConnectionTrait, DbBackend, Statement};
use teloxide::prelude::*;

/// Start the abandoned-cart reminder sweep.
///
/// `interval_secs` is the sleep between sweeps; the first sweep happens after
/// one interval so the server isn't slammed on boot.
pub(crate) fn spawn_cart_abandonment_reminder_loop(
    orm: sea_orm::DatabaseConnection,
    bot: std::sync::Arc<teloxide::Bot>,
    config: std::sync::Arc<crate::config::Config>,
    interval_secs: u64,
) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(interval_secs));
        interval.tick().await; // discard cold-start tick
        loop {
            interval.tick().await;
            match send_cart_abandonment_reminders(&orm, &bot, &config).await {
                Ok(0) => {}
                Ok(n) => tracing::info!("cart abandonment reminders: sent {} reminder(s)", n),
                Err(e) => tracing::warn!("cart abandonment sweep failed: {}", e),
            }
        }
    });
}

/// Send at most one reminder per cart. Carts qualify when:
/// - they have at least one item,
/// - `updated_at` is older than the abandonment threshold,
/// - no reminder has been sent for this cart yet (`reminder_count = 0`).
///
/// After a reminder we bump `reminder_count` and set `reminder_sent_at` so the
/// same cart is never spammed. A successful order clears the cart (via
/// `delete_authed` in checkout), which removes the items and naturally resets
/// the reminder state for the next purchase session.
#[cfg(not(target_arch = "wasm32"))]
async fn send_cart_abandonment_reminders(
    orm: &sea_orm::DatabaseConnection,
    bot: &teloxide::Bot,
    config: &crate::config::Config,
) -> Result<usize, sea_orm::DbErr> {
    // 10 minutes of inactivity before we consider the cart abandoned.
    // Telegram Mini App users often context-switch; this window is long
    // enough to avoid annoying fast shoppers but short enough to catch real
    // drop-offs while the intent is still warm.
    let threshold_minutes = 10i64;

    let rows = orm
        .query_all(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT c.id, c.telegram_id, COALESCE(ul.language, 'en') AS lang \
             FROM carts c \
             JOIN cart_items ci ON ci.cart_id = c.id \
             LEFT JOIN user_languages ul ON ul.telegram_id = c.telegram_id \
             WHERE c.reminder_count = 0 \
               AND c.updated_at < (now() - interval '1 minute' * $1) \
               AND c.telegram_id != 0 \
             GROUP BY c.id, c.telegram_id, ul.language \
             LIMIT 100",
            [threshold_minutes.into()],
        ))
        .await?;

    let mut sent = 0usize;
    for r in rows {
        let cart_id: String = r.try_get("", "id").unwrap_or_default();
        let telegram_id: i64 = r.try_get("", "telegram_id").unwrap_or(0);
        if telegram_id == 0 {
            continue;
        }
        let lang: String = r.try_get("", "lang").unwrap_or_else(|_| "en".into());
        let locale = crate::locales::get_locale(&lang);

        let deep_link = crate::bot::miniapp_deep_link(&config.bot_username, "cart");
        let text = locale.cart_abandonment_reminder.clone();
        let markup = teloxide::types::InlineKeyboardMarkup::new(vec![vec![
            crate::bot::url_btn(&locale.cart_open, &deep_link),
        ]]);

        if let Err(e) = bot
            .send_message(ChatId(telegram_id), text)
            .reply_markup(markup)
            .await
        {
            tracing::warn!(
                "cart abandonment reminder failed: cart_id={} telegram_id={} err={}",
                cart_id,
                telegram_id,
                e
            );
            // Still mark reminder sent so a transient send failure doesn't
            // retry every sweep and hammer Telegram.
        }

        orm.execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "UPDATE carts SET reminder_sent_at = now(), reminder_count = reminder_count + 1 WHERE id = $1",
            [cart_id.into()],
        ))
        .await?;

        crate::metrics::cart_abandonment_reminder_sent();
        sent += 1;
    }

    Ok(sent)
}
