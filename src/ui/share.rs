//! Product sharing helpers for Telegram Mini App deep links.
//!
//! Any sellable item can be shared as `https://t.me/{bot}?startapp=p_{kind}_{id}`.
//! The recipient's Mini App reads `Telegram.WebApp.initDataUnsafe.start_param`,
//! navigates to the right catalog screen and opens the product detail modal.
//!
//! Share UI uses Telegram's native picker via `t.me/share/url`, opened with
//! `Telegram.WebApp.openTelegramLink` so it stays inside Telegram instead of
//! falling back to an external browser.

use crate::trios::i18n::{tf, T_SHARE_MESSAGE};
use crate::ui::lang::current_lang;
use crate::ui::routes::Route;

/// Hard-coded bot username used in t.me deep links and share URLs.
/// Matches `Config::bot_username` default and the existing referral links.
const BOT_USERNAME: &str = "Woody_WeedPecker_bot";

/// Maximum length Telegram allows for `startapp` parameter.
const MAX_START_PARAM_LEN: usize = 64;

/// Product catalog kinds that support sharing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProductKind {
    Strain,
    Accessory,
    Set,
    Tea,
    Event,
}

impl ProductKind {
    /// Payload prefix used in `startapp`.
    pub fn payload_prefix(self) -> &'static str {
        match self {
            Self::Strain => "p_strain",
            Self::Accessory => "p_acc",
            Self::Set => "p_set",
            Self::Tea => "p_tea",
            Self::Event => "p_event",
        }
    }

    /// Route the recipient should land on.
    pub fn route(self) -> Route {
        match self {
            Self::Strain => Route::Menu {},
            Self::Accessory => Route::Accessories {},
            Self::Set => Route::Sets {},
            Self::Tea => Route::Tea {},
            Self::Event => Route::Events {},
        }
    }
}

/// A product that was requested via a deep link and is waiting to be shown.
#[derive(Clone, Debug, PartialEq)]
pub struct SharedProduct {
    pub kind: ProductKind,
    pub id: String,
}

/// Parse a `startapp` value into the product it references.
///
/// Format: `{prefix}_{id}`. Unknown prefixes and payloads that are too long
/// are rejected so malformed links degrade gracefully.
pub fn parse_start_param(param: &str) -> Option<SharedProduct> {
    if param.is_empty() || param.len() > MAX_START_PARAM_LEN {
        return None;
    }
    // Only allow safe characters: alphanumerics, underscores, hyphens.
    if !param
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    {
        return None;
    }

    // The payload format is `{prefix}_{id}`. Prefixes themselves contain an
    // underscore (e.g. "p_set"), so split_once('_') on the first underscore
    // would mis-parse "p_set_xxx" into prefix "p" and id "set_xxx". Use
    // strip_prefix instead, then validate the remaining id.
    let (kind, id) = if let Some(id) = param.strip_prefix("p_strain_") {
        (ProductKind::Strain, id)
    } else if let Some(id) = param.strip_prefix("p_acc_") {
        (ProductKind::Accessory, id)
    } else if let Some(id) = param.strip_prefix("p_set_") {
        (ProductKind::Set, id)
    } else if let Some(id) = param.strip_prefix("p_tea_") {
        (ProductKind::Tea, id)
    } else if let Some(id) = param.strip_prefix("p_event_") {
        (ProductKind::Event, id)
    } else {
        return None;
    };

    if id.is_empty() {
        return None;
    }
    Some(SharedProduct {
        kind,
        id: id.to_string(),
    })
}

/// Runtime bot username. On WASM reads from Telegram initData so test/staging
/// bots do not accidentally emit production links; falls back to the
/// compile-time constant when the WebApp SDK is unavailable.
fn bot_username() -> String {
    #[cfg(target_arch = "wasm32")]
    {
        if let Some(name) = crate::ui::telegram::TelegramApp::init().bot_username() {
            return name;
        }
    }
    BOT_USERNAME.to_string()
}

/// Build the `t.me` deep link that opens the mini-app with this product.
fn deep_link_url(kind: ProductKind, id: &str) -> String {
    format!(
        "https://t.me/{}?startapp={}_{}",
        bot_username(),
        kind.payload_prefix(),
        id
    )
}

/// Public accessor for the raw `t.me` deep-link URL.
///
/// Useful when callers want to display or copy the link without opening the
/// native share picker (e.g. a share icon on a home-screen card).
pub fn product_deep_link(kind: ProductKind, id: &str) -> String {
    deep_link_url(kind, id)
}

/// Build a `startapp` parameter that opens the Mini App on the order detail
/// screen. Format: `o_{order_id}`. Order IDs are UUID-like, so the prefix keeps
/// them disjoint from product shares.
pub fn order_start_param(order_id: &str) -> String {
    format!("o_{}", order_id)
}

/// Raw `t.me` deep link that opens a specific order.
pub fn order_deep_link(order_id: &str) -> String {
    format!(
        "https://t.me/{}?startapp={}",
        bot_username(),
        order_start_param(order_id)
    )
}

/// Build a `startapp` parameter that opens the Mini App on the cart screen.
pub fn cart_start_param() -> String {
    "cart".to_string()
}

/// Raw `t.me` deep link that opens the cart.
pub fn cart_deep_link() -> String {
    format!(
        "https://t.me/{}?startapp={}",
        bot_username(),
        cart_start_param()
    )
}

