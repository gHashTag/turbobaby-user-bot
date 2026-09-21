//! Friendly localised error messages for any HTTP response from any API
//! endpoint — not just `POST /api/orders`.
//!
//! Cycle #65 introduced `checkout_errors::friendly_order_error` for the
//! checkout flow. Cycles #69 + #72 localised it and added neighbour-lang
//! fallback. Cycle #73 audited every other screen and found raw err
//! strings everywhere (see `docs/ERROR_UX_AUDIT.md`).
//!
//! This module is the cycle #74 generalisation:
//!   - same status → key table as checkout (so RU/EN copy is reused),
//!   - one extra key (`T_API_ERR_401`) for the Telegram-session-expired
//!     case which doesn't fire on `POST /api/orders` (because checkout
//!     itself requires a fresh init_data) but does fire on long-lived
//!     screens like menu/quest after WebApp reload,
//!   - localised unknown-status fallback (`T_API_ERR_UNKNOWN`) instead
//!     of the inline RU `Ошибка сервера: HTTP {n}`.
//!
//! `checkout_errors::friendly_order_error` stays as a thin wrapper for
//! the existing checkout call site — same semantics, no regression.

use super::core::Lang;
use super::i18n::{
    t, T_API_ERR_401, T_API_ERR_UNKNOWN, T_CHECKOUT_ERR_400, T_CHECKOUT_ERR_403,
    T_CHECKOUT_ERR_404, T_CHECKOUT_ERR_409, T_CHECKOUT_ERR_422, T_CHECKOUT_ERR_429,
    T_CHECKOUT_ERR_5XX, T_ORDER_DETAIL_STARS,
};

/// Map an HTTP status code from any API call to a customer-friendly
/// localised sentence. Pure — looks up via the shared i18n table.
///
/// Recognised statuses: 400, 401, 403, 404, 409, 422, 429, 5xx. Anything
/// else returns the localised `T_API_ERR_UNKNOWN` copy — we no longer
/// leak `HTTP {status}` to the user (admin tooling has its own raw path
/// in `admin_screen` by design).
pub fn friendly_response_error(lang: Lang, status: u16) -> String {
    let key = match status {
        400 => T_CHECKOUT_ERR_400,
        401 => T_API_ERR_401,
        403 => T_CHECKOUT_ERR_403,
        404 => T_CHECKOUT_ERR_404,
        409 => T_CHECKOUT_ERR_409,
        422 => T_CHECKOUT_ERR_422,
        429 => T_CHECKOUT_ERR_429,
        500..=599 => T_CHECKOUT_ERR_5XX,
        _ => T_API_ERR_UNKNOWN,
    };
    t(lang, key).to_string()
}

/// What an absent number renders as. D9 in one glyph: an unread count is not
/// a zero, and on a screen about money a zero is the sentence "nothing is at
/// stake here".
///
/// The same glyph as `src/ui/components/bike_card.rs:10` (`DASH`, re-exported
/// through `catalog_screen` as `MONEY_DASH`), declared again rather than
/// imported because `src/lib.rs` gates `pub mod ui;` on `wasm32`: this module
/// compiles into the Axum server too, and nothing under `src/ui` is reachable
/// from it. The dependency runs ui -> trios and not back.
pub const ABSENT_NUMBER: &str = "\u{2014}";

/// The admin Delete button's failure line: the server's own refusal code and
/// the numbers it measured, or `None` when this build has nothing to add.
///
/// WHY IT IS HERE and not in the screen. `DELETE /api/admin/events/:id` answers
/// 409 `paid_bookings_exist` while an event still holds paid bookings, and 409
/// `acknowledgement_is_stale` when the count the caller acknowledged is not the
/// count the server measured (`src/api/events.rs`, `delete_verdict`). Until
/// 2026-09-21 `src/ui/screens/admin_screen.rs` answered every non-success with
/// one generic toast, so "three people paid for this event" and "the connection
/// dropped" were the same sentence. Reading the body in the screen instead
/// would have put that reading where no test can run it: `src/lib.rs` gates
/// `pub mod ui;` on `wasm32`, so `cargo test` compiles nothing under `src/ui`.
/// It lives here, with the tests below, and the screen calls it.
///
/// `None` is deliberate and is not an error path: it means this build has
/// nothing better to say than the caller already does, so the caller keeps its
/// own wording and its status. Answering for a body nobody recognised is the
/// same defect in the other direction.
///
/// The numbers are never invented. A code word that arrives without its counts
/// renders them as [`ABSENT_NUMBER`] (D9).
pub fn admin_event_delete_failure(lang: Lang, status: u16, body: &str) -> Option<String> {
    // The two refusals are the only 409s this endpoint answers with a body of
    // its own; every other status is somebody else's subject.
    if status != 409 {
        return None;
    }
    let parsed: serde_json::Value = serde_json::from_str(body).ok()?;
    let code = parsed.get("error")?.as_str()?;
    let bookings = number_or_dash(parsed.get("paid_bookings"));
    let stars = number_or_dash(parsed.get("stars_held"));
    // The label is the order screen's, because it is the same money: the key
    // is ASCII and greppable, the copy is the translation table's business.
    let money = t(lang, T_ORDER_DETAIL_STARS);
    match code {
        "paid_bookings_exist" => Some(format!("{code}: {bookings} \u{00b7} {money}: {stars}")),
        // The caller's own number travels too: the gap between the two is the
        // thing he has to look at, and printing only ours would hide half of
        // it behind a word.
        "acknowledgement_is_stale" => {
            let acknowledged = number_or_dash(parsed.get("acknowledged"));
            Some(format!(
                "{code}: {bookings} \u{00b7} {money}: {stars} (acknowledged {acknowledged})"
            ))
        }
        _ => None,
    }
}

