// Customer-facing Telegram notifications for order lifecycle events.
//
// Admin callbacks in `callbacks.rs` update the order status in the DB and
// edit their own admin message; this module owns the separate message that
// goes to the customer's private chat so they know their order moved.

use std::sync::Arc;
use teloxide::prelude::*;
use teloxide::types::InlineKeyboardMarkup;

use crate::bot::{miniapp_deep_link, url_btn};
use crate::config::Config;
use crate::db::Database;
use crate::locales::get_locale;

// `notify_garden_reminder` and `notify_garden_reward_expiry` stood here: the
// water/harvest nudge and the reward-expiry nudge. Their only callers were the
// two reminder loops in main.rs, which went with the garden mechanic (D5), so
// both were dead code — and CI runs clippy with `-D warnings`, so dead code
// here is a failed build, not a cosmetic complaint.
//
// The locale strings they read (`garden_water_reminder`, `garden_harvest_ready`,
// `garden_reward_expiry`) are deliberately still in src/locales.rs: the
// notification worker renders queued rows by `kind`, and a deployed database
// can still hold unsent garden rows. See DECISIONS.md D18.

/// Cycle #79: notify the customer that their order status changed.
///
/// `status` must be one of: "confirmed", "completed", "rejected".
/// The message is localised via the customer's stored language (falls back to
/// Telegram client language, then English). A button opens the Mini App so the
/// customer can track the order.
pub(crate) async fn notify_order_status(
    bot: &Bot,
    db: &Arc<Database>,
    config: &Arc<Config>,
    customer_telegram_id: i64,
    order_id: &str,
    status: &str,
    cashback_amount: Option<f64>,
) {
    if customer_telegram_id == 0 {
        return;
    }

    let lang = db
        .get_user_lang(customer_telegram_id)
        .await
        .unwrap_or_else(|| "en".to_string());
    let locale = get_locale(&lang);

    let status_text = match status {
        "confirmed" => &locale.order_status_confirmed,
        "preparing" => &locale.order_status_preparing,
        "ready" => &locale.order_status_ready,
        "out_for_delivery" => &locale.order_status_out_for_delivery,
        "completed" | "delivered" => &locale.order_status_completed,
        "rejected" => &locale.order_status_rejected,
        "cancelled" => &locale.order_status_cancelled,
        _ => "Order status updated",
    };

    let short_id = &order_id[order_id.len().saturating_sub(6)..];
    let cashback_line = cashback_amount
        .filter(|a| *a > 0.01)
        .map(|a| format!("\n\n💸 {} +{:.0} ฿", locale.cashback_earned, a))
        .unwrap_or_default();
    let text = format!(
        "{} #{}\n\n{}{}",
        status_text, short_id, locale.order_open_app, cashback_line
    );

    // Cycle #80: deep-link straight into the order detail screen.
    let deep_link = miniapp_deep_link(
        &config.bot_username,
        &crate::bot::order_start_param(order_id),
    );
    let markup = InlineKeyboardMarkup::new(vec![vec![url_btn(&locale.order_open_app, &deep_link)]]);

    if let Err(e) = bot
        .send_message(ChatId(customer_telegram_id), text)
        .reply_markup(markup)
        .await
    {
        tracing::warn!(
            "notify_order_status failed: customer_telegram_id={} order_id={} status={} err={}",
            customer_telegram_id,
            order_id,
            status,
            e
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_text_uses_locale_for_known_statuses() {
        let locale = get_locale("ru");
        assert!(!locale.order_status_confirmed.is_empty());
        assert!(!locale.order_status_preparing.is_empty());
        assert!(!locale.order_status_ready.is_empty());
        assert!(!locale.order_status_out_for_delivery.is_empty());
        assert!(!locale.order_status_completed.is_empty());
        assert!(!locale.order_status_rejected.is_empty());
        assert!(!locale.order_status_cancelled.is_empty());
        assert!(!locale.order_open_app.is_empty());
    }

    #[test]
    fn garden_reminder_locale_fields_are_present() {
        let ru = get_locale("ru");
        assert!(!ru.garden_water_reminder.is_empty());
        assert!(!ru.garden_harvest_ready.is_empty());
        assert!(!ru.garden_open_app.is_empty());
        let en = get_locale("en");
        assert!(!en.garden_water_reminder.is_empty());
        assert!(!en.garden_harvest_ready.is_empty());
        assert!(!en.garden_open_app.is_empty());
    }
}
