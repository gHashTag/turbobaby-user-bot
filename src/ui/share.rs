//! Product sharing helpers for Telegram Mini App deep links.
//!
//! Any sellable item can be shared as `https://t.me/{bot}?start=p_{kind}_{id}`.
//!
//! `?start=` (a classic bot deep link), not `?startapp=`: the latter only
//! launches the Mini App when the bot has a **Main Mini App** configured in
//! BotFather, otherwise Telegram just opens the bot chat — which is why a
//! reposted card used to land on the bot's home screen. With `?start=` the bot
//! always receives the payload and answers with a Mini App button that carries
//! it through (see `src/bot/commands.rs`).
//! The recipient's Mini App reads `Telegram.WebApp.initDataUnsafe.start_param`,
//! navigates to the right catalog screen and opens the product detail modal.
//!
//! Share UI uses Telegram's native picker via `t.me/share/url`, opened with
//! `Telegram.WebApp.openTelegramLink` so it stays inside Telegram instead of
//! falling back to an external browser.

use crate::trios::i18n::{
    tf, T_GARDEN_SHARE_TEXT, T_GARDEN_SHARE_TITLE, T_SHARE_MESSAGE,
};
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
        "https://t.me/{}?start={}_{}",
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
        "https://t.me/{}?start={}",
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
        "https://t.me/{}?start={}",
        bot_username(),
        cart_start_param()
    )
}

/// Deep-link target for a single order (`o_{order_id}`).
///
/// A newtype, not a bare `Option<String>`: Dioxus keys context by type, so
/// providing two `Signal<Option<String>>` contexts in the same scope made the
/// second one (reorder) shadow the first, and order deep links silently
/// resolved to the reorder signal.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PendingOrder(pub Option<String>);

/// Deep-link target for a reorder (`reorder__{order_id}`). See [`PendingOrder`]
/// for why this is a distinct newtype.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PendingReorder(pub Option<String>);

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

/// Parse a cart `startapp` value. Accepts plain `cart` or attribution
/// variants like `cart__<source>` so we can track which campaign drove the
/// open. The source segment must be safe characters only.
pub fn parse_cart_start_param(param: &str) -> Option<&str> {
    if param == "cart" {
        return Some("");
    }
    param.strip_prefix("cart__").filter(|s| {
        !s.is_empty()
            && s.len() <= 50
            && s.bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    })
}

/// Build a `startapp` parameter that opens the Mini App and pre-loads the
/// customer's last order into their cart for one-tap reorder.
/// Format: `reorder__{order_id}`.
pub fn reorder_start_param(order_id: &str) -> String {
    format!("reorder__{}", order_id)
}

/// Raw `t.me` deep link that opens the Mini App in reorder mode.
pub fn reorder_deep_link(order_id: &str) -> String {
    format!(
        "https://t.me/{}?start={}",
        bot_username(),
        reorder_start_param(order_id)
    )
}

/// Build a `startapp` parameter that opens the Mini App on the garden screen.
/// When `referrer_id` is provided, the invitee can be attributed back to the
/// referrer for a two-sided garden-referral reward.
pub fn garden_start_param(referrer_id: Option<i64>) -> String {
    match referrer_id {
        Some(id) => format!("garden__{}", id),
        None => "garden".to_string(),
    }
}

/// Raw `t.me` deep link that opens the Mini App in the garden.
pub fn garden_deep_link(referrer_id: Option<i64>) -> String {
    format!(
        "https://t.me/{}?start={}",
        bot_username(),
        garden_start_param(referrer_id)
    )
}

/// Parse a garden `startapp` value. Plain `garden` returns no referrer.
/// `garden__{id}[__{source}]` returns the referrer id and optional source.
/// The id segment must be a valid i64 and the optional source is safe chars only.
pub fn parse_garden_start_param(param: &str) -> Option<(Option<i64>, Option<&str>)> {
    if param.is_empty() || param.len() > MAX_START_PARAM_LEN {
        return None;
    }
    if !param
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    {
        return None;
    }
    if param == "garden" {
        return Some((None, None));
    }
    let rest = param.strip_prefix("garden__")?;
    if rest.is_empty() {
        return None;
    }
    // Split optional UTM source: garden__123__utm_a
    let (id_part, source) = match rest.find("__") {
        Some(idx) => (&rest[..idx], Some(&rest[idx + 2..])),
        None => (rest, None),
    };
    if let Some(s) = source {
        if s.is_empty()
            || s.len() > 50
            || !s
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
        {
            return None;
        }
    }
    let referrer_id = id_part.parse::<i64>().ok()?;
    if referrer_id <= 0 {
        return None;
    }
    Some((Some(referrer_id), source))
}