/// A JSON number, or the dash. Never `0`: see [`ABSENT_NUMBER`].
fn number_or_dash(value: Option<&serde_json::Value>) -> String {
    match value.and_then(serde_json::Value::as_i64) {
        Some(n) => n.to_string(),
        None => ABSENT_NUMBER.to_string(),
    }
}

/// Should a failed authenticated request trigger a one-time re-auth retry?
///
/// Only a `401` warrants it: the Telegram WebApp sometimes exposes
/// `initDataUnsafe.user.id` (so the client knows its telegram_id and fires the
/// request) *before* the signed `initData` string is populated, and initData
/// also expires after 24h. Both surface as a `401` from `check_owner`. Re-reading
/// a fresh initData and retrying once recovers those cases without looping.
/// Other statuses (403 blocked, 5xx, network) are not fixed by re-auth.
pub fn should_retry_reauth(status: u16) -> bool {
    status == 401
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retry_reauth_only_on_401() {
        assert!(should_retry_reauth(401));
        // Not fixable by re-authenticating.
        for s in [0u16, 400, 403, 404, 409, 422, 429, 500, 502, 503] {
            assert!(
                !should_retry_reauth(s),
                "status {s} must not trigger re-auth"
            );
        }
    }

    #[test]
    fn maps_401_to_telegram_resign_in_phrase_ru() {
        let s = friendly_response_error(Lang::Russian, 401);
        assert!(s.contains("Telegram") || s.contains("Войдите"));
        // Negative: no raw status leak.
        assert!(!s.contains("401"));
    }

    #[test]
    fn maps_401_to_telegram_resign_in_phrase_en() {
        let s = friendly_response_error(Lang::English, 401);
        assert!(
            s.contains("Sign in") || s.contains("Telegram"),
            "EN 401 should mention sign-in, got: {}",
            s
        );
        // No Cyrillic leakage.
        assert!(!s.contains("Войдите"));
    }

    #[test]
    fn unknown_status_returns_localised_fallback_ru() {
        let s = friendly_response_error(Lang::Russian, 418);
        // The whole reason for this cycle: never leak raw codes through
        // the friendly path. The RU fallback should be the localised
        // unknown copy, not "HTTP 418".
        assert!(!s.contains("418"));
        assert!(!s.contains("HTTP"));
        assert!(s.contains("не так") || s.contains("позже"));
    }

    #[test]
    fn unknown_status_returns_localised_fallback_en() {
        let s = friendly_response_error(Lang::English, 418);
        assert!(!s.contains("418"));
        // EN copy must not have Cyrillic.
        assert!(!s.contains("Что"));
        assert!(s.contains("wrong") || s.contains("later"));
    }

    #[test]
    fn shares_403_copy_with_checkout() {
        // Cycle #65's auto-block phrasing should round-trip through
        // the shared helper unchanged — no fork of the message.
        let s = friendly_response_error(Lang::Russian, 403);
        assert!(s.contains("ограничен"));
        assert!(s.contains("поддержкой"));
    }

    #[test]
    fn shares_5xx_copy_with_checkout() {
        for status in [500u16, 502, 503, 504] {
            let s = friendly_response_error(Lang::Russian, status);
            assert!(s.contains("недоступен") || s.contains("Сервер"));
            assert!(!s.contains(&status.to_string()));
        }
    }

    // -- The admin's Delete button, and the refusal it must stop confusing
    //    with a dropped connection --------------------------------------
    //
    // No person is in any of these bodies: a count, a total and a code word.
    // D14 keeps names, handles and plates out of this repository, fixtures
    // included, and nothing here needs one.

    /// Three people paid, 1500 Stars are held, and the delete refused.
    const REFUSED: &str = r#"{"error":"paid_bookings_exist","paid_bookings":3,"stars_held":1500}"#;
    /// The same event, but the caller acknowledged a count from an older
    /// screen: one, where the server measures three.
    const STALE: &str = r#"{"error":"acknowledgement_is_stale","paid_bookings":3,"stars_held":1500,"acknowledged":1}"#;

    #[test]
    fn a_refused_delete_reads_as_the_code_the_count_and_the_money() {
        let stars = t(Lang::English, T_ORDER_DETAIL_STARS);
        assert_eq!(
            admin_event_delete_failure(Lang::English, 409, REFUSED)
                .expect("a 409 the server explains is a 409 this reader explains"),
            format!("paid_bookings_exist: 3 \u{00b7} {stars}: 1500")
        );
    }

    #[test]
    fn a_stale_acknowledgement_shows_both_counts() {
        let stars = t(Lang::English, T_ORDER_DETAIL_STARS);
        assert_eq!(
            admin_event_delete_failure(Lang::English, 409, STALE)
                .expect("the stale-screen refusal is read too"),
            format!("acknowledgement_is_stale: 3 \u{00b7} {stars}: 1500 (acknowledged 1)")
        );
    }

    #[test]
    fn the_refusal_is_not_the_generic_409() {
        let refusal = admin_event_delete_failure(Lang::Russian, 409, REFUSED).expect("read");
        assert_ne!(
            refusal,
            friendly_response_error(Lang::Russian, 409),
            "the admin sees one sentence for three paid bookings and for every other conflict"
        );
        assert!(refusal.contains("1500"), "the money is missing: {refusal}");
    }

    #[test]
    fn an_absent_count_is_a_dash_and_never_a_zero() {
        // D9: the code word arrived, the numbers did not. A zero here would
        // tell the admin nothing is at stake, on the one screen that exists
        // to say how much is.
        let s =
            admin_event_delete_failure(Lang::English, 409, r#"{"error":"paid_bookings_exist"}"#)
                .expect("the code alone is still the refusal");
        assert!(
            !s.contains('0'),
            "an absent count became a confident zero: {s}"
        );
        assert!(s.contains(ABSENT_NUMBER), "an absent count is a dash: {s}");
    }

    #[test]
    fn a_body_this_build_cannot_read_is_left_to_the_caller() {
        // None means "say what you always said": the caller keeps its own
        // wording and its status. Inventing a sentence for a body nobody
        // recognises is how a network failure came to look like a refusal.
        for body in [
            r#"{"error":"server_error"}"#,
            r#"{"paid_bookings":3}"#,
            "<html>502 Bad Gateway</html>",
            "",
        ] {
            assert_eq!(
                admin_event_delete_failure(Lang::English, 409, body),
                None,
                "body {body:?} was answered for"
            );
        }
        // Not every 409 in the world is this one, and no other status is.
        assert_eq!(
            admin_event_delete_failure(Lang::English, 500, REFUSED),
            None
        );
        assert_eq!(
            admin_event_delete_failure(Lang::English, 200, REFUSED),
            None
        );
    }

    #[test]
    fn the_money_label_is_localised_and_the_numbers_are_not() {
        let ru = admin_event_delete_failure(Lang::Russian, 409, REFUSED).expect("read");
        let en = admin_event_delete_failure(Lang::English, 409, REFUSED).expect("read");
        assert_ne!(ru, en, "the label is not coming from the i18n table");
        for s in [&ru, &en] {
            assert!(s.contains('3') && s.contains("1500"), "{s}");
        }
    }

    #[test]
    fn every_known_status_includes_actionable_hint() {
        // Catches future regressions where someone shortens a message
        // to "Ошибка" and removes the recovery hint.
        for status in [400u16, 401, 403, 404, 409, 422, 429, 500] {
            let s = friendly_response_error(Lang::Russian, status);
            let has_action = [
                "корзин",
                "поддерж",
                "Обновите",
                "Мои заказы",
                "минут",
                "Telegram",
                "Войдите",
            ]
            .iter()
            .any(|kw| s.contains(kw));
            assert!(
                has_action,
                "status {} must include an actionable next step, got: {}",
                status, s
            );
        }
    }
}
