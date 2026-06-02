pub mod callbacks;
pub mod commands;
pub mod handlers;

use std::time::Duration;

/// Cycle #129: AI per-user rate-limit moved onto the shared
/// `SyncSlidingWindowStore` primitive (cycle #106). Pre-#129 each of
/// the four bot entrypoints (text, two command branches, callback)
/// duplicated a `HashMap<i64, Instant>` + `tokio::sync::Mutex`
/// debouncer with a hand-rolled stale-entry GC. Same semantics —
/// "1 AI request per `AI_COOLDOWN` per user" — but now consolidated
/// behind one helper so a future tweak ("bump to 10s") edits one
/// constant and one expectation, not four.
///
/// Mechanically: max_attempts=1 + window=AI_COOLDOWN matches the
/// original behaviour exactly. The library's `max_keys` eviction
/// replaces the manual `retain` 5-minute sweep; the practical
/// memory bound is comparable (we cap at 10_000 active users).
pub static BOT_AI_RATE_LIMIT: std::sync::LazyLock<crate::api::rate_limit::SyncSlidingWindowStore> =
    std::sync::LazyLock::new(crate::api::rate_limit::new_sync_store);
pub const AI_COOLDOWN: Duration = Duration::from_secs(5);
const BOT_AI_RL_MAX_KEYS: usize = 10_000;

/// Returns `true` if the user is allowed to make an AI request now,
/// `false` if they hit the cooldown. Records the attempt on `true`.
pub fn ai_rate_limit_allow(user_id: i64) -> bool {
    crate::api::rate_limit::check_and_record_sync(
        &BOT_AI_RATE_LIMIT,
        &user_id.to_string(),
        AI_COOLDOWN,
        1,
        BOT_AI_RL_MAX_KEYS,
    )
}

#[cfg(test)]
mod ai_rate_limit_tests {
    // Cycle #129 contract: per-user 1-attempt-per-window. Tests use a
    // randomised user_id so they don't share the global `BOT_AI_RATE_LIMIT`
    // store's keyspace across parallel test runs.

    fn random_user_id() -> i64 {
        // u32 portion of a v4 UUID — guaranteed unique across the suite.
        let id = uuid::Uuid::new_v4().as_u128() as i64;
        // Avoid 0 so the helper sees a real key.
        if id == 0 {
            1
        } else {
            id
        }
    }

    #[test]
    fn first_call_allowed_second_blocked() {
        let uid = random_user_id();
        assert!(
            super::ai_rate_limit_allow(uid),
            "first call from a fresh user must be allowed"
        );
        assert!(
            !super::ai_rate_limit_allow(uid),
            "second call within the cooldown window must be blocked"
        );
    }

    #[test]
    fn different_users_independent() {
        let uid_a = random_user_id();
        let uid_b = random_user_id();
        assert_ne!(uid_a, uid_b);
        assert!(super::ai_rate_limit_allow(uid_a));
        assert!(super::ai_rate_limit_allow(uid_b));
        // A's bucket is exhausted, B's is independent and still allowed
        // for its OWN second call only if window expired — but here we
        // just verify that A's exhaustion doesn't propagate to B's
        // first call. (Cross-user isolation invariant.)
        assert!(
            !super::ai_rate_limit_allow(uid_a),
            "A's bucket should remain exhausted"
        );
    }
}

use teloxide::dispatching::UpdateHandler;
use teloxide::prelude::*;
use teloxide::types::{InlineKeyboardButton, WebAppInfo};

pub(crate) fn web_app_btn(text: &str, url: &str) -> InlineKeyboardButton {
    match url.parse() {
        Ok(u) => InlineKeyboardButton::web_app(text, WebAppInfo { url: u }),
        Err(e) => {
            tracing::error!("Invalid web_app URL '{}': {}", url, e);
            InlineKeyboardButton::url(
                text,
                "https://t.me".parse().expect("static URL is always valid"),
            )
        }
    }
}

pub(crate) fn callback_btn(text: &str, data: &str) -> InlineKeyboardButton {
    InlineKeyboardButton::callback(text, data)
}

pub(crate) fn url_btn(text: &str, url: &str) -> InlineKeyboardButton {
    match url.parse() {
        Ok(u) => InlineKeyboardButton::url(text, u),
        Err(e) => {
            tracing::error!("Invalid URL '{}': {}", url, e);
            InlineKeyboardButton::url(
                text,
                "https://t.me".parse().expect("static URL is always valid"),
            )
        }
    }
}

/// Wrap a Telegram API call that's allowed to fail silently. Rate-limit
/// (HTTP 429) and "message too old to edit" failures are routine — those
/// are dropped without noise. Everything else gets a `debug!` log so ops
/// can see real connectivity / permissions issues that previously got
/// swallowed by `.await.ok()`.
///
/// Cycle #78: was 15+ bare `.ok()` callsites on `edit_message_text` /
/// `delete_message` etc. Replacing every one is invasive; this helper
/// targets the highest-signal ones where the user sees the failure (e.g.
/// "Joke fail" stuck on screen because the final edit was rejected).
pub(crate) async fn tg_fire_and_forget<T, E: std::fmt::Display>(
    fut: impl std::future::Future<Output = std::result::Result<T, E>>,
    ctx: &'static str,
) {
    if let Err(e) = fut.await {
        let s = e.to_string();
        let is_expected = s.contains("Too Many Requests")
            || s.contains("retry_after")
            || s.contains("message is not modified")
            || s.contains("message to edit not found")
            || s.contains("message to delete not found");
        if !is_expected {
            tracing::debug!("tg: {} failed: {}", ctx, s);
        }
    }
}

pub fn create_handler() -> UpdateHandler<teloxide::RequestError> {
    dptree::entry()
        .branch(
            Update::filter_message()
                .branch(
                    dptree::entry()
                        .filter_command::<commands::Command>()
                        .endpoint(commands::handle_command),
                )
                .branch(Message::filter_text().endpoint(handlers::handle_text))
                .branch(
                    dptree::filter(|msg: Message| msg.web_app_data().is_some())
                        .endpoint(handlers::handle_web_app_data),
                ),
        )
        .branch(Update::filter_callback_query().endpoint(callbacks::handle_callback))
}