/// Open Telegram's native share picker for the garden.
/// `referrer_id` is encoded so invitees are attributed. `source` is an optional
/// A/B variant (e.g. "utm_a") logged on the server when the invite is accepted.
pub fn share_garden(referrer_id: Option<i64>, source: Option<&str>) {
    let mut link = garden_deep_link(referrer_id);
    if let Some(s) = source {
        // The parser expects garden__{id}__{source}; append source if present.
        if referrer_id.is_some() {
            link.push_str("__");
            link.push_str(s);
        }
    }
    let lang = current_lang();
    let text = format!(
        "{}\n{}",
        tf(lang, T_GARDEN_SHARE_TITLE, &[]),
        tf(lang, T_GARDEN_SHARE_TEXT, &[])
    );
    let share_url = format!(
        "https://t.me/share/url?url={}&text={}",
        urlencoding::encode(&link),
        urlencoding::encode(&text)
    );
    open_telegram_link(&share_url);
}

/// Parse a reorder `startapp` value (`reorder__{order_id}`). Unknown prefixes
/// and payloads that are too long are rejected so malformed links degrade
/// gracefully.
pub fn parse_reorder_start_param(param: &str) -> Option<String> {
    if param.is_empty() || param.len() > MAX_START_PARAM_LEN {
        return None;
    }
    if !param
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    {
        return None;
    }
    param.strip_prefix("reorder__").map(|id| id.to_string())
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
        assert!(url.contains("start=p_tea_t42"));
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
            let start_param = link.split("start=").nth(1).unwrap();
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
        let start_param = url.split("start=").nth(1).unwrap();
        assert_eq!(parse_order_start_param(start_param).unwrap(), id);
    }

    #[test]
    fn parse_valid_reorder_payload() {
        let id =
            parse_reorder_start_param("reorder__550e8400-e29b-41d4-a716-446655440000").unwrap();
        assert_eq!(id, "550e8400-e29b-41d4-a716-446655440000");
    }

    #[test]
    fn reorder_deep_link_round_trips() {
        let id = "abc-123";
        let url = reorder_deep_link(id);
        let start_param = url.split("start=").nth(1).unwrap();
        assert_eq!(parse_reorder_start_param(start_param).unwrap(), id);
    }

    #[test]
    fn garden_start_param_plain() {
        assert_eq!(garden_start_param(None), "garden");
    }

    #[test]
    fn garden_start_param_with_referrer() {
        assert_eq!(garden_start_param(Some(12345)), "garden__12345");
    }

    #[test]
    fn parse_garden_plain() {
        assert_eq!(parse_garden_start_param("garden"), Some((None, None)));
    }

    #[test]
    fn parse_garden_with_referrer() {
        assert_eq!(
            parse_garden_start_param("garden__12345"),
            Some((Some(12345), None))
        );
    }

    #[test]
    fn parse_garden_with_referrer_and_source() {
        assert_eq!(
            parse_garden_start_param("garden__12345__utm_a"),
            Some((Some(12345), Some("utm_a")))
        );
    }

    #[test]
    fn parse_garden_rejects_non_positive_id() {
        assert!(parse_garden_start_param("garden__0").is_none());
        assert!(parse_garden_start_param("garden__-5").is_none());
    }

    #[test]
    fn parse_garden_rejects_malformed_source() {
        assert!(parse_garden_start_param("garden__12345__").is_none());
        assert!(parse_garden_start_param("garden__12345__utm!").is_none());
    }

    #[test]
    fn garden_deep_link_round_trips() {
        let url = garden_deep_link(Some(12345));
        let start_param = url.split("start=").nth(1).unwrap();
        assert_eq!(
            parse_garden_start_param(start_param),
            Some((Some(12345), None))
        );
    }
}
