pub(crate) mod callbacks;
pub(crate) mod commands;
pub(crate) mod handlers;
pub(crate) mod notify;

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
pub(crate) static BOT_AI_RATE_LIMIT: std::sync::LazyLock<
    crate::api::rate_limit::SyncSlidingWindowStore,
> = std::sync::LazyLock::new(crate::api::rate_limit::new_sync_store);
pub(crate) const AI_COOLDOWN: Duration = Duration::from_secs(5);
const BOT_AI_RL_MAX_KEYS: usize = 10_000;

/// Returns `true` if the user is allowed to make an AI request now,
/// `false` if they hit the cooldown. Records the attempt on `true`.
pub(crate) fn ai_rate_limit_allow(user_id: i64) -> bool {
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

#[allow(clippy::expect_used)] // Fallback URL `https://t.me` is a static literal; parse is infallible.
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

#[allow(clippy::expect_used)] // Fallback URL `https://t.me` is a static literal; parse is infallible.
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

/// Build a `t.me/{bot}?startapp={param}` deep link that opens the Mini App
/// with a specific context. These links survive forwarding: the recipient's
/// Telegram client parses `startapp` and launches the bot's Mini App with the
/// same parameter.
pub(crate) fn miniapp_deep_link(bot_username: &str, start_param: &str) -> String {
    // `?start=` (not `?startapp=`) — see `is_miniapp_start_payload` for why.
    format!(
        "https://t.me/{}?start={}",
        urlencoding::encode(bot_username),
        urlencoding::encode(start_param)
    )
}

// Telegram's `start` cap lives in `trios::deeplink::MAX_START_PARAM_LEN`,
// with the parser that enforces it. A second copy here was a second thing to
// keep in sync, which is the defect this whole area just paid for.

/// Does this `/start` payload address a Mini App target (product card, order,
/// cart, reorder, garden invite) rather than a plain chat start?
///
/// Shared links use `t.me/<bot>?start=<payload>` rather than `?startapp=`:
/// `?startapp=` only launches the Mini App when the bot has a **Main Mini App**
/// configured in BotFather, and without it Telegram just opens the bot chat —
/// which is why shared cards landed on the bot's home screen. `?start=` always
/// reaches the bot, and the handler answers with a Mini App button carrying the
/// payload through to the app.
///
/// Prefixes must stay in sync with the parsers in `src/ui/share.rs`.
pub(crate) fn is_miniapp_start_payload(payload: &str) -> bool {
    // Delegates. This used to be a second list of prefixes, and the two lists
    // disagreed about twelve payloads out of a forty-six-payload sweep — a
    // disagreement nothing could see, because the other list lives in
    // `src/ui`, which is `#[cfg(target_arch = "wasm32")]`.
    //
    // Both directions were live defects and both read to a customer as "the
    // link does not work":
    //
    //   cart__<source>   the app opens it, the bot refused to answer -> every
    //                    campaign cart link landed on the plain welcome
    //   garden           same, for a garden invite carrying no referrer id
    //   p_set_, o_,      the bot answered with a Mini App button for payloads
    //   reorder__,       the app cannot parse, so the button opened the home
    //   garden__0, ...   screen instead of the card
    crate::trios::deeplink::is_miniapp_payload(payload)
}

/// Build a `startapp` parameter for a catalog product.
pub(crate) fn product_start_param(kind_prefix: &str, product_id: &str) -> String {
    format!("{}_{}", kind_prefix, product_id)
}

/// Build a `startapp` parameter that opens the Mini App on the order detail
/// screen. Format: `o_{order_id}`. Must stay in sync with
/// `src/ui/share.rs::parse_order_start_param`.
pub(crate) fn order_start_param(order_id: &str) -> String {
    format!("o_{}", order_id)
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

#[allow(dead_code)] // Called only from main.rs (bin); lib has no user.
pub(crate) fn create_handler() -> UpdateHandler<teloxide::RequestError> {
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

/// Architectural fitness function (OWASP LLM10 — Unbounded Consumption /
/// denial-of-wallet): every bot source file that calls the paid AI
/// (`ask_grok`) must also gate it behind the per-user rate limiter
/// (`ai_rate_limit_allow`). This locks in the cycle-#129 coverage so a future
/// AI command/callback can't be added without the rate limit and silently open
/// a cost-abuse hole.
#[cfg(test)]
mod ai_rate_limit_coverage_tests {
    use std::path::Path;

    #[test]
    fn every_ask_grok_caller_in_bot_is_rate_limited() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/bot");
        let mut offenders = Vec::new();
        for entry in std::fs::read_dir(&dir).expect("read src/bot") {
            let path = entry.expect("dir entry").path();
            if path.extension().and_then(|e| e.to_str()) != Some("rs") {
                continue;
            }
            let src = std::fs::read_to_string(&path).expect("read bot file");
            // `.ask_grok(` = a call site (the fn is DEFINED in src/ai.rs, not here).
            if src.contains(".ask_grok(") && !src.contains("ai_rate_limit_allow") {
                offenders.push(path.display().to_string());
            }
        }
        assert!(
            offenders.is_empty(),
            "every bot file calling .ask_grok() must gate it behind \
             ai_rate_limit_allow (OWASP LLM10 denial-of-wallet). Offenders:\n  {}",
            offenders.join("\n  ")
        );
    }
}

#[cfg(test)]
mod deep_link_tests {
    use super::{miniapp_deep_link, product_start_param};

    #[test]
    fn miniapp_deep_link_uses_start_not_startapp() {
        // `?startapp=` needs a Main Mini App registered in BotFather; without
        // it Telegram opened the bot's home screen instead of the shared card.
        let url = miniapp_deep_link("Woody_WeedPecker_bot", "p_set_abc123");
        assert_eq!(url, "https://t.me/Woody_WeedPecker_bot?start=p_set_abc123");
    }

    #[test]
    fn product_start_param_matches_ui_convention() {
        // This must stay in sync with src/ui/share.rs::ProductKind prefixes so
        // that a link generated by the bot parses correctly in the Mini App.
        assert_eq!(product_start_param("p_strain", "42"), "p_strain_42");
        assert_eq!(product_start_param("p_set", "x"), "p_set_x");
        assert_eq!(product_start_param("p_acc", "y"), "p_acc_y");
        assert_eq!(product_start_param("p_tea", "z"), "p_tea_z");
    }

    #[test]
    fn miniapp_deep_link_url_encodes_username_and_param() {
        let url = miniapp_deep_link("bot name", "p_set:a b");
        assert!(url.starts_with("https://t.me/"));
        assert!(!url.contains(' '));
        assert!(url.contains("bot%20name"));
        assert!(url.contains("p_set%3Aa%20b"));
    }
    /// The two implementations of "is this a Mini App target" — the bot's, and
    /// the one the Mini App parses with — swept against each other.
    ///
    /// They were written in different files, one of which (`src/ui`) is
    /// `#[cfg(target_arch = "wasm32")]` and therefore invisible to this suite,
    /// so nothing has ever compared them. A disagreement is not a cosmetic
    /// drift: where the bot says no, the customer taps a shared link and gets
    /// the ordinary welcome instead of the card; where the app says no, the
    /// button opens the home screen. Both read as "the link does not work".
    #[test]
    fn the_classifier_agrees_with_the_parser_the_app_uses() {
        use crate::bot::is_miniapp_start_payload;
        use crate::trios::deeplink;

        let mut corpus: Vec<String> = Vec::new();
        for kind in deeplink::Kind::ALL {
            for id in [
                "fe346171-aa5b-4f88-93ed-8be0ec38aa6c",
                "black-heavy-hit-pack",
                "42",
                "",
                "set_x",
            ] {
                corpus.push(format!("{}_{}", kind.prefix(), id));
            }
        }
        for other in [
            "cart",
            "cart__utm_a",
            "cart__",
            "o_7f3a",
            "o_",
            "reorder__7f3a",
            "reorder__",
            "garden",
            "garden__123",
            "garden__123__utm_a",
            "garden__0",
            "garden__-5",
            "garden__abc",
            "ref_0u3KYyAT",
            "channel",
            "",
            "p_set_a b",
            "p_set_\u{43e}\u{43f}",
            &"x".repeat(65),
            &format!("p_set_{}", "x".repeat(58)),
            &format!("p_set_{}", "x".repeat(59)),
        ] {
            corpus.push(other.to_string());
        }

        let mut disagreements = Vec::new();
        for payload in &corpus {
            let bot_says = is_miniapp_start_payload(payload);
            let app_says = deeplink::is_miniapp_payload(payload);
            if bot_says != app_says {
                disagreements.push(format!(
                    "{payload:?}: bot answers {bot_says}, app parses {app_says}"
                ));
            }
        }
        assert!(
            corpus.len() >= 45,
            "the sweep only built {} payloads; a scan that checks nothing is clean by default",
            corpus.len()
        );
        // The sweep above compares one function with itself now that the bot
        // delegates, so on its own it is vacuous. These two are not: each was
        // FALSE under the bot's old list and TRUE in the app, and each is a
        // link customers actually send. They fail against any re-introduced
        // second list that forgets them, which is the only way this defect
        // comes back.
        assert!(
            is_miniapp_start_payload("cart__utm_a"),
            "a campaign cart link must be answered with a Mini App button"
        );
        assert!(
            is_miniapp_start_payload("garden"),
            "a garden invite with no referrer must still open the garden"
        );
        assert!(
            !is_miniapp_start_payload("p_set_"),
            "a prefix with no id must not produce a button that opens the home screen"
        );

        assert!(
            disagreements.is_empty(),
            "the bot and the Mini App disagree about {} of {} payloads:\n  {}",
            disagreements.len(),
            corpus.len(),
            disagreements.join("\n  ")
        );
    }
} // mod deep_link_tests
