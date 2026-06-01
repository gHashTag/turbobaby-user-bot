//! Friendly Russian error messages for the checkout HTTP response codes
//! returned by `POST /api/orders`.
//!
//! Cycle #65: before this, the checkout screen rendered raw "Ошибка
//! сервера: 403" for an auto-blocked user (cycle #60), and similar opaque
//! strings for every other failure mode. Each known status now maps to a
//! specific Russian sentence with an actionable next step ("свяжитесь с
//! поддержкой" / "обновите меню" / etc.). Unknown statuses fall back to
//! the raw code — better than silently lying about what happened.
//!
//! Lives in `trios/` (not under `ui/`) so it compiles for both backend
//! and wasm targets and can be unit-tested via plain `cargo test`.

/// Map an HTTP status code from `POST /api/orders` to a customer-friendly
/// Russian sentence. Pure — no IO, no allocations beyond the returned String.
pub fn friendly_order_error(status: u16) -> String {
    match status {
        400 => "Что-то не так с корзиной. Попробуйте очистить её и собрать заново.".to_string(),
        403 => "Аккаунт временно ограничен. Свяжитесь с поддержкой, чтобы продолжить заказы."
            .to_string(),
        404 => "Один из товаров больше не доступен. Обновите меню и попробуйте снова.".to_string(),
        409 => "Этот заказ уже создан. Откройте «Мои заказы» — он там.".to_string(),
        422 => "Цены или товары изменились с момента добавления в корзину. Обновите меню и оформите заказ заново.".to_string(),
        429 => "Слишком быстро. Подождите минуту и попробуйте снова.".to_string(),
        500..=599 => "Сервер сейчас недоступен. Попробуйте через минуту.".to_string(),
        _ => format!("Ошибка сервера: HTTP {}", status),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_403_to_account_restricted_phrase() {
        // The whole point of cycle #65 — auto-blocked users from cycle #60
        // must see something they can act on, not just "HTTP 403".
        let s = friendly_order_error(403);
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
        let s = friendly_order_error(429);
        assert!(s.contains("Слишком быстро") || s.contains("минут"));
        assert!(!s.contains("429"));
    }

    #[test]
    fn maps_422_to_price_change_hint() {
        // 422 from cycle #56-#58 price authority means catalog drift between
        // menu load and submit. Customer should be told to refresh, not
        // stare at a generic error.
        let s = friendly_order_error(422);
        assert!(s.contains("Цен") || s.contains("товар"));
        assert!(s.contains("Обновите"));
    }

    #[test]
    fn maps_400_to_cart_corruption_hint() {
        let s = friendly_order_error(400);
        assert!(s.contains("корзин"));
    }

    #[test]
    fn maps_409_to_duplicate_hint() {
        let s = friendly_order_error(409);
        assert!(s.contains("Мои заказы") || s.contains("уже создан"));
    }

    #[test]
    fn maps_5xx_range_to_server_unavailable_phrase() {
        for status in [500u16, 502, 503, 504, 511, 599] {
            let s = friendly_order_error(status);
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
    fn maps_unknown_status_to_raw_code_fallback() {
        // Better to show "HTTP 999" than silently say nothing when we
        // genuinely don't know what happened.
        let s = friendly_order_error(999);
        assert!(s.contains("999"));
    }

    #[test]
    fn includes_actionable_next_step_for_every_known_code() {
        // Heuristic: every known code's message points the user at *something*
        // they can do (cart, menu, support, орденс, minute). This catches
        // future regressions where someone shortens a message to "Ошибка"
        // and removes the recovery hint.
        for status in [400u16, 403, 404, 409, 422, 429, 500] {
            let s = friendly_order_error(status);
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
}
