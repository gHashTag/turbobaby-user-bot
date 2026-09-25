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

// -- A customer cancelling their own order, and what they are told of it ------
//
// Until 2026-09-22 the order detail screen sent `POST /api/orders/:id/cancel`,
// bound the answer to an unread local and closed its dialog on every path, so
// a success, a refusal and a lost connection looked the same. The reading lives
// here and not in the screen for the reason `admin_event_delete_failure` gives:
// `src/lib.rs` gates `pub mod ui;` on `wasm32`, so `cargo test` compiles nothing
// under `src/ui`. The screen asks these functions; the tests below pin them.
//
// Three outcomes and not two. That is the owner's rule for the bot's service
// receipt and its cash desk (knowledge base, 13-14.08.2026): done, not done, or
// unknown -- and an unknown outcome is looked at again BEFORE anyone repeats it.
// The Mini App has no rule of its own in the owner's words; this follows that one.
// Four kinds of answer reach those three outcomes: a success is done, a refusal
// is not done, and a failure of the server's own and no answer at all are both
// unknown.

/// What the cancellation request got back.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderCancelAnswer {
    /// A 2xx. `cancel_order` in `src/api/orders.rs` answers one only after its
    /// commit, so the order IS cancelled.
    Cancelled,
    /// A status outside 2xx and outside the server class. Every such refusal of
    /// that handler, and of the identity gates and the limiter in front of it,
    /// is returned before the commit: nothing was written.
    Refused(u16),
    /// A status of the server class (5xx). The handler answers one when its own
    /// commit fails, and a proxy in front of the server can answer one after
    /// the commit landed, so whether the cancellation landed is UNKNOWN.
    ServerFailed(u16),
    /// No status arrived. Whether the cancellation landed is UNKNOWN.
    NoAnswer,
}

impl OrderCancelAnswer {
    /// Whether what arrived leaves the outcome unknown: a failure of the
    /// server's own, or no answer at all.
    pub fn outcome_is_unknown(self) -> bool {
        matches!(self, Self::ServerFailed(_) | Self::NoAnswer)
    }
}

/// Where the customer's attempt stands. The order card owns it, and only the
/// card and the card's own task write it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderCancelProgress {
    Idle,
    InFlight,
    Answered(OrderCancelAnswer),
}

/// What the screen has read of the order's status SINCE the answer arrived.
/// The screen drops its last reading when an answer arrives and asks again,
/// which is what makes "not read yet" observable. Only an attempt whose
/// outcome is unknown consults it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusSinceAnswer {
    /// Nothing has landed since the answer.
    NotYetRead,
    /// A reading landed and carried no status: the read itself failed.
    Unreadable,
    /// A reading landed. `still_cancellable` is whether its status is the one a
    /// customer may cancel, as the caller's status reading decides it.
    Read { still_cancellable: bool },
}

impl StatusSinceAnswer {
    /// `landed` is `None` while nothing has landed, `Some(false)` when the read
    /// failed and `Some(true)` when it gave a status.
    pub fn from_landing(landed: Option<bool>, still_cancellable: bool) -> Self {
        match landed {
            None => Self::NotYetRead,
            Some(false) => Self::Unreadable,
            Some(true) => Self::Read { still_cancellable },
        }
    }
}

/// The three outcomes, as the line about the attempt is coloured.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderCancelTone {
    Done,
    NotDone,
    Unknown,
}

impl OrderCancelTone {
    /// The green, red and grey the order screens already use for a finished, a
    /// stopped and an unreadable status.
    pub fn color(self) -> &'static str {
        match self {
            Self::Done => "#39ff14",
            Self::NotDone => "#ff4757",
            Self::Unknown => "#8b8b9e",
        }
    }
}

/// One line about the attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderCancelLine {
    pub text: String,
    pub tone: OrderCancelTone,
}

/// The answer, read from the status alone: the same 2xx range as
/// `LocalStatus::is_success`, and `None` when no status arrived.
pub fn order_cancel_answer(status: Option<u16>) -> OrderCancelAnswer {
    match status {
        Some(200..=299) => OrderCancelAnswer::Cancelled,
        Some(s @ 500..=599) => OrderCancelAnswer::ServerFailed(s),
        Some(s) => OrderCancelAnswer::Refused(s),
        None => OrderCancelAnswer::NoAnswer,
    }
}

