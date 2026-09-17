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

use crate::trios::i18n::{tf, T_SHARE_MESSAGE};
use crate::ui::lang::current_lang;
use crate::ui::routes::Route;

/// Hard-coded bot username used in t.me deep links and share URLs.
/// Matches `Config::bot_username` default and the existing referral links.
const BOT_USERNAME: &str = "turboagent_phuket_bot";

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

    /// This kind, as the shared vocabulary spells it.
    pub fn to_core(self) -> crate::trios::deeplink::Kind {
        match self {
            Self::Strain => crate::trios::deeplink::Kind::Strain,
            Self::Accessory => crate::trios::deeplink::Kind::Accessory,
            Self::Set => crate::trios::deeplink::Kind::Set,
            Self::Tea => crate::trios::deeplink::Kind::Tea,
            Self::Event => crate::trios::deeplink::Kind::Event,
        }
    }

    /// The same kind, as the shared vocabulary spells it.
    pub fn from_core(kind: crate::trios::deeplink::Kind) -> Self {
        match kind {
            crate::trios::deeplink::Kind::Strain => Self::Strain,
            crate::trios::deeplink::Kind::Accessory => Self::Accessory,
            crate::trios::deeplink::Kind::Set => Self::Set,
            crate::trios::deeplink::Kind::Tea => Self::Tea,
            crate::trios::deeplink::Kind::Event => Self::Event,
        }
    }

    /// Route the recipient should land on.
    ///
    /// Every one of these kinds is a cannabis-era product line, and none of
    /// them has anything left to show: 083 drops the tables and 085 unpublishes
    /// what survived. Until 2026-09-16 four of the five still named their own
    /// retired screen, and only `Strain` had been repointed at the fleet.
    ///
    /// That asymmetry is worse than it looks, because a deep link is not a tap.
    /// The customer did not choose to be here — a link arrived in Telegram,
    /// they opened it, and the app decided where to put them. Landing them on a
    /// permanently empty grid for a product line the shop no longer sells is a
    /// dead end with no back button; the fleet is the honest destination and
    /// the only screen that can still answer "what can I get?".
    ///
    /// `screens/mod.rs` keeps rendering the retired screens for anyone who
    /// types the path — this is about where the app *sends* people (D19: the
    /// heritage is hidden, not deleted).
    pub fn route(self) -> Route {
        match self {
            Self::Strain | Self::Accessory | Self::Set | Self::Tea | Self::Event => Route::Menu {},
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
    // One vocabulary, in `trios::deeplink`, which the host test suite can see.
    // This file is `#[cfg(target_arch = "wasm32")]`, so anything decided here
    // is decided where no test on the host can reach it — and the bot kept a
    // second list of the same prefixes that disagreed with this one about
    // twelve payloads.
    match crate::trios::deeplink::parse(param)? {
        // `source` is deliberately dropped here: it says which campaign
        // published the link, and the screen only needs to know which card to
        // open. It is reported separately, in `app.rs`, as a client event.
        crate::trios::deeplink::Target::Product { kind, id, .. } => Some(SharedProduct {
            kind: ProductKind::from_core(kind),
            id,
        }),
        _ => None,
    }
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
    let payload = crate::trios::deeplink::payload_for(&crate::trios::deeplink::Target::Product {
        kind: kind.to_core(),
        id: id.to_string(),
        // A customer sharing a card is not a campaign, and counting it as one
        // would put the promoter's numbers in with ordinary word of mouth.
        source: None,
    });
    match payload {
        Some(p) => format!("https://t.me/{}?start={}", bot_username(), p),
        // An id Telegram cannot carry. Send them to the shop rather than to a
        // link that opens the home screen and looks like a broken share.
        None => format!("https://t.me/{}", bot_username()),
    }
}

/// Public accessor for the raw `t.me` deep-link URL.
///
/// Useful when callers want to display or copy the link without opening the
/// native share picker (e.g. a share icon on a home-screen card).
pub fn product_deep_link(kind: ProductKind, id: &str) -> String {
    deep_link_url(kind, id)
}

// Every payload below is built by `crate::trios::deeplink::payload_for`, not
// spelled out here.
//
// The shapes used to be written by hand in this file, and this file's
// `#[cfg(test)] mod tests` — 22 assertions, several of them about exactly
// these strings — runs **none of them**, because `src/ui` is
// `#[cfg(target_arch = "wasm32")]` and `cargo test` never reaches it. The
// parser was already shared; the builder was not, so a drift between
// `garden__<id>` and `garden_<id>` would have shipped green.
//
// `payload_for` returns `None` for something Telegram cannot carry. These
// wrappers keep their `String` return so no caller changes, and fall back to
// the plain screen — a link to the cart is better than a link to nothing.
//
// (This was `///` until the wasm lib was first compiled with `-D warnings`:
// a doc comment followed by a blank line documents nothing, and clippy's
// `empty_line_after_doc_comments` is deny-by-default here. It is a section
// note about the group, not the docs of any one function, so `//` is correct.)

/// Build a `startapp` parameter that opens the Mini App on the order detail
/// screen. Format: `o_{order_id}`.
pub fn order_start_param(order_id: &str) -> String {
    crate::trios::deeplink::payload_for(&crate::trios::deeplink::Target::Order(
        order_id.to_string(),
    ))
    .unwrap_or_else(|| "cart".to_string())
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
    crate::trios::deeplink::payload_for(&crate::trios::deeplink::Target::Cart {
        source: String::new(),
    })
    .unwrap_or_else(|| "cart".to_string())
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
    match crate::trios::deeplink::parse(param)? {
        crate::trios::deeplink::Target::Order(id) => Some(id),
        _ => None,
    }
}

/// Parse a cart `startapp` value. Accepts plain `cart` or attribution
/// variants like `cart__<source>` so we can track which campaign drove the
/// open. The source segment must be safe characters only.
pub fn parse_cart_start_param(param: &str) -> Option<&str> {
    // The decision comes from the shared vocabulary; the borrow stays here so
    // the callers keep their signature.
    match crate::trios::deeplink::parse(param)? {
        crate::trios::deeplink::Target::Cart { .. } => {
            Some(param.strip_prefix("cart__").unwrap_or(""))
        }
        _ => None,
    }
}

/// Build a `startapp` parameter that opens the Mini App and pre-loads the
/// customer's last order into their cart for one-tap reorder.
/// Format: `reorder__{order_id}`.
pub fn reorder_start_param(order_id: &str) -> String {
    crate::trios::deeplink::payload_for(&crate::trios::deeplink::Target::Reorder(
        order_id.to_string(),
    ))
    .unwrap_or_else(|| "cart".to_string())
}

/// Raw `t.me` deep link that opens the Mini App in reorder mode.
pub fn reorder_deep_link(order_id: &str) -> String {
    format!(
        "https://t.me/{}?start={}",
        bot_username(),
        reorder_start_param(order_id)
    )
}

// `garden_start_param`, `garden_deep_link`, `parse_garden_start_param` and
// `share_garden` stood here — four functions whose whole job was to build and
// read a link to `/garden`. The route is gone (D5), so such a link could only
// land nowhere. Nothing replaces them: the link a customer shares to invite a
// friend is `?start=ref_<code>`, which the bot has always handled and which
// the referrals page already renders.

/// Parse a reorder `startapp` value (`reorder__{order_id}`). Unknown prefixes
/// and payloads that are too long are rejected so malformed links degrade
/// gracefully.
pub fn parse_reorder_start_param(param: &str) -> Option<String> {
    match crate::trios::deeplink::parse(param)? {
        crate::trios::deeplink::Target::Reorder(id) => Some(id),
        _ => None,
    }
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

/// Wire name for `kind`, matching `api::share::ShareKind::parse`.
fn share_kind_wire(kind: ProductKind) -> &'static str {
    match kind {
        ProductKind::Strain => "strain",
        ProductKind::Accessory => "accessory",
        ProductKind::Set => "set",
        ProductKind::Tea => "tea",
        ProductKind::Event => "event",
    }
}

/// Fallback share: hands Telegram a plain link.
///
/// Telegram previews that link, and the link points at the bot — so the
/// recipient sees the bot's own profile card, not the product. Only used when
/// the rich path is unavailable (old client, server refusal).
fn share_product_link_only(kind: ProductKind, id: &str, name: &str) {
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

/// Share a product as a real Telegram card: photo, name, price, description
/// and an "Open product" button that deep-links back to this exact item.
///
/// Asks the server to build the card (`POST /api/share/prepare`) and hands the
/// returned `prepared_message_id` to `WebApp.shareMessage()`. The card content
/// is built from the database, so a shared price is always the real one.
///
/// Falls back to the plain-link share when anything is missing — an old
/// Telegram client without `shareMessage`, a product the server can't find, a
/// network failure. Sharing something is always better than a dead button.
pub fn share_product(kind: ProductKind, id: &str, name: &str) {
    #[cfg(target_arch = "wasm32")]
    {
        let id = id.to_string();
        let name = name.to_string();
        dioxus::prelude::spawn(async move {
            let tg = crate::ui::telegram::TelegramApp::init();
            if tg.supports_share_message() {
                // The id goes in the body because the endpoint authenticates
                // the way the rest of the API does — `check_owner_lenient`,
                // which confirms an id rather than deriving one. Without a
                // user there is nobody to prepare a message for, so fall
                // straight through to the link share.
                let Some(tid) = tg.get_user_id() else {
                    share_product_link_only(kind, &id, &name);
                    return;
                };
                let body = format!(
                    r#"{{"kind":"{}","id":"{}","telegram_id":{}}}"#,
                    share_kind_wire(kind),
                    id.replace('"', ""),
                    tid
                );
                let url = format!(
                    "{}/api/share/prepare",
                    crate::ui::api::context::api_base_url()
                );
                let init_data = tg.get_init_data();
                if let Ok(resp) =
                    crate::ui::api::http::post_json_authed(&url, &init_data, &body).await
                {
                    if let Some(prepared) = serde_json::from_str::<serde_json::Value>(&resp)
                        .ok()
                        .and_then(|v| {
                            v.get("prepared_message_id")
                                .and_then(|p| p.as_str())
                                .map(String::from)
                        })
                    {
                        if tg.share_message(&prepared) {
                            return;
                        }
                    }
                }
            }
            share_product_link_only(kind, &id, &name);
        });
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        share_product_link_only(kind, id, name);
    }
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
        assert!(url.starts_with("https://t.me/turboagent_phuket_bot"));
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
}
