//! Friendly localised error messages for the checkout HTTP response codes
//! returned by `POST /api/orders`.
//!
//! Cycle #65 introduced the helper with Russian strings hardcoded inline.
//! Cycle #69 moved the copy into `trios::i18n` keys so adding English /
//! Thai / etc. is one table edit instead of forking the function. The
//! caller now passes its current `Lang`; rendering goes through the
//! standard `t(lang, key)` lookup with English fallback for locales that
//! haven't been translated yet.
//!
//! Unknown HTTP statuses keep their inline `format!` fallback — routing
//! arbitrary integers through the static key table is more machinery
//! than the rare "we got HTTP 999" path warrants.

use super::core::Lang;
use super::i18n::{
    t, T_CHECKOUT_ERR_400, T_CHECKOUT_ERR_403, T_CHECKOUT_ERR_404, T_CHECKOUT_ERR_409,
    T_CHECKOUT_ERR_422, T_CHECKOUT_ERR_429, T_CHECKOUT_ERR_5XX, T_CHECKOUT_ERR_AGE_NOT_CONFIRMED,
    T_CHECKOUT_ERR_ZONE_INVALID,
};

/// Map an HTTP status code from `POST /api/orders` to a customer-friendly
/// localised sentence. Pure — looks up via the shared i18n table.
pub fn friendly_order_error(lang: Lang, status: u16) -> String {
    let key = match status {
        400 => T_CHECKOUT_ERR_400,
        403 => T_CHECKOUT_ERR_403,
        404 => T_CHECKOUT_ERR_404,
        409 => T_CHECKOUT_ERR_409,
        422 => T_CHECKOUT_ERR_422,
        429 => T_CHECKOUT_ERR_429,
        500..=599 => T_CHECKOUT_ERR_5XX,
        _ => return format!("Ошибка сервера: HTTP {}", status),
    };
    t(lang, key).to_string()
}

/// Map a stable `error` code from the JSON body of a failed checkout response
/// to a customer-friendly localised sentence. Used for per-field 422 codes
/// such as `age_not_confirmed` or `zone_invalid` that the generic status
/// mapper would otherwise collapse into the price-change hint.
pub fn friendly_order_error_code(lang: Lang, code: &str) -> Option<String> {
    let key = match code {
        "age_not_confirmed" => T_CHECKOUT_ERR_AGE_NOT_CONFIRMED,
        "zone_invalid" => T_CHECKOUT_ERR_ZONE_INVALID,
        _ => return None,
    };
    Some(t(lang, key).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ru(status: u16) -> String {
        friendly_order_error(Lang::Russian, status)
    }

    fn ru_code(code: &str) -> Option<String> {
        friendly_order_error_code(Lang::Russian, code)
    }

    #[test]
    fn maps_403_to_account_restricted_phrase() {
        // The whole point of cycle #65 — auto-blocked users from cycle #60
        // must see something they can act on, not just "HTTP 403".
        let s = ru(403);
        assert!(
            s.contains("ограничен"),
            "403 should mention account being restricted, got: {}",
            s
        );
        assert!(
            s.contains("поддержкой"),
            "403 should point user at support, got: {}",
            s
        );
        // Negative check: no raw status leak.
        assert!(!s.contains("403"), "403 raw code must not leak: {}", s);
    }

    #[test]
    fn maps_429_to_rate_limit_phrase() {
        let s = ru(429);
        assert!(s.contains("Слишком быстро") || s.contains("минут"));
        assert!(!s.contains("429"));
    }

    #[test]
    fn maps_422_to_price_change_hint() {
        let s = ru(422);
        assert!(s.contains("Цен") || s.contains("товар"));
        assert!(s.contains("Обновите"));
    }

    #[test]
    fn maps_400_to_cart_corruption_hint() {
        let s = ru(400);
        assert!(s.contains("корзин"));
    }

    #[test]
    fn maps_409_to_duplicate_hint() {
        let s = ru(409);
        assert!(s.contains("Мои заказы") || s.contains("уже создан"));
    }

    #[test]
    fn maps_5xx_range_to_server_unavailable_phrase() {
        for status in [500u16, 502, 503, 504, 511, 599] {
            let s = ru(status);
            assert!(
                s.contains("недоступен") || s.contains("Сервер"),
                "5xx ({}) should mention server unavailable, got: {}",
                status,
                s
            );
            assert!(
                !s.contains(&status.to_string()),
                "5xx ({}) must not leak raw code in friendly message, got: {}",
                status,
                s
            );
        }
    }

    #[test]
    fn maps_age_not_confirmed_code_to_sentence() {
        let s = ru_code("age_not_confirmed").expect("known code should map");
        assert!(
            s.contains("20") || s.contains("20+"),
            "should mention age requirement: {s}"
        );
        assert!(
            !s.contains("price"),
            "should not be the generic 422 hint: {s}"
        );
    }

    #[test]
    fn maps_zone_invalid_code_to_sentence() {
        let s = ru_code("zone_invalid").expect("known code should map");
        assert!(
            s.contains("район") || s.contains("зон"),
            "should mention delivery zone: {s}"
        );
    }

    #[test]
    fn returns_none_for_unknown_error_code() {
        assert!(ru_code("totally_unknown_code").is_none());
    }

    #[test]
    fn unknown_status_to_raw_code_fallback() {
        // Better to show "HTTP 999" than silently say nothing when we
        // genuinely don't know what happened.
        let s = ru(999);
        assert!(s.contains("999"));
    }

    #[test]
    fn includes_actionable_next_step_for_every_known_code() {
        // Heuristic: every known code's message points the user at *something*
        // they can do (cart, menu, support, заказы, minute). Catches future
        // regressions where someone shortens a message to "Ошибка" and removes
        // the recovery hint.
        for status in [400u16, 403, 404, 409, 422, 429, 500] {
            let s = ru(status);
            let has_action = ["корзин", "поддерж", "Обновите", "Мои заказы", "минут"]
                .iter()
                .any(|kw| s.contains(kw));
            assert!(
                has_action,
                "status {} must include an actionable next step, got: {}",
                status, s
            );
        }
    }

    // ── i18n surface (cycle #69) ─────────────────────────────────────────

    #[test]
    fn english_locale_renders_for_403() {
        // Sanity: the EN table actually got wired up. We don't need to test
        // every code — that would duplicate the Russian table — but at
        // least one key must round-trip through Lang::English so a future
        // typo in the EN match arm gets caught.
        let s = friendly_order_error(Lang::English, 403);
        assert!(s.contains("restricted"));
        assert!(s.contains("support"));
        // No Cyrillic leakage from the RU fallback.
        assert!(!s.contains("ограничен"));
    }

    #[test]
    fn unknown_status_renders_same_text_regardless_of_lang() {
        // The fallback path bypasses i18n, so Russian and English produce
        // identical output. Pin that explicitly so a future refactor that
        // adds per-lang fallback doesn't silently change behaviour.
        let ru_msg = friendly_order_error(Lang::Russian, 999);
        let en_msg = friendly_order_error(Lang::English, 999);
        assert_eq!(ru_msg, en_msg);
    }
}
