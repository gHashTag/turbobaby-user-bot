use crate::config::Config;
use teloxide::prelude::*;
use teloxide::types::ParseMode;
use teloxide::Bot;

/// Send a Telegram message to every admin in `config.admin_ids`.
///
/// Cycle #149: previously this re-escaped `text` via `html_escape` and
/// sent with no `parse_mode`, so callers that formatted with `<b>` /
/// `<i>` saw literal `&lt;b&gt;` markup in the admin chat — and any
/// `html_escape`-already-applied user field came through double-escaped
/// (`&lt;Bob&gt;` → `&amp;lt;Bob&amp;gt;`). The fix:
///   1. Send with `parse_mode=Html` so the markup renders.
///   2. Don't re-escape — callers are responsible for `html_escape`ing
///      their user-controlled substrings (audited cycle #149).
pub async fn notify_admins(bot: &Bot, config: &Config, text: &str) {
    for admin_id in &config.admin_ids {
        if let Err(e) = bot
            .send_message(teloxide::types::ChatId(*admin_id), text)
            .parse_mode(ParseMode::Html)
            .await
        {
            tracing::warn!("notify_admins failed for admin_id={}: {}", admin_id, e);
        }
    }
}