/// Parse an order `startapp` value (`o_{order_id}`). Unknown prefixes and
/// payloads that are too long are rejected so malformed links degrade gracefully.
pub fn parse_order_start_param(param: &str) -> Option<String> {
    if param.is_empty() || param.len() > MAX_START_PARAM_LEN {
        return None;
    }
    if !param
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    {
        return None;
    }
    param.strip_prefix("o_").map(|id| id.to_string())
}

/// Parse a cart `startapp` value (`cart`).
pub fn parse_cart_start_param(param: &str) -> bool {
    param == "cart"
}

/// Open a `t.me` URL using Telegram's native method, falling back to a plain
/// browser open if the WebApp SDK is unavailable.
pub fn open_telegram_link(url: &str) {
    #[cfg(target_arch = "wasm32")]
    {
        use wasm_bindgen::prelude::*;
        use wasm_bindgen::JsCast;
        if let Some(window) = web_sys::window() {
            if let Ok(tg) = js_sys::Reflect::get(&window, &JsValue::from_str("Telegram")) {
                if let Ok(webapp) = js_sys::Reflect::get(&tg, &JsValue::from_str("WebApp")) {
                    let func =
                        js_sys::Reflect::get(&webapp, &JsValue::from_str("openTelegramLink"))
                            .ok()
                            .and_then(|f| f.dyn_into::<js_sys::Function>().ok());
                    if let Some(f) = func {
                        let _ = f.call1(&webapp, &JsValue::from_str(url));
                        return;
                    }
                }
            }
            let _ = window.open_with_url_and_target(url, "_blank");
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = url;
    }
}

/// Open Telegram's native share picker for a product.
///
/// `name` should be the localized product name; the share text is localized
/// automatically via `T_SHARE_MESSAGE`.
pub fn share_product(kind: ProductKind, id: &str, name: &str) {
    let link = deep_link_url(kind, id);
    let lang = current_lang();
    let text = tf(lang, T_SHARE_MESSAGE, &[name.to_string()]);
    let share_url = format!(
        "https://t.me/share/url?url={}&text={}",
        urlencoding::encode(&link),
        urlencoding::encode(&text)
    );
    open_telegram_link(&share_url);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_valid_strain_payload() {
        let p = parse_start_param("p_strain_abc123").unwrap();
        assert_eq!(p.kind, ProductKind::Strain);
        assert_eq!(p.id, "abc123");
    }

    #[test]
    fn parse_valid_set_payload_with_uuid_like_id() {
        let p = parse_start_param("p_set_550e8400-e29b-41d4-a716-446655440000").unwrap();
        assert_eq!(p.kind, ProductKind::Set);
        assert_eq!(p.id, "550e8400-e29b-41d4-a716-446655440000");
    }

    #[test]
    fn parse_rejects_ambiguous_single_underscore_prefix() {
        // "p_set_xxx" must NOT be mis-parsed as prefix "p" + id "set_xxx".
        assert!(parse_start_param("p_set_123").is_some());
        let p = parse_start_param("p_set_123").unwrap();
        assert_eq!(p.kind, ProductKind::Set);
        assert_eq!(p.id, "123");
    }

    #[test]
    fn parse_rejects_unknown_prefix() {
        assert!(parse_start_param("p_unknown_123").is_none());
    }

    #[test]
    fn parse_rejects_too_long_payload() {
        let long = format!("p_strain_{}", "x".repeat(80));
        assert!(parse_start_param(&long).is_none());
    }

    #[test]
    fn deep_link_contains_startapp() {
        let url = deep_link_url(ProductKind::Tea, "t42");
        assert!(url.contains("startapp=p_tea_t42"));
        assert!(url.starts_with("https://t.me/Woody_WeedPecker_bot"));
    }

    #[test]
    fn product_deep_link_public_wrapper_matches_internal() {
        let via_public =
            product_deep_link(ProductKind::Set, "550e8400-e29b-41d4-a716-446655440000");
        let via_internal = deep_link_url(ProductKind::Set, "550e8400-e29b-41d4-a716-446655440000");
        assert_eq!(via_public, via_internal);
    }

    #[test]
    fn all_catalog_kinds_round_trip_through_parse() {
        let cases = [
            (ProductKind::Strain, "strain-42"),
            (ProductKind::Accessory, "acc-99"),
            (ProductKind::Set, "set-007"),
            (ProductKind::Tea, "tea-chai"),
            (ProductKind::Event, "evt-42"),
        ];
        for (kind, id) in cases {
            let link = product_deep_link(kind, id);
            let start_param = link.split("startapp=").nth(1).unwrap();
            let parsed = parse_start_param(start_param).unwrap();
            assert_eq!(parsed.kind, kind);
            assert_eq!(parsed.id, id);
        }
    }

    #[test]
    fn parse_event_payload() {
        let p = parse_start_param("p_event_550e8400-e29b-41d4-a716-446655440000").unwrap();
        assert_eq!(p.kind, ProductKind::Event);
        assert_eq!(p.id, "550e8400-e29b-41d4-a716-446655440000");
    }

    #[test]
    fn parse_valid_order_payload() {
        let id = parse_order_start_param("o_550e8400-e29b-41d4-a716-446655440000").unwrap();
        assert_eq!(id, "550e8400-e29b-41d4-a716-446655440000");
    }

    #[test]
    fn parse_order_rejects_missing_prefix() {
        assert!(parse_order_start_param("550e8400-e29b-41d4-a716-446655440000").is_none());
    }

    #[test]
    fn order_deep_link_round_trips() {
        let id = "abc-123";
        let url = order_deep_link(id);
        let start_param = url.split("startapp=").nth(1).unwrap();
        assert_eq!(parse_order_start_param(start_param).unwrap(), id);
    }
}
