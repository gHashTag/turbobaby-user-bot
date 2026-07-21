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
    T_CHECKOUT_ERR_5XX,
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
            assert!(!should_retry_reauth(s), "status {s} must not trigger re-auth");
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
