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

/// On startup after a successful Railway deploy, tell every admin WHAT shipped
/// (the commit subject) + the version, so they know which area to test/check.
///
/// Gated to real deploys: Railway injects `RAILWAY_GIT_COMMIT_*` env vars, so we
/// only send when `RAILWAY_GIT_COMMIT_SHA` is present (skips local `cargo run`).
/// Fired on boot — the only reliable "deploy succeeded" signal (the new backend
/// booted). A rare extra ping on a crash-restart is acceptable (also useful).
pub async fn notify_deploy(bot: &Bot, config: &Config) {
    let Ok(sha) = std::env::var("RAILWAY_GIT_COMMIT_SHA") else {
        return; // not on Railway (local dev) — don't spam admins.
    };
    let sha7: String = sha.chars().take(7).collect();
    let full_msg = std::env::var("RAILWAY_GIT_COMMIT_MESSAGE").unwrap_or_default();
    // First line = the change headline; keep up to ~5 lines of detail.
    let mut lines = full_msg.lines();
    let subject = lines.next().unwrap_or("").trim();
    let body: Vec<&str> = lines
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .take(5)
        .collect();
    let author = std::env::var("RAILWAY_GIT_AUTHOR").unwrap_or_default();

    let headline = if subject.is_empty() {
        "новый деплой".to_string()
    } else {
        crate::util::html_escape(subject)
    };
    let mut text = format!(
        "\u{1F680} <b>Новый деплой на проде</b>\n\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\n\u{1F4E6} {headline}\n\u{1F516} {} \u{00B7} v{}",
        sha7,
        env!("BUILD_VERSION"),
    );
    if !author.is_empty() {
        text.push_str(&format!(
            "\n\u{1F464} {}",
            crate::util::html_escape(&author)
        ));
    }
    if !body.is_empty() {
        text.push_str("\n\n<b>Что изменено:</b>");
        for l in body {
            text.push_str(&format!("\n\u{2022} {}", crate::util::html_escape(l)));
        }
    }
    text.push_str("\n\n\u{1F9EA} Проверьте раздел, которого касается это изменение.");

    notify_admins(bot, config, &text).await;
}
