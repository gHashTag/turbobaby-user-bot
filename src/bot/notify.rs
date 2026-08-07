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
        "completed" => &locale.order_status_completed,
        "rejected" => &locale.order_status_rejected,
        _ => "Order status updated",
    };

    let short_id = &order_id[order_id.len().saturating_sub(6)..];
    let text = format!("{} #{}\n\n{}", status_text, short_id, locale.order_open_app);

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
        assert!(!locale.order_status_completed.is_empty());
        assert!(!locale.order_status_rejected.is_empty());
        assert!(!locale.order_open_app.is_empty());
    }
}