/// AGENTS.md, lesson 4: never close the UI before the server confirms.
pub fn order_cancel_dialog_closes(answer: OrderCancelAnswer) -> bool {
    matches!(answer, OrderCancelAnswer::Cancelled)
}

/// Whether "cancel order" is still offered. Withdrawn for the rest of the view
/// once the server confirmed, even while a stale reading still says pending.
pub fn order_cancel_offer_shown(progress: OrderCancelProgress) -> bool {
    !matches!(
        progress,
        OrderCancelProgress::Answered(OrderCancelAnswer::Cancelled)
    )
}

/// Whether Confirm may send. Never while a request is out. After an attempt
/// whose outcome is unknown -- no answer, or a failure of the server's own --
/// only once a fresh reading shows the order still cancellable: until then a
/// second attempt would be a blind repeat. A refusal was given before anything
/// was written, so repeating one is not blind.
pub fn order_cancel_may_send(progress: OrderCancelProgress, since: StatusSinceAnswer) -> bool {
    match progress {
        OrderCancelProgress::InFlight => false,
        OrderCancelProgress::Answered(answer) if answer.outcome_is_unknown() => matches!(
            since,
            StatusSinceAnswer::Read {
                still_cancellable: true
            }
        ),
        _ => true,
    }
}

/// What the customer is told about the attempt, or `None` when there is
/// nothing to tell: no answer yet, or an attempt whose outcome is unknown and
/// whose fresh reading shows the order has left pending, where the status label
/// is the answer and nothing may claim what THIS attempt did.
///
/// Keys that already existed, and one written for this surface. While an
/// unknown outcome is checked, the line says what the screen is doing: reading
/// the status, or failing to. Once a fresh reading shows the order still
/// pending, the attempt did not land and is told as what it was. The general
/// mapper is used where its sentence is true of a cancellation (401, 403, 429,
/// 5xx; a 5xx only after that check). It is not used for 400, 404, 409 and 422:
/// those sentences speak of a cart, an item, a duplicate order and a price, and
/// 409 is the one refusal this route actually makes (`cancel_order` in
/// `src/api/orders.rs` answers it when the order has left pending). That
/// refusal is told `T_ORDER_DETAIL_CANCEL_REFUSED`: the app can no longer
/// cancel this order, write to the manager. It says nothing about what the shop
/// is doing with the order, because the same 409 answers an order the shop took,
/// one it rejected and one already cancelled. Until 2026-09-25 it was told the
/// general mapper's own fallback, whose "try again later" is false advice for
/// it; the copy was chosen that day under the owner's delegation, reworded the
/// same day so that it holds for every 409 (the first wording said the order was
/// already being handled), and `client_errors.t27` records the decision. Any
/// other refusal keeps that fallback.
pub fn order_cancel_line(
    lang: Lang,
    progress: OrderCancelProgress,
    since: StatusSinceAnswer,
) -> Option<OrderCancelLine> {
    use super::i18n::{
        T_CHECKOUT_ERR_NETWORK, T_ORDER_DETAIL_CANCELLED_BY_USER, T_ORDER_DETAIL_CANCEL_REFUSED,
        T_ORDER_DETAIL_NOT_FOUND, T_SUCCESS_STATUS_ERROR, T_SUCCESS_STATUS_LOADING,
    };
    let OrderCancelProgress::Answered(answer) = progress else {
        return None;
    };
    let (text, tone) = match (answer.outcome_is_unknown(), since) {
        (true, StatusSinceAnswer::NotYetRead) => (
            t(lang, T_SUCCESS_STATUS_LOADING).to_string(),
            OrderCancelTone::Unknown,
        ),
        (true, StatusSinceAnswer::Unreadable) => (
            t(lang, T_SUCCESS_STATUS_ERROR).to_string(),
            OrderCancelTone::Unknown,
        ),
        (
            true,
            StatusSinceAnswer::Read {
                still_cancellable: false,
            },
        ) => return None,
        // A known outcome, or an unknown one a fresh reading has shown did not land.
        _ => match answer {
            OrderCancelAnswer::Cancelled => (
                t(lang, T_ORDER_DETAIL_CANCELLED_BY_USER).to_string(),
                OrderCancelTone::Done,
            ),
            OrderCancelAnswer::Refused(s @ (401 | 403 | 429))
            | OrderCancelAnswer::ServerFailed(s) => {
                (friendly_response_error(lang, s), OrderCancelTone::NotDone)
            }
            OrderCancelAnswer::Refused(404) => (
                t(lang, T_ORDER_DETAIL_NOT_FOUND).to_string(),
                OrderCancelTone::NotDone,
            ),
            OrderCancelAnswer::Refused(409) => (
                t(lang, T_ORDER_DETAIL_CANCEL_REFUSED).to_string(),
                OrderCancelTone::NotDone,
            ),
            OrderCancelAnswer::Refused(_) => (
                t(lang, T_API_ERR_UNKNOWN).to_string(),
                OrderCancelTone::NotDone,
            ),
            OrderCancelAnswer::NoAnswer => (
                t(lang, T_CHECKOUT_ERR_NETWORK).to_string(),
                OrderCancelTone::NotDone,
            ),
        },
    };
    Some(OrderCancelLine { text, tone })
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

    // -- A customer cancelling their own order ----------------------------
    //
    // Only status numerals and the reading's own states appear below; no order
    // id, no name and no identity (D14).

    use crate::trios::i18n::{
        T_CHECKOUT_ERR_NETWORK, T_ORDER_DETAIL_CANCELLED_BY_USER, T_ORDER_DETAIL_CANCEL_REFUSED,
        T_ORDER_DETAIL_NOT_FOUND, T_SUCCESS_STATUS_ERROR, T_SUCCESS_STATUS_LOADING,
    };

    const LANGS: [Lang; 2] = [Lang::Russian, Lang::English];

    /// Every state a fresh status reading can be in after an answer.
    const SINCE: [StatusSinceAnswer; 4] = [
        StatusSinceAnswer::NotYetRead,
        StatusSinceAnswer::Unreadable,
        StatusSinceAnswer::Read {
            still_cancellable: true,
        },
        StatusSinceAnswer::Read {
            still_cancellable: false,
        },
    ];

    fn answered(answer: OrderCancelAnswer) -> OrderCancelProgress {
        OrderCancelProgress::Answered(answer)
    }

    fn text_of(lang: Lang, progress: OrderCancelProgress, since: StatusSinceAnswer) -> String {
        order_cancel_line(lang, progress, since)
            .map(|line| line.text)
            .unwrap_or_default()
    }

    #[test]
    fn the_answer_is_read_from_the_status_alone() {
        for s in [200u16, 201, 204, 299] {
            assert_eq!(order_cancel_answer(Some(s)), OrderCancelAnswer::Cancelled);
        }
        for s in [
            100u16, 199, 300, 302, 400, 401, 403, 404, 409, 429, 499, 600,
        ] {
            assert_eq!(order_cancel_answer(Some(s)), OrderCancelAnswer::Refused(s));
        }
        for s in [500u16, 502, 503, 504, 599] {
            assert_eq!(
                order_cancel_answer(Some(s)),
                OrderCancelAnswer::ServerFailed(s)
            );
        }
        assert_eq!(order_cancel_answer(None), OrderCancelAnswer::NoAnswer);
        // Three outcomes: done, not done, and unknown for the last two kinds.
        assert!(!OrderCancelAnswer::Cancelled.outcome_is_unknown());
        assert!(!OrderCancelAnswer::Refused(409).outcome_is_unknown());
        assert!(OrderCancelAnswer::ServerFailed(503).outcome_is_unknown());
        assert!(OrderCancelAnswer::NoAnswer.outcome_is_unknown());
    }

    #[test]
    fn only_a_confirmed_success_closes_the_cancel_dialog() {
        assert!(order_cancel_dialog_closes(order_cancel_answer(Some(200))));
        assert!(order_cancel_dialog_closes(order_cancel_answer(Some(204))));
        for s in [400u16, 401, 403, 404, 409, 500, 502, 503] {
            assert!(
                !order_cancel_dialog_closes(order_cancel_answer(Some(s))),
                "a {s} closed the dialog"
            );
        }
        assert!(!order_cancel_dialog_closes(order_cancel_answer(None)));
    }

    #[test]
    fn a_refused_cancellation_is_never_told_it_succeeded() {
        let left = StatusSinceAnswer::Read {
            still_cancellable: false,
        };
        let checked = StatusSinceAnswer::Read {
            still_cancellable: true,
        };
        for lang in LANGS {
            let success = text_of(lang, answered(OrderCancelAnswer::Cancelled), SINCE[0]);
            for s in (100u16..=599).filter(|s| !(200..=299).contains(s)) {
                let answer = order_cancel_answer(Some(s));
                assert!(!order_cancel_dialog_closes(answer), "{s} closes the dialog");
                for since in SINCE {
                    let Some(line) = order_cancel_line(lang, answered(answer), since) else {
                        // Only an unknown outcome whose fresh reading shows the
                        // order gone from pending is told by the status label.
                        assert!(
                            answer.outcome_is_unknown() && since == left,
                            "a {s} is told nothing ({since:?})"
                        );
                        continue;
                    };
                    assert!(!line.text.is_empty(), "a {s} is told an empty line");
                    assert_ne!(line.text, success, "a {s} is told it succeeded");
                    let tone = if answer.outcome_is_unknown() && since != checked {
                        OrderCancelTone::Unknown
                    } else {
                        OrderCancelTone::NotDone
                    };
                    assert_eq!(line.tone, tone, "{s} / {since:?}");
                    assert!(!line.text.contains(&s.to_string()), "{s} leaks its numeral");
                }
            }
        }
    }

    #[test]
    fn the_general_mapper_speaks_where_its_sentence_is_true() {
        // 401, 403, 429 and 5xx speak of the session, the account, the pace and
        // the server -- true of a cancellation. The rest speak of a cart, an
        // item, a duplicate order and a price, and are not used.
        // A 5xx speaks only once a fresh reading shows the attempt did not land.
        let checked = StatusSinceAnswer::Read {
            still_cancellable: true,
        };
        for lang in LANGS {
            for s in [401u16, 403, 429, 500, 502, 503, 504] {
                assert_eq!(
                    text_of(lang, answered(order_cancel_answer(Some(s))), checked),
                    friendly_response_error(lang, s),
                    "{s}"
                );
            }
            for s in [401u16, 403, 429] {
                assert_eq!(
                    text_of(lang, answered(order_cancel_answer(Some(s))), SINCE[0]),
                    friendly_response_error(lang, s),
                    "{s}"
                );
            }
            assert_eq!(
                text_of(lang, answered(OrderCancelAnswer::Refused(404)), SINCE[0]),
                t(lang, T_ORDER_DETAIL_NOT_FOUND)
            );
            // 409 is not here: it has its own key since 2026-09-25 (below).
            for s in [400u16, 418, 422] {
                assert_eq!(
                    text_of(lang, answered(OrderCancelAnswer::Refused(s)), SINCE[0]),
                    t(lang, T_API_ERR_UNKNOWN),
                    "{s}"
                );
            }
        }
    }

    /// The one refusal `cancel_order` makes itself: 409, the order has left
    /// pending. Until 2026-09-25 it was told the general fallback, whose
    /// "try again later" is false advice: a repeat meets the same 409 for as
    /// long as the order stays where the shop moved it. Since then it has a key
    /// of its own, chosen under the owner's delegation of that day and recorded
    /// in `client_errors.t27`. The first wording said the order was already
    /// being handled, which is false when the shop rejected it or an earlier
    /// attempt cancelled it; the sentence was reworded the same day to speak of
    /// the app alone, and the last assertions below hold it there.
    #[test]
    fn a_refused_cancellation_is_told_its_own_sentence_and_not_the_fallback() {
        for lang in LANGS {
            let refused = t(lang, T_ORDER_DETAIL_CANCEL_REFUSED);
            // A refusal is a known outcome: whatever has been read since, the
            // line is the same, and it is coloured as not done.
            for since in SINCE {
                let line = order_cancel_line(lang, answered(order_cancel_answer(Some(409))), since)
                    .expect("a refused cancellation is always told");
                assert_eq!(line.text, refused, "{since:?}");
                assert_eq!(line.tone, OrderCancelTone::NotDone, "{since:?}");
            }
            // The old sentence is gone from this answer, and so is its advice.
            assert_ne!(refused, t(lang, T_API_ERR_UNKNOWN));
            // Only 409 gets it: every other status is told something else.
            for s in (100u16..=599).filter(|&s| s != 409) {
                assert_ne!(
                    text_of(lang, answered(order_cancel_answer(Some(s))), SINCE[2]),
                    refused,
                    "{s} is told the refused-cancellation sentence"
                );
            }
        }
        let ru = t(Lang::Russian, T_ORDER_DETAIL_CANCEL_REFUSED);
        let en = t(Lang::English, T_ORDER_DETAIL_CANCEL_REFUSED);
        assert_ne!(ru, en, "the sentence is not coming from the table");
        // No invitation to repeat an attempt that meets the same refusal.
        assert!(!ru.contains("позже"), "{ru}");
        assert!(!en.contains("later"), "{en}");
        // The next step it names is a person, and it is named in both locales.
        assert!(ru.contains("менеджер"), "{ru}");
        assert!(en.contains("manager"), "{en}");
        // It says what the app can no longer do, which is true of every 409.
        assert!(ru.contains("в приложении уже нельзя"), "{ru}");
        assert!(en.contains("can no longer be cancelled in the app"), "{en}");
        // And nothing about what the shop is doing with the order: the same 409
        // answers an order the shop took, one it rejected and one already
        // cancelled, so a claim that the order is being handled is false for
        // two of the three (the first wording of 2026-09-25 made it).
        for claim in ["в работе", "обрабат", "выполня"] {
            assert!(!ru.contains(claim), "{ru} claims the order is {claim}");
        }
        for claim in ["being handled", "in progress", "processing", "accepted"] {
            assert!(!en.contains(claim), "{en} claims the order is {claim}");
        }
    }

    #[test]
    fn the_cancel_surface_never_speaks_checkout_copy() {
        let mut progresses = vec![
            answered(OrderCancelAnswer::Cancelled),
            answered(OrderCancelAnswer::NoAnswer),
        ];
        progresses.extend((100u16..=599).map(|s| answered(order_cancel_answer(Some(s)))));
        for lang in LANGS {
            let checkout = [
                T_CHECKOUT_ERR_400,
                T_CHECKOUT_ERR_404,
                T_CHECKOUT_ERR_409,
                T_CHECKOUT_ERR_422,
            ]
            .map(|key| t(lang, key).to_string());
            for progress in &progresses {
                for since in SINCE {
                    let said = text_of(lang, *progress, since);
                    assert!(
                        !checkout.contains(&said),
                        "{progress:?} / {since:?} is told checkout copy: {said}"
                    );
                }
            }
        }
    }

    #[test]
    fn the_success_line_is_the_key_written_for_it() {
        let ru = text_of(
            Lang::Russian,
            answered(OrderCancelAnswer::Cancelled),
            SINCE[0],
        );
        let en = text_of(
            Lang::English,
            answered(OrderCancelAnswer::Cancelled),
            SINCE[0],
        );
        assert_eq!(ru, t(Lang::Russian, T_ORDER_DETAIL_CANCELLED_BY_USER));
        assert_eq!(en, t(Lang::English, T_ORDER_DETAIL_CANCELLED_BY_USER));
        assert_ne!(ru, en, "the success line is not coming from the table");
        for since in SINCE {
            let line =
                order_cancel_line(Lang::English, answered(OrderCancelAnswer::Cancelled), since)
                    .expect("a success is always told");
            assert_eq!(line.tone, OrderCancelTone::Done);
        }
    }

    #[test]
    fn no_answer_is_an_unknown_outcome_and_is_checked_before_it_is_repeated() {
        let unknown = answered(OrderCancelAnswer::NoAnswer);
        for lang in LANGS {
            // Nothing has been read since: the screen says it is reading, and
            // Confirm stays shut.
            let line = order_cancel_line(lang, unknown, StatusSinceAnswer::NotYetRead)
                .expect("an unknown outcome is told");
            assert_eq!(line.text, t(lang, T_SUCCESS_STATUS_LOADING));
            assert_eq!(line.tone, OrderCancelTone::Unknown);
            // The fresh read failed: still unknown, and still no second attempt.
            let line = order_cancel_line(lang, unknown, StatusSinceAnswer::Unreadable)
                .expect("an unreadable status is told");
            assert_eq!(line.text, t(lang, T_SUCCESS_STATUS_ERROR));
            assert_eq!(line.tone, OrderCancelTone::Unknown);
            // The fresh read shows the order still cancellable: the attempt did
            // not land, and trying again is now a checked repeat.
            let still = StatusSinceAnswer::Read {
                still_cancellable: true,
            };
            let line = order_cancel_line(lang, unknown, still).expect("a failed attempt is told");
            assert_eq!(line.text, t(lang, T_CHECKOUT_ERR_NETWORK));
            assert_eq!(line.tone, OrderCancelTone::NotDone);
            // The fresh read shows it has left pending: the status label in the
            // card is the answer, and nothing claims what THIS attempt did.
            let left = StatusSinceAnswer::Read {
                still_cancellable: false,
            };
            assert_eq!(order_cancel_line(lang, unknown, left), None);
            // An unknown outcome never reads as a success or as a refusal.
            let success = text_of(lang, answered(OrderCancelAnswer::Cancelled), SINCE[0]);
            let refusal = text_of(lang, answered(OrderCancelAnswer::Refused(409)), SINCE[0]);
            for since in [StatusSinceAnswer::NotYetRead, StatusSinceAnswer::Unreadable] {
                let said = text_of(lang, unknown, since);
                assert_ne!(said, success);
                assert_ne!(said, refusal);
            }
        }
        assert!(!order_cancel_dialog_closes(OrderCancelAnswer::NoAnswer));
        assert!(!order_cancel_may_send(
            unknown,
            StatusSinceAnswer::NotYetRead
        ));
        assert!(!order_cancel_may_send(
            unknown,
            StatusSinceAnswer::Unreadable
        ));
        assert!(!order_cancel_may_send(
            unknown,
            StatusSinceAnswer::Read {
                still_cancellable: false
            }
        ));
        assert!(order_cancel_may_send(
            unknown,
            StatusSinceAnswer::Read {
                still_cancellable: true
            }
        ));
    }

    /// A status of the server class leaves the outcome as unknown as no answer
    /// does: `cancel_order` answers one when its own commit fails, and the edge
    /// in front of the server can answer one after that commit landed. So it
    /// is checked before it is repeated, exactly like no answer, and its
    /// sentence -- the server is unavailable, try again -- is told only once a
    /// fresh reading shows the attempt did not land.
    #[test]
    fn a_server_failure_is_an_unknown_outcome_and_is_checked_before_it_is_repeated() {
        let still = StatusSinceAnswer::Read {
            still_cancellable: true,
        };
        let left = StatusSinceAnswer::Read {
            still_cancellable: false,
        };
        for s in [500u16, 502, 503, 504, 599] {
            let failed = answered(order_cancel_answer(Some(s)));
            assert!(
                !order_cancel_may_send(failed, StatusSinceAnswer::NotYetRead),
                "a {s} is sent again before the status is read"
            );
            assert!(
                !order_cancel_may_send(failed, StatusSinceAnswer::Unreadable),
                "a {s} is sent again although the fresh read failed"
            );
            assert!(
                !order_cancel_may_send(failed, left),
                "a {s} is sent again although the order has left pending"
            );
            assert!(
                order_cancel_may_send(failed, still),
                "a {s} whose fresh reading shows the order pending stays shut"
            );
            assert!(!order_cancel_dialog_closes(order_cancel_answer(Some(s))));
            for lang in LANGS {
                let line = order_cancel_line(lang, failed, StatusSinceAnswer::NotYetRead)
                    .expect("a server failure is told while it is checked");
                assert_eq!(line.text, t(lang, T_SUCCESS_STATUS_LOADING), "{s}");
                assert_eq!(line.tone, OrderCancelTone::Unknown, "{s}");
                let line = order_cancel_line(lang, failed, StatusSinceAnswer::Unreadable)
                    .expect("a failed check is told");
                assert_eq!(line.text, t(lang, T_SUCCESS_STATUS_ERROR), "{s}");
                assert_eq!(line.tone, OrderCancelTone::Unknown, "{s}");
                // The order has left pending: what THIS attempt did is unknown
                // for good, and the status label is the one true sentence.
                assert_eq!(order_cancel_line(lang, failed, left), None, "{s}");
                // The order is still pending: the attempt did not land, and the
                // general mapper's server sentence is now true of it.
                let line =
                    order_cancel_line(lang, failed, still).expect("a failed attempt is told");
                assert_eq!(line.text, friendly_response_error(lang, s), "{s}");
                assert_eq!(line.tone, OrderCancelTone::NotDone, "{s}");
            }
        }
    }

    #[test]
    fn nothing_is_sent_while_a_request_is_out_and_a_determined_answer_may_be_repeated() {
        for since in SINCE {
            assert!(!order_cancel_may_send(OrderCancelProgress::InFlight, since));
            assert!(order_cancel_may_send(OrderCancelProgress::Idle, since));
            // A refusal was given before anything was written: repeating it is
            // not blind.
            for s in [400u16, 401, 403, 404, 409, 429] {
                assert!(
                    order_cancel_may_send(answered(order_cancel_answer(Some(s))), since),
                    "{s} / {since:?}"
                );
            }
        }
    }

    #[test]
    fn nothing_is_told_before_an_answer() {
        for lang in LANGS {
            for since in SINCE {
                assert_eq!(
                    order_cancel_line(lang, OrderCancelProgress::Idle, since),
                    None
                );
                assert_eq!(
                    order_cancel_line(lang, OrderCancelProgress::InFlight, since),
                    None
                );
            }
        }
    }

    #[test]
    fn the_cancel_offer_is_withdrawn_once_the_server_confirms() {
        assert!(!order_cancel_offer_shown(answered(
            OrderCancelAnswer::Cancelled
        )));
        for progress in [
            OrderCancelProgress::Idle,
            OrderCancelProgress::InFlight,
            answered(OrderCancelAnswer::Refused(409)),
            answered(OrderCancelAnswer::ServerFailed(503)),
            answered(OrderCancelAnswer::NoAnswer),
        ] {
            assert!(order_cancel_offer_shown(progress), "{progress:?}");
        }
    }

    #[test]
    fn a_fresh_reading_is_read_from_what_landed() {
        assert_eq!(
            StatusSinceAnswer::from_landing(None, true),
            StatusSinceAnswer::NotYetRead
        );
        assert_eq!(
            StatusSinceAnswer::from_landing(Some(false), true),
            StatusSinceAnswer::Unreadable
        );
        assert_eq!(
            StatusSinceAnswer::from_landing(Some(true), true),
            StatusSinceAnswer::Read {
                still_cancellable: true
            }
        );
        assert_eq!(
            StatusSinceAnswer::from_landing(Some(true), false),
            StatusSinceAnswer::Read {
                still_cancellable: false
            }
        );
    }

    #[test]
    fn each_tone_has_its_own_colour() {
        let colours = [
            OrderCancelTone::Done.color(),
            OrderCancelTone::NotDone.color(),
            OrderCancelTone::Unknown.color(),
        ];
        assert_ne!(colours[0], colours[1]);
        assert_ne!(colours[1], colours[2]);
        assert_ne!(colours[0], colours[2]);
        for c in colours {
            assert!(c.starts_with('#') && c.len() == 7, "{c}");
        }
    }
}
