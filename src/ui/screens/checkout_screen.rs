use crate::trios::checkout_errors::{friendly_order_error, friendly_order_error_code};
use crate::trios::core::Lang;
use crate::trios::i18n::{
    t, tf, T_BACK, T_BIKE_PRICE_ON_REQUEST, T_CHECKOUT_ADDRESS_LABEL,
    T_CHECKOUT_ADDRESS_PLACEHOLDER, T_CHECKOUT_AGE_CONFIRM, T_CHECKOUT_AGE_NOTICE,
    T_CHECKOUT_BLOCKED_TITLE, T_CHECKOUT_BONUS, T_CHECKOUT_BONUS_APPLIED,
    T_CHECKOUT_BONUS_AVAILABLE, T_CHECKOUT_BONUS_MAX, T_CHECKOUT_CART_EMPTY,
    T_CHECKOUT_CASH_ON_DELIVERY, T_CHECKOUT_CHANGE, T_CHECKOUT_ERR_400, T_CHECKOUT_ERR_ADDRESS,
    T_CHECKOUT_ERR_ADDRESS_LONG, T_CHECKOUT_ERR_ITEMS, T_CHECKOUT_ERR_NAME,
    T_CHECKOUT_ERR_NAME_LONG, T_CHECKOUT_ERR_NETWORK, T_CHECKOUT_ERR_NO_TELEGRAM,
    T_CHECKOUT_ERR_PARSE, T_CHECKOUT_ERR_PHONE, T_CHECKOUT_ERR_PHONE_INVALID,
    T_CHECKOUT_ERR_PHONE_LONG, T_CHECKOUT_FULFILLMENT, T_CHECKOUT_FULFILLMENT_DELIVERY,
    T_CHECKOUT_FULFILLMENT_PICKUP, T_CHECKOUT_NAME_LABEL, T_CHECKOUT_NAME_PLACEHOLDER,
    T_CHECKOUT_NOTES_LABEL, T_CHECKOUT_NOTES_PLACEHOLDER, T_CHECKOUT_OPEN_MAP,
    T_CHECKOUT_PAY_ON_RECEIVE, T_CHECKOUT_PHONE_FROM_TELEGRAM, T_CHECKOUT_PHONE_LABEL,
    T_CHECKOUT_PHONE_PLACEHOLDER, T_CHECKOUT_PROCESSING, T_CHECKOUT_RETRY, T_CHECKOUT_SELECT_ZONE,
    T_CHECKOUT_STARS, T_CHECKOUT_STARS_AVAILABLE, T_CHECKOUT_STARS_MINUS, T_CHECKOUT_STEP_CART,
    T_CHECKOUT_STEP_CONFIRM, T_CHECKOUT_STEP_DETAILS, T_CHECKOUT_TITLE, T_CHECKOUT_TRUST_COD,
    T_CHECKOUT_TRUST_SECURE, T_CHECKOUT_TRUST_TITLE, T_CHECKOUT_TRUST_VERIFIED,
    T_CHECKOUT_USE_MY_LOCATION, T_DELIVERY, T_DELIVERY_ETA, T_DELIVERY_FEE, T_DELIVERY_ZONE,
    T_PAYMENT, T_PICKUP_LOCATION, T_PLACE_ORDER, T_TOTAL, T_YOUR_INFO, T_YOUR_ORDER,
};
use crate::trios::store::{checkout_blockers, normalize_phone, validate_checkout_for, Fulfillment};
use crate::ui::api::context::api_base_url;
// `fetch_text_authed_full` left this import with the 409 arm that was its only
// caller here (see `submit_order_with_retry`).
use crate::ui::api::http::{delete_authed, post_json_authed_idempotent_full};
use crate::ui::api::types::{DeliveryZone, DeliveryZonesResponse};
use crate::ui::components::error_banner::ErrorBanner;
use crate::ui::routes::Route;
use crate::ui::state::{Cart, CartItem, CartItemType};
use crate::ui::telegram::{
    use_main_button_click, use_telegram_id, use_telegram_init_data, use_telegram_username,
    HapticNotification, TelegramApp,
};
use dioxus::prelude::*;
use serde_json::json;
use wasm_bindgen::JsCast;
use web_sys::window;

/// Loop #15: fire a lightweight checkout-funnel event to the backend. Failures
/// are ignored so telemetry can never block the purchase flow.
fn emit_checkout_event(event: &'static str, detail: String) {
    spawn(async move {
        let _ = crate::ui::api::http::post_client_event(
            &crate::ui::api::context::api_base_url(),
            event,
            &detail,
        )
        .await;
    });
}

// `ApiReward` and `RewardsResp` stood here — the shapes `/api/garden/rewards`
// returned, alongside the fetch, the picker and the discount they fed. That
// endpoint is gone with the garden (D5), so the fetch could only 404 and the
// applied reward could only ever be `None`: the picker was already invisible
// and the discount already zero in every live checkout. This deletes the code,
// not the behaviour.

#[derive(serde::Deserialize)]
struct StarsBalanceResp {
    balance: i64,
}

#[derive(serde::Deserialize, Clone)]
struct LoyaltyProfileResp {
    bonus_balance: f64,
}

// API mirror: the config response carries the whole tier ladder; checkout
// only spends cashback here, the rest documents the shape we deserialize.
#[derive(serde::Deserialize, Clone)]
#[allow(dead_code)]
struct LoyaltyConfigResp {
    cashback_pct: f64,
    max_bonus_usage_pct: f64,
    next_tier: String,
    next_threshold: f64,
}

#[derive(serde::Deserialize, Clone)]
struct LoyaltyFullResp {
    profile: LoyaltyProfileResp,
    config: LoyaltyConfigResp,
}

/// Cycle #78: checkout form draft persisted to Telegram CloudStorage so the
/// customer doesn't lose their place if the Mini App is closed mid-checkout.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize, PartialEq)]
struct CheckoutDraft {
    #[serde(default)]
    name: String,
    #[serde(default)]
    phone: String,
    #[serde(default)]
    address: String,
    #[serde(default)]
    zone_id: Option<String>,
    #[serde(default)]
    notes: String,
    #[serde(default)]
    stars: i64,
    // `reward_id`/`reward_discount` were here. serde ignores unknown fields by
    // default, so a draft written by the shipped build still restores — it just
    // drops the two keys that named a reward nothing can grant any more.
    #[serde(default)]
    bonus: f64,
    #[serde(default)]
    age_confirmed: bool,
}

const CHECKOUT_DRAFT_LOCAL_KEY: &str = "woody_checkout_draft";
const CHECKOUT_DRAFT_CLOUD_KEY: &str = "wwb_checkout_draft";

/// Cycle #77: outcome of the idempotent checkout submission with retry.
enum SubmitResult {
    Success(String), // order_id
    HttpError(u16, String),
    NetworkError,
    ParseError,
}

/// Cycle #77: retry checkout submission with exponential backoff.
///
/// The loop chains one immediate attempt to the three backoff delays, so the
/// ceiling is FOUR requests per tap and not three — the count this comment
/// claimed until 2026-09-21, and the one a reader sizing the worst case would
/// have taken away. Only a network error and a 5xx are retried; every other
/// status is terminal and reaches the customer.
///
/// A 409 arm stood here until 2026-09-21. It called 409 "already accepted",
/// fetched `/api/orders/user/{id}` and returned the FIRST element of a
/// newest-first list as the order the customer had just placed — chosen by
/// recency, never checked against the idempotency key that was sent. It was
/// removed rather than repaired because it cannot fire: `POST /api/orders`
/// answers a replay with 200 and an `idempotent_replay` flag
/// (`src/api/orders.rs:1371-1376`), refuses an unusable key with 400 (:876),
/// and the only `StatusCode::CONFLICT` in that file is `cancel_order`
/// (:2336). A 409 from this endpoint now falls to the terminal arm below and
/// is shown to the customer, which is the honest outcome for a status nothing
/// in the server is known to send. `tests/wire_absence_wiring.rs` re-takes
/// that measurement on every run, so the premise cannot rot silently.
///
/// Removing the arm also removed its `telegram_id.unwrap_or(0)`: the recovery
/// URL was built for user 0 whenever identity was absent, so the one path the
/// branch could have taken would have queried a user that does not exist.
async fn submit_order_with_retry(
    url: &str,
    init_data: &str,
    idempotency_key: &str,
    body: &str,
) -> SubmitResult {
    const DELAYS_MS: [u32; 3] = [1_000, 2_000, 4_000];
    let mut last_status = 0u16;
    let mut last_body = String::new();

    for (attempt, delay_ms) in std::iter::once(0)
        .chain(DELAYS_MS.iter().copied())
        .enumerate()
    {
        if attempt > 0 {
            gloo_timers::future::TimeoutFuture::new(delay_ms).await;
        }

        match post_json_authed_idempotent_full(url, init_data, idempotency_key, body).await {
            Ok((status, response_body)) => {
                last_status = status;
                last_body.clone_from(&response_body);

                if (200..300).contains(&status) {
                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(&response_body) {
                        if let Some(id) = val.get("order_id").and_then(|v| v.as_str()) {
                            return SubmitResult::Success(id.to_string());
                        }
                    }
                    return SubmitResult::ParseError;
                }

                if (500..600).contains(&status) {
                    continue;
                }

                return SubmitResult::HttpError(status, response_body);
            }
            // Network error (no status at all) — retry with the next delay.
            Err(_) => continue,
        }
    }

    if (500..=599).contains(&last_status) {
        SubmitResult::HttpError(last_status, last_body)
    } else {
        // Every path that exhausts the retries without a 5xx is
        // network-shaped: an explicit network error, or a status the loop
        // never turned into a response worth surfacing.
        SubmitResult::NetworkError
    }
}

fn zone_display_name(zone: &DeliveryZone) -> String {
    if crate::ui::lang::current_lang() == Lang::English {
        zone.name_en.clone().unwrap_or_else(|| zone.name.clone())
    } else {
        zone.name.clone()
    }
}

/// Try to read the browser/Telegram geolocation and prepend a 📍 pin to the
/// address field. Falls back silently if geolocation is unavailable or denied.
fn fill_address_from_geolocation(mut set_address: Signal<String>) {
    spawn(async move {
        let js = r#"
            new Promise((resolve, reject) => {
                if (!navigator.geolocation) { reject("no geolocation"); return; }
                navigator.geolocation.getCurrentPosition(
                    p => resolve({ lat: p.coords.latitude, lng: p.coords.longitude }),
                    e => reject(e.message || "geolocation denied")
                );
            })
        "#;
        let Ok(promise_val) = wasm_bindgen::JsCast::dyn_into::<wasm_bindgen::JsValue>(
            js_sys::eval(js).unwrap_or_default(),
        ) else {
            return;
        };
        let Ok(promise) = promise_val.dyn_into::<js_sys::Promise>() else {
            return;
        };
        let fut = wasm_bindgen_futures::JsFuture::from(promise);
        if let Ok(val) = fut.await {
            let lat = js_sys::Reflect::get(&val, &wasm_bindgen::JsValue::from_str("lat"))
                .ok()
                .and_then(|v| v.as_f64())
                .unwrap_or(f64::NAN);
            let lng = js_sys::Reflect::get(&val, &wasm_bindgen::JsValue::from_str("lng"))
                .ok()
                .and_then(|v| v.as_f64())
                .unwrap_or(f64::NAN);
            if lat.is_finite() && lng.is_finite() {
                set_address.set(format!("📍 {:.6}, {:.6}", lat, lng));
            }
        }
    });
}

fn phone_valid(phone: &str) -> bool {
    // Single source of truth, shared with the server (`api/orders.rs`) so the
    // two gates can never drift apart again.
    crate::trios::store::normalize_phone(phone).is_some()
}

/// Real-time, per-field validation used for inline checkout feedback.
/// Returns the translation key for the first problem, or None if the field
/// looks acceptable so far (empty fields are reported so the user sees the
/// required indicator while typing).
///
/// `address` is only checked for emptiness when the order is delivered; see
/// [`checkout_blockers`].
fn validate_checkout_field(value: &str, kind: &str) -> Option<&'static str> {
    match kind {
        "name" => {
            if value.trim().is_empty() {
                Some(T_CHECKOUT_ERR_NAME)
            } else if value.len() > 200 {
                Some(T_CHECKOUT_ERR_NAME_LONG)
            } else {
                None
            }
        }
        "phone" => {
            if value.trim().is_empty() {
                Some(T_CHECKOUT_ERR_PHONE)
            } else if value.len() > 50 {
                Some(T_CHECKOUT_ERR_PHONE_LONG)
            } else if !phone_valid(value) {
                Some(T_CHECKOUT_ERR_PHONE_INVALID)
            } else {
                None
            }
        }
        "address" => {
            if value.trim().is_empty() {
                Some(T_CHECKOUT_ERR_ADDRESS)
            } else if value.len() > 500 {
                Some(T_CHECKOUT_ERR_ADDRESS_LONG)
            } else {
                None
            }
        }
        _ => None,
    }
}

fn to_trios_items(items: &[CartItem]) -> Vec<crate::trios::store::CartItem> {
    items
        .iter()
        .map(|i| match i.item_type {
            CartItemType::Strain => {
                crate::trios::store::CartItem::new_strain(i.id.clone(), i.quantity)
            }
            CartItemType::Accessory => {
                crate::trios::store::CartItem::new_accessory(i.id.clone(), i.quantity)
            }
            CartItemType::Tea => crate::trios::store::CartItem::new_tea(i.id.clone(), i.quantity),
            CartItemType::Set => crate::trios::store::CartItem::new_set(i.id.clone(), i.quantity),
        })
        .collect()
}

#[derive(Props, PartialEq, Clone)]
struct CheckoutStepperProps {
    current: usize,
}

#[component]
fn CheckoutStepper(props: CheckoutStepperProps) -> Element {
    let lang = crate::ui::lang::current_lang();
    let steps = [
        t(lang, T_CHECKOUT_STEP_CART).to_string(),
        t(lang, T_CHECKOUT_STEP_DETAILS).to_string(),
        t(lang, T_CHECKOUT_STEP_CONFIRM).to_string(),
    ];
    let current = props.current;
    rsx! {
        div { style: "display:flex;align-items:center;gap:6px;padding:0 16px 16px;",
            {
                steps.iter().enumerate().map(|(idx, label)| {
                    let is_active = idx == current;
                    let is_completed = idx < current;
                    let (bg, border, color) = if is_active {
                        ("#39ff14", "#39ff14", "#000")
                    } else if is_completed {
                        ("rgba(57,255,20,0.15)", "#39ff14", "#39ff14")
                    } else {
                        ("transparent", "#2a2a4a", "#8b8b9e")
                    };
                    let dot = if is_completed { "✓" } else { &format!("{}", idx + 1) };
                    rsx! {
                        div {
                            key: "{idx}",
                            style: "flex:1;display:flex;align-items:center;justify-content:center;gap:6px;padding:8px 4px;border:2px solid {border};background:{bg};color:{color};font-size:11px;font-weight:700;text-transform:uppercase;letter-spacing:1px;",
                            span { style: "display:flex;align-items:center;justify-content:center;width:18px;height:18px;border:2px solid {border};border-radius:50%;font-size:10px;", "{dot}" }
                            "{label}"
                        }
                    }
                })
            }
        }
    }
}

#[component]
pub fn CheckoutScreen() -> Element {
    let mut cart = use_context::<Signal<Cart>>();
    let cart_items = cart.read().items.clone();
    // D9/D11: a cart holding a line nobody priced has no total, so there is no
    // number to quote and no order to place. `cart_is_quotable` gates every
    // figure this screen renders and the submit handler refuses outright; the
    // cart screen refuses earlier, at the door into here.
    let cart_is_quotable = !cart.read().has_unpriced_line();
    // The sum of the lines that DO carry a price -- a measured figure, never
    // shown or sent as the cart's total. It is reached only when the cart is
    // quotable, and then it EQUALS the total; the bonus and star caps are
    // arithmetic on a subtotal, and threading an `Option` through them would
    // have bought a branch per cap and no extra honesty.
    let priced_subtotal = cart.read().priced_subtotal();
    let mut customer_name = use_signal(String::new);
    let mut customer_phone = use_signal(String::new);
    let mut delivery_address = use_signal(String::new);
    let mut delivery_notes = use_signal(String::new);
    let mut delivery_zone_id = use_signal(|| Option::<String>::None);
    // Age gate: the user must explicitly confirm they are 20+ before placing
    // an order. This drives both the in-app primary button and Telegram
    // MainButton enabled state.
    let mut age_confirmed = use_signal(|| false);

    // Loop #15: emit checkout_started once when the screen mounts with a
    // non-empty cart. Track only the first signalization to avoid noise.
    let mut checkout_started_emitted = use_signal(|| false);
    let cart_items_for_start = cart_items.clone();
    use_effect(move || {
        if !cart_items_for_start.is_empty() && !checkout_started_emitted() {
            checkout_started_emitted.set(true);
            emit_checkout_event("checkout_started", String::new());
        }
    });

    // Stars (⭐) the user wants to spend as internal-currency discount.
    let mut stars_to_use = use_signal(|| 0i64);
    // Loop #14: bonus balance the user wants to redeem at checkout.
    let mut bonus_to_use = use_signal(|| 0.0f64);
    // Cycle #78: tracks the draft we loaded from localStorage so the async
    // CloudStorage restore can decide whether the user has already edited it.
    let mut loaded_draft = use_signal(|| Option::<CheckoutDraft>::None);

    // Cycle #78: restore the checkout draft. Prefer the consolidated JSON key,
    // then fall back to the legacy individual keys written by previous cycles.
    use_effect(move || {
        let mut draft = CheckoutDraft::default();
        if let Some(window) = web_sys::window() {
            if let Ok(Some(storage)) = window.local_storage() {
                if let Ok(Some(json)) = storage.get_item(CHECKOUT_DRAFT_LOCAL_KEY) {
                    if let Ok(parsed) = serde_json::from_str::<CheckoutDraft>(&json) {
                        draft = parsed;
                    }
                }
                // Legacy individual-key fallback only if the consolidated key
                // wasn't present or didn't have a field.
                if draft.name.is_empty() {
                    if let Ok(Some(v)) = storage.get_item("woody_last_name") {
                        draft.name = v;
                    }
                }
                if draft.phone.is_empty() {
                    if let Ok(Some(v)) = storage.get_item("woody_last_phone") {
                        draft.phone = v;
                    }
                }
                if draft.address.is_empty() {
                    if let Ok(Some(v)) = storage.get_item("woody_last_address") {
                        draft.address = v;
                    }
                }
                if draft.zone_id.is_none() {
                    if let Ok(Some(v)) = storage.get_item("woody_last_zone_id") {
                        draft.zone_id = Some(v);
                    }
                }
                if draft.notes.is_empty() {
                    if let Ok(Some(v)) = storage.get_item("woody_last_notes") {
                        draft.notes = v;
                    }
                }
                if draft.stars == 0 {
                    if let Ok(Some(v)) = storage.get_item("woody_last_stars") {
                        if let Ok(n) = v.parse::<i64>() {
                            draft.stars = n;
                        }
                    }
                }
                if draft.bonus == 0.0 {
                    if let Ok(Some(v)) = storage.get_item("woody_last_bonus") {
                        if let Ok(n) = v.parse::<f64>() {
                            draft.bonus = n;
                        }
                    }
                }
                // `woody_last_reward_id` is deliberately neither read nor
                // cleared: it is a key in live browsers, and a removal that
                // reaches into someone's storage to delete it buys nothing.
                if !draft.age_confirmed {
                    if let Ok(Some(v)) = storage.get_item("woody_last_age_confirmed") {
                        draft.age_confirmed = v == "true";
                    }
                }
            }
        }
        customer_name.set(draft.name.clone());
        customer_phone.set(draft.phone.clone());
        delivery_address.set(draft.address.clone());
        delivery_zone_id.set(draft.zone_id.clone());
        delivery_notes.set(draft.notes.clone());
        stars_to_use.set(draft.stars);
        bonus_to_use.set(draft.bonus.max(0.0));
        age_confirmed.set(draft.age_confirmed);
        loaded_draft.set(Some(draft));
    });

    // Cycle #78: async restore from Telegram CloudStorage. We only apply it if
    // the user hasn't edited the draft we just loaded from localStorage, so a
    // slow callback can't clobber an in-progress form fill.
    #[cfg(target_arch = "wasm32")]
    use_hook(move || {
        let mut name_sig = customer_name;
        let mut phone_sig = customer_phone;
        let mut address_sig = delivery_address;
        let mut zone_sig = delivery_zone_id;
        let mut notes_sig = delivery_notes;
        let mut stars_sig = stars_to_use;
        let mut bonus_sig = bonus_to_use;
        let mut age_sig = age_confirmed;
        let loaded = loaded_draft;
        spawn(async move {
            let tg = TelegramApp;
            if let Some(json) = tg.cloud_storage_get(CHECKOUT_DRAFT_CLOUD_KEY).await {
                if let Ok(draft) = serde_json::from_str::<CheckoutDraft>(&json) {
                    let should_apply = match loaded.read().clone() {
                        Some(ld) => {
                            let current = CheckoutDraft {
                                name: name_sig(),
                                phone: phone_sig(),
                                address: address_sig(),
                                zone_id: zone_sig(),
                                notes: notes_sig(),
                                stars: stars_sig(),
                                bonus: bonus_sig(),
                                age_confirmed: age_sig(),
                            };
                            current == ld
                        }
                        None => true,
                    };
                    if should_apply {
                        name_sig.set(draft.name);
                        phone_sig.set(draft.phone);
                        address_sig.set(draft.address);
                        zone_sig.set(draft.zone_id);
                        notes_sig.set(draft.notes);
                        stars_sig.set(draft.stars);
                        bonus_sig.set(draft.bonus.max(0.0));
                        age_sig.set(draft.age_confirmed);
                    }
                }
            }
        });
    });

    // Cycle #78: persist the checkout draft on every change so the customer can
    // close the Mini App mid-form and resume later. localStorage is the browser
    // fallback; CloudStorage follows the Telegram account across devices.
    use_effect(move || {
        let draft = CheckoutDraft {
            name: customer_name(),
            phone: customer_phone(),
            address: delivery_address(),
            zone_id: delivery_zone_id(),
            notes: delivery_notes(),
            stars: stars_to_use(),
            bonus: bonus_to_use().max(0.0),
            age_confirmed: age_confirmed(),
        };
        #[cfg(target_arch = "wasm32")]
        {
            let json = serde_json::to_string(&draft).unwrap_or_default();
            if let Some(window) = web_sys::window() {
                if let Ok(Some(storage)) = window.local_storage() {
                    let _ = storage.set_item(CHECKOUT_DRAFT_LOCAL_KEY, &json);
                }
            }
            let tg = TelegramApp;
            tg.cloud_storage_set(CHECKOUT_DRAFT_CLOUD_KEY, &json);
        }
    });

    // Name comes from Telegram; the field is only shown when it does not.
    let telegram_name = TelegramApp::init().get_full_name();
    let can_request_contact = TelegramApp::init().supports_request_contact();
    let mut editing_name = use_signal(|| false);
    let mut shop_selected = use_signal(|| 0usize);

    // Fill the name from Telegram once, and only into an empty field so a
    // restored draft or a correction the customer typed is never overwritten.
    {
        let tg_name = telegram_name.clone();
        use_effect(move || {
            if let Some(ref n) = tg_name {
                if customer_name().trim().is_empty() {
                    customer_name.set(n.clone());
                }
            }
        });
    }

    // `requestContact` answers asynchronously through Telegram's own consent
    // dialog; `TelegramApp::request_contact` republishes the number as a
    // `woody:contact` event so it can be picked up here.
    #[cfg(target_arch = "wasm32")]
    use_hook(move || {
        use wasm_bindgen::JsCast;
        let Some(win) = web_sys::window() else {
            return;
        };
        // Same hazard as the main button: this fires from raw JS with no
        // Dioxus scope. See `crate::ui::telegram::in_dioxus_scope`.
        let scope = current_scope_id().ok();
        let listener = gloo_events::EventListener::new(&win, "woody:contact", move |event| {
            let phone = event
                .dyn_ref::<web_sys::CustomEvent>()
                .and_then(|e| e.detail().as_string())
                .unwrap_or_default();
            if !phone.trim().is_empty() {
                crate::ui::telegram::in_dioxus_scope(scope, "contact", || {
                    customer_phone.set(phone);
                });
            }
        });
        // Held for the lifetime of the screen; dropping it would unsubscribe.
        listener.forget();
    });
    // Delivery vs pickup. Pickup orders carry no address, which is why the
    // address gate below is conditional rather than unconditional.
    let mut fulfillment = use_signal(Fulfillment::default);
    let mut is_processing = use_signal(|| false);
    let mut order_error = use_signal(|| Option::<String>::None);
    // Cycle #57: stable idempotency key per logical submit. Lazy-init on the
    // first click and reused for retries within this mount so server
    // collapses them into one order (migration 029). Navigating away or
    // unmounting the screen resets — exactly the boundary we want.
    let mut idempotency_key = use_signal(|| Option::<String>::None);
    let nav = navigator();
    let telegram_id = use_telegram_id();
    let telegram_username = use_telegram_username();
    let init_data = use_telegram_init_data();
    let tg = TelegramApp::init();
    tg.show_back_button();
    // Prevent accidental close while the user is filling the checkout form.
    tg.enable_closing_confirmation();

    // Cycle #73 / A: lang is now resolved once at WASM startup
    // (lib.rs::run via pick_lang) and stored in OnceLock; just read it.
    // No more per-screen inline pick_lang dance.
    let lang = crate::ui::lang::current_lang();

    // Real-time field-level validation feedback. These are derived from the
    // current input values and update immediately as the user types.
    let name_error = use_memo(move || {
        validate_checkout_field(&customer_name(), "name").map(|k| t(lang, k).to_string())
    });
    let phone_error = use_memo(move || {
        validate_checkout_field(&customer_phone(), "phone").map(|k| t(lang, k).to_string())
    });
    // Pickup orders have nothing to deliver to, so an empty address is not an
    // error there — only an over-long one is.
    let address_error = use_memo(move || {
        let value = delivery_address();
        if !fulfillment().requires_address() && value.trim().is_empty() {
            return None;
        }
        validate_checkout_field(&value, "address").map(|k| t(lang, k).to_string())
    });
    let name_border = if name_error().is_some() {
        "#ff4757"
    } else {
        "#2a2a4a"
    };
    let phone_border = if phone_error().is_some() {
        "#ff4757"
    } else {
        "#2a2a4a"
    };
    let address_border = if address_error().is_some() {
        "#ff4757"
    } else {
        "#2a2a4a"
    };

    let checkout_title = t(crate::ui::lang::current_lang(), T_CHECKOUT_TITLE);
    let your_order = t(crate::ui::lang::current_lang(), T_YOUR_ORDER);
    let your_info = format!("👤 {}", t(crate::ui::lang::current_lang(), T_YOUR_INFO));
    let pickup_location = t(crate::ui::lang::current_lang(), T_PICKUP_LOCATION);
    let delivery = t(crate::ui::lang::current_lang(), T_DELIVERY);
    let delivery_zone_label = t(crate::ui::lang::current_lang(), T_DELIVERY_ZONE);
    let payment = t(crate::ui::lang::current_lang(), T_PAYMENT);
    let place_order = t(crate::ui::lang::current_lang(), T_PLACE_ORDER);
    let back = t(crate::ui::lang::current_lang(), T_BACK);
    let total_label = t(crate::ui::lang::current_lang(), T_TOTAL);

    // Единственная реальная точка самовывоза.
    let shops = [("🏠 TurboBaby", "Kamala, Phuket 83150")];

    let stars_balance_res = {
        let init = init_data.clone();
        use_resource(move || {
            let init = init.clone();
            async move {
                let tid = telegram_id?;
                let url = format!("{}/api/stars/balance/{}", api_base_url(), tid);
                let text = crate::ui::api::http::fetch_text_authed(&url, &init)
                    .await
                    .ok()?;
                serde_json::from_str::<StarsBalanceResp>(&text)
                    .ok()
                    .map(|r| r.balance)
            }
        })
    };
    let stars_balance = stars_balance_res
        .read()
        .as_ref()
        .and_then(|opt| opt.as_ref())
        .cloned()
        .unwrap_or(0);

    // Loop #14: load loyalty profile + config so the customer can see and
    // redeem bonus balance at checkout with the same cap/cashback the server uses.
    let loyalty_res = {
        let init = init_data.clone();
        use_resource(move || {
            let init = init.clone();
            async move {
                let tid = telegram_id?;
                let url = format!("{}/api/loyalty/{}", api_base_url(), tid);
                let text = crate::ui::api::http::fetch_text_authed(&url, &init)
                    .await
                    .ok()?;
                serde_json::from_str::<LoyaltyFullResp>(&text).ok()
            }
        })
    };
    let (bonus_balance, max_bonus_usage_pct) = loyalty_res
        .read()
        .as_ref()
        .and_then(|opt| opt.as_ref())
        .map(|r| (r.profile.bonus_balance, r.config.max_bonus_usage_pct))
        .unwrap_or((0.0, 30.0));
    // Delivery zones/ETA are public; no auth header needed.
    let zones_res = use_resource(move || async move {
        let url = format!("{}/api/delivery/zones", api_base_url());
        let text = crate::ui::api::http::fetch_text(&url).await.ok()?;
        serde_json::from_str::<DeliveryZonesResponse>(&text)
            .ok()
            .map(|r| r.zones)
    });
    // The cart total used to have a garden-reward discount subtracted here.
    // Nothing subtracts from it now: the only discounts left are bonus points
    // and stars, both of which the server re-verifies.
    let pre_bonus_total = priced_subtotal.max(0.0);
    // Loop #14: bonus redemption is capped by both the user's balance and the
    // business-configured share of the order subtotal. Clamp the applied amount
    // so the UI never proposes a value the server would reject.
    let max_bonus_for_order = ((pre_bonus_total * max_bonus_usage_pct / 100.0)
        .floor()
        .min(bonus_balance)
        .max(0.0))
    .min(pre_bonus_total);
    let bonus_val = bonus_to_use().clamp(0.0, max_bonus_for_order).max(0.0);
    let after_bonus = (pre_bonus_total - bonus_val).max(0.0);
    let max_stars = (after_bonus.floor() as i64).min(stars_balance).max(0);
    let stars_val = (*stars_to_use.read()).clamp(0, max_stars.max(0));
    let effective_total = (after_bonus - stars_val as f64).max(0.0);
    // The one place this screen turns the total into text. A dash when the cart
    // cannot be quoted; `format_baht` otherwise, NOT `thb_or_dash` -- an order
    // paid entirely with bonus and stars really does come to zero, and that
    // zero is a computed figure rather than the absence D9 is about.
    let effective_total_str = if cart_is_quotable {
        crate::trios::pricing::format_baht(effective_total)
    } else {
        crate::ui::components::bike_card::DASH.to_string()
    };

    // Loop #15: emit conversion events when the customer actually uses loyalty
    // rewards. Each fires once per mount to keep the signal clean. (A third
    // event, `garden_reward_applied`, fired beside these until D5.)
    let mut bonus_event_fired = use_signal(|| false);
    use_effect(move || {
        if bonus_val > 0.01 && !bonus_event_fired() {
            bonus_event_fired.set(true);
            emit_checkout_event("bonus_applied", format!("{bonus_val:.0}"));
        }
    });
    let mut stars_event_fired = use_signal(|| false);
    use_effect(move || {
        if stars_val > 0 && !stars_event_fired() {
            stars_event_fired.set(true);
            emit_checkout_event("stars_applied", stars_val.to_string());
        }
    });

    // Loop #14: pre-format bonus strings outside rsx! so nested format! braces
    // don't confuse the Dioxus macro parser.
    //
    // Every amount below goes through `format_baht`, the same formatter the
    // MainButton total uses a few lines down. They used to disagree on the same
    // screen at the same moment: the button said `฿3,000` and the bonus row
    // said `3000 ฿`, because the row's `{:.0}` had the glyph baked into its
    // translation and no idea where the market puts it (D15, D18).
    let bonus_header_text = use_memo(move || {
        let available = tf(
            lang,
            T_CHECKOUT_BONUS_AVAILABLE,
            &[crate::trios::pricing::format_baht(bonus_balance)],
        );
        let max = tf(
            lang,
            T_CHECKOUT_BONUS_MAX,
            &[crate::trios::pricing::format_baht(max_bonus_for_order)],
        );
        format!("{available} · {max}")
    });
    let bonus_applied_text = use_memo(move || {
        tf(
            lang,
            T_CHECKOUT_BONUS_APPLIED,
            &[crate::trios::pricing::format_baht(bonus_val)],
        )
    });
    let stars_available_text = use_memo(move || {
        tf(
            lang,
            T_CHECKOUT_STARS_AVAILABLE,
            &[stars_balance.to_string()],
        )
    });
    let stars_minus_text =
        // Money, not a star count: `effective_total` subtracts `stars_val as
        // f64` directly, so this line is the baht that came off the bill. The
        // count itself is `T_CHECKOUT_STARS_AVAILABLE`, which carries no symbol
        // for exactly that reason.
        use_memo(move || {
            tf(
                lang,
                T_CHECKOUT_STARS_MINUS,
                &[crate::trios::pricing::format_baht(stars_val as f64)],
            )
        });

    let zones = match &*zones_res.read() {
        Some(Some(z)) => z.clone(),
        _ => Vec::new(),
    };
    let selected_zone = delivery_zone_id
        .read()
        .as_ref()
        .and_then(|id| zones.iter().find(|z| z.id == *id))
        .or_else(|| zones.first())
        .cloned();
    // The fee, and an ETA line only when the zone row carries both minutes.
    // None is published (owner, 2026-09-24): the shop promises a window, so
    // an absent pair shows no ETA at all, and no literal range stands in. The
    // fee's own fallback for an empty zone list, below, is unchanged.
    let delivery_eta_text: Option<String> = selected_zone.as_ref().and_then(|z| {
        let (min, max) = (z.min_eta_minutes?, z.max_eta_minutes?);
        Some(tf(lang, T_DELIVERY_ETA, &[format!("{min}-{max}")]))
    });
    let delivery_fee_text = match selected_zone.as_ref() {
        Some(z) => t(crate::ui::lang::current_lang(), T_DELIVERY_FEE).replace(
            "{0}",
            &crate::trios::pricing::format_baht(z.delivery_fee_baht),
        ),
        None => tf(
            lang,
            T_DELIVERY_FEE,
            &[crate::trios::pricing::format_baht(0.0)],
        ),
    };

    // Sync the native Telegram MainButton with the live total and form validity.
    let cart_len = cart_items.len();
    // The effect and the submit callback both outlive this scope, so each takes
    // its own copy of the one rendered figure rather than recomputing it.
    let main_button_total = effective_total_str.clone();
    let restore_total = effective_total_str.clone();
    use_effect(move || {
        let lang = crate::ui::lang::current_lang();
        tg.set_main_button_text(&format!(
            "{} — {}",
            t(lang, T_PLACE_ORDER),
            main_button_total
        ));
        // Same gate as the in-page button — one source of truth, so the native
        // MainButton and the on-screen button can never disagree about whether
        // the order is placeable.
        let valid = checkout_blockers(
            telegram_id.is_some(),
            &customer_name(),
            &customer_phone(),
            &delivery_address(),
            fulfillment(),
            cart_len,
            age_confirmed(),
        )
        .is_empty()
            && !is_processing()
            // An unquotable cart blocks the native button too: `checkout_blockers`
            // reasons about the form, and this is a fact about the cart.
            && cart_is_quotable;
        if valid {
            tg.enable_main_button();
        } else {
            tg.disable_main_button();
        }
    });

    let submit_cart_items = cart_items.clone();
    let submit_selected_zone = selected_zone.clone();
    let submit_order = use_callback(move |_: ()| {
        if is_processing() {
            return;
        }
        // D9/D11, before any form check: a cart holding a line the shop cannot
        // price cannot be ordered, because the body would carry a `subtotal`
        // and a `total` computed from the priced lines alone -- a number nobody
        // measured, sent to a price-authoritative endpoint and charged. The
        // customer is told what D11 requires instead of being told nothing,
        // and the sentence is the one the catalog already ships for the same
        // situation (no new wording, no new locale row).
        if !cart_is_quotable {
            order_error.set(Some(t(lang, T_BIKE_PRICE_ON_REQUEST).to_string()));
            tg.haptic_notification(HapticNotification::Warning);
            return;
        }
        if telegram_id.is_none() {
            order_error.set(Some(t(lang, T_CHECKOUT_ERR_NO_TELEGRAM).to_string()));
            tg.haptic_notification(HapticNotification::Error);
            return;
        }
        if !age_confirmed() {
            order_error.set(Some(t(lang, T_CHECKOUT_AGE_NOTICE).to_string()));
            tg.haptic_notification(HapticNotification::Warning);
            return;
        }
        let trios_items = to_trios_items(&submit_cart_items);
        if let Err(e) = validate_checkout_for(
            &customer_name(),
            &customer_phone(),
            &delivery_address(),
            &trios_items,
            fulfillment(),
        ) {
            let key = match e {
                crate::trios::core::Error::Validation(msg) if msg.contains("Name is required") => {
                    T_CHECKOUT_ERR_NAME
                }
                crate::trios::core::Error::Validation(msg) if msg.contains("Name is too long") => {
                    T_CHECKOUT_ERR_NAME_LONG
                }
                crate::trios::core::Error::Validation(msg) if msg.contains("Phone is required") => {
                    T_CHECKOUT_ERR_PHONE
                }
                crate::trios::core::Error::Validation(msg) if msg.contains("Phone is too long") => {
                    T_CHECKOUT_ERR_PHONE_LONG
                }
                crate::trios::core::Error::Validation(msg)
                    if msg.contains("Invalid phone number") =>
                {
                    T_CHECKOUT_ERR_PHONE_INVALID
                }
                crate::trios::core::Error::Validation(msg)
                    if msg.contains("Delivery address is required") =>
                {
                    T_CHECKOUT_ERR_ADDRESS
                }
                crate::trios::core::Error::Validation(msg)
                    if msg.contains("Delivery address is too long") =>
                {
                    T_CHECKOUT_ERR_ADDRESS_LONG
                }
                crate::trios::core::Error::Validation(msg)
                    if msg.contains("Cart cannot be empty") =>
                {
                    T_CHECKOUT_ERR_ITEMS
                }
                _ => T_CHECKOUT_ERR_400,
            };
            order_error.set(Some(t(lang, key).to_string()));
            tg.haptic_notification(HapticNotification::Warning);
            return;
        }
        is_processing.set(true);
        order_error.set(None);

        // Show the native Telegram MainButton spinner while the network request runs.
        tg.show_main_button_progress(t(lang, T_CHECKOUT_PROCESSING), true);
        let restore_text = format!("{} — {}", t(lang, T_PLACE_ORDER), restore_total);
        let zone_info = submit_selected_zone.clone();

        let base = api_base_url();
        let url = format!("{}/api/orders", base);

        // Generate or reuse the idempotency key (cycle #57). The signal stays
        // alive across spawned tasks because Dioxus signals are rooted in the
        // component, not the closure.
        //
        // Cycle #77: `Option::get_or_insert_with` encodes the
        // "is_none → set; is_some → keep" invariant in the type system,
        // returning `&mut String` directly. Cleaner than the old
        // write-then-clone-unwrap pattern, and provably panic-free.
        let key = {
            let mut k = idempotency_key.write();
            k.get_or_insert_with(|| uuid::Uuid::new_v4().to_string())
                .clone()
        };

        let items_json: Vec<serde_json::Value> = submit_cart_items
            .iter()
            .map(|item| match item.item_type {
                CartItemType::Strain => json!({
                    "strain_id": item.id,
                    "strain_name": item.name,
                    "quantity": item.quantity,
                    "unit_price": item.price,
                }),
                CartItemType::Accessory => json!({
                    "accessory_id": item.id,
                    "accessory_name": item.name,
                    "quantity": item.quantity,
                    "unit_price": item.price,
                }),
                CartItemType::Tea => json!({
                    "tea_id": item.id,
                    "tea_name": item.name,
                    "quantity": item.quantity,
                    "unit_price": item.price,
                    // A3: carry the drink's dine-in/takeaway choice (default
                    // takeaway if the customer never toggled it).
                    "fulfillment": item.fulfillment.clone().unwrap_or_else(|| "takeaway".to_string()),
                }),
                CartItemType::Set => json!({
                    "set_id": item.id,
                    "set_name": item.name,
                    "quantity": item.quantity,
                    "unit_price": item.price,
                }),
            })
            .collect();

        let submit_stars = (*stars_to_use.read()).clamp(0, max_stars);
        let submit_bonus = bonus_val;
        // Reached only past the `cart_is_quotable` refusal above, where the
        // priced subtotal IS the cart's total.
        let order_total = (priced_subtotal - submit_stars as f64 - submit_bonus).max(0.0);

        let body = json!({
            "telegram_id": telegram_id,
            "customer_name": customer_name(),
            // Send E.164 so the courier-facing number is unambiguous; the
            // customer keeps seeing whatever they typed.
            "customer_phone": normalize_phone(&customer_phone()).unwrap_or_else(&*customer_phone),
            "customer_telegram": telegram_username.clone(),
            "items": items_json,
            "subtotal": priced_subtotal,
            "bonus_used": submit_bonus,
            "stars_used": submit_stars,
            "total": order_total,
            "shop_id": shops[shop_selected()].0,
            "fulfillment": fulfillment().as_str(),
            "delivery_address": delivery_address(),
            "delivery_notes": delivery_notes(),
            "age_confirmed": age_confirmed(),
            "delivery_zone_id": zone_to_submit(delivery_zone_id(), zone_info.as_ref()),
        });

        let init_data_clone = init_data.clone();
        let key_clone = key.clone();
        let body_text = body.to_string();
        let restore_text_clone = restore_text.clone();
        let base_clone = base.clone();
        // The identity travels as the `Option` it is. It used to be flattened
        // here with `unwrap_or(0)`, and `0` then had to be re-read as "absent"
        // further down by a `!= 0` test — a sentinel standing in for the
        // absence the type already expressed.
        let telegram_id_for_cleanup = telegram_id;

        spawn(async move {
            let result =
                submit_order_with_retry(&url, &init_data_clone, &key_clone, &body_text).await;

            match result {
                SubmitResult::Success(order_id) => {
                    // Persist last used delivery details for next checkout.
                    if let Some(window) = web_sys::window() {
                        if let Ok(Some(storage)) = window.local_storage() {
                            let _ = storage.set_item("woody_last_name", &customer_name());
                            let _ = storage.set_item("woody_last_phone", &customer_phone());
                            let _ = storage.set_item("woody_last_address", &delivery_address());
                            let _ = storage.set_item(
                                "woody_last_zone_id",
                                delivery_zone_id().as_deref().unwrap_or(""),
                            );
                            if let Some(ref z) = zone_info {
                                let _ =
                                    storage.set_item("woody_last_zone_name", &zone_display_name(z));
                                // No ETA is stored any more: the success screen reads
                                // it from the server alone, and no zone row carries one
                                // (owner, 2026-09-24). A key an older build wrote is
                                // left in place and never read.
                            }
                            let _ = storage.set_item("woody_last_notes", &delivery_notes());
                            let _ = storage.set_item("woody_last_stars", &submit_stars.to_string());
                            let _ =
                                storage.set_item("woody_last_bonus", &format!("{submit_bonus:.0}"));
                            let _ = storage.set_item(
                                "woody_last_age_confirmed",
                                if age_confirmed() { "true" } else { "false" },
                            );
                        }
                    }
                    // Stop the spinner and hide the native button before leaving the screen.
                    tg.hide_main_button_progress(&restore_text_clone);
                    tg.hide_main_button();
                    // The user is leaving the form; allow Telegram swipe-to-close again.
                    tg.disable_closing_confirmation();
                    // Loop #11: clear the server-side cart so a returning
                    // customer doesn't see stale items after a successful order.
                    if let Some(tid) = telegram_id_for_cleanup {
                        let clear_url = format!("{}/api/cart?telegram_id={}", base_clone, tid);
                        let _ = delete_authed(&clear_url, &init_data_clone).await;
                    }
                    // Navigate to success and clear cart.
                    cart.write().clear();
                    tg.haptic_notification(HapticNotification::Success);
                    emit_checkout_event("checkout_completed", order_id.clone());
                    nav.push(Route::Success { id: order_id });
                }
                SubmitResult::ParseError => {
                    tg.haptic_notification(HapticNotification::Error);
                    order_error.set(Some(t(lang, T_CHECKOUT_ERR_PARSE).to_string()));
                    emit_checkout_event("checkout_error", "parse".to_string());
                    tg.hide_main_button_progress(&restore_text_clone);
                    is_processing.set(false);
                }
                SubmitResult::HttpError(status, response_body) => {
                    tg.haptic_notification(HapticNotification::Error);
                    // Loop #7: if the server returned a stable `error` code in
                    // the JSON body, surface a per-field sentence before
                    // falling back to the generic status mapper.
                    let code_msg = serde_json::from_str::<serde_json::Value>(&response_body)
                        .ok()
                        .and_then(|v| v.get("error").and_then(|e| e.as_str()).map(String::from))
                        .and_then(|code| friendly_order_error_code(lang, &code));
                    let msg = code_msg.unwrap_or_else(|| friendly_order_error(lang, status));
                    order_error.set(Some(msg));
                    emit_checkout_event("checkout_error", status.to_string());
                    tg.hide_main_button_progress(&restore_text_clone);
                    is_processing.set(false);
                }
                SubmitResult::NetworkError => {
                    tg.haptic_notification(HapticNotification::Error);
                    order_error.set(Some(t(lang, T_CHECKOUT_ERR_NETWORK).to_string()));
                    emit_checkout_event("checkout_error", "network".to_string());
                    tg.hide_main_button_progress(&restore_text_clone);
                    is_processing.set(false);
                }
            }
        });
    });

    // Wire the native Telegram MainButton to the same submit path as the
    // in-app primary button. The callback is wrapped so the hook receives a
    // zero-argument FnMut while reusing the existing submit closure.
    use_main_button_click(move || {
        submit_order.call(());
    });

    rsx! {
            div { style: "
            min-height: 100vh;
            background: #0f0f1a;
            color: #e8e8e8;
            padding-bottom: calc(96px + env(safe-area-inset-bottom));
        ",
                CheckoutStepper { current: 1 }
                div { style: "padding: 4px 16px 16px; text-align: center;",
                    h1 { style: "font-size: 24px; font-weight: 800; color: #39ff14; text-shadow: 3px 3px 0 #000, 0 0 10px rgba(57,255,20,0.5); letter-spacing: 2px;", "{checkout_title}" }
                }

                div { style: "padding: 0 16px;",
    // Order summary from cart
                    div { style: "
                    background: #16213e; border: 4px solid #2a2a4a;
                    border-radius: 0; padding: 14px; margin-bottom: 12px;
                    box-shadow: 4px 4px 0 #000;
                ",
                        h2 { style: "font-size: 13px; font-weight: 700; color: #00e5ff; text-transform: uppercase; letter-spacing: 1px; text-shadow: 2px 2px 0 #000; margin-bottom: 10px;", "{your_order}" }
                        if cart_items.is_empty() {
                            p { style: "font-size: 15px; color: #8b8b9e; text-align: center; padding: 10px;", "{t(lang, T_CHECKOUT_CART_EMPTY)}" }
                        } else {
                            for item in cart_items.iter() {
                                div { style: "display:flex; gap:10px; align-items:center; margin-bottom:10px;",
                                    if let Some(url) = crate::trios::legacy_view::cart_line_image(crate::ui::api::http::cart_item_type_to_kind(&item.item_type), &item.image_url) {
                                        img {
                                            src: "{url}",
                                            alt: "{crate::trios::legacy_view::shown_cart_line_name(lang, crate::ui::api::http::cart_item_type_to_kind(&item.item_type), &item.name)}",
                                            style: "width:44px; height:44px; object-fit:cover; border:2px solid #2a2a4a; flex-shrink:0;",
                                            loading: "lazy",
                                        }
                                    }
                                    div { style: "flex:1; min-width:0;",
                                        div { style: "font-size:13px; color:#e8e8e8; white-space:nowrap; overflow:hidden; text-overflow:ellipsis;", "{crate::trios::legacy_view::shown_cart_line_name(lang, crate::ui::api::http::cart_item_type_to_kind(&item.item_type), &item.name)}" }
                                        div { style: "font-size:12px; color:#8b8b9e;", "x{item.quantity}" }
                                    }
                                    div { style: "font-size:13px; font-weight:700; color:#e8e8e8;",
                                        {
                                            // The shared line rule (D15): an absent
                                            // unit price has no line total, and the
                                            // slot shows a dash rather than a zero.
                                            crate::ui::components::bike_card::thb_or_dash(
                                                crate::trios::pricing::cart_line_total(
                                                    item.price, item.quantity,
                                                ),
                                            )
                                        }
                                    }
                                }
                            }
                            // Bonus balance picker.
                            if bonus_balance > 0.0 && pre_bonus_total > 0.0 {
                                div { style: "border-top:1px solid #2a2a4a;margin-top:8px;padding-top:8px;",
                                    div { style: "display:flex;justify-content:space-between;align-items:center;margin-bottom:6px;",
                                        div { style: "font-size:12px;color:#39ff14;font-weight:700;", "{t(lang, T_CHECKOUT_BONUS)}" }
                                        div { style: "font-size:12px;color:#8b8b9e;", "{bonus_header_text}" }
                                    }
                                    div { style: "display:flex;align-items:center;gap:8px;",
                                        input {
                                            r#type: "number",
                                            inputmode: "numeric",
                                            min: "0",
                                            max: "{max_bonus_for_order:.0}",
                                            value: "{bonus_val:.0}",
                                            style: "width:80px;font-size:14px;padding:6px 8px;background:#0f0f1a;color:#e8e8e8;border:3px solid #2a2a4a;",
                                            oninput: move |e| {
                                                let v = e.value().parse::<f64>().unwrap_or(0.0);
                                                bonus_to_use.set(v.clamp(0.0, max_bonus_for_order));
                                            }
                                        }
                                        span { style: "font-size:12px;color:#39ff14;", "{bonus_applied_text}" }
                                    }
                                }
                            }

                            // Stars discount picker.
                            if stars_balance > 0 && after_bonus > 0.0 {
                                div { style: "border-top:1px solid #2a2a4a;margin-top:8px;padding-top:8px;",
                                    div { style: "display:flex;justify-content:space-between;align-items:center;margin-bottom:6px;",
                                        div { style: "font-size:12px;color:#7dd3fc;font-weight:700;", "{t(lang, T_CHECKOUT_STARS)}" }
                                        div { style: "font-size:12px;color:#8b8b9e;", "{stars_available_text}" }
                                    }
                                    div { style: "display:flex;align-items:center;gap:8px;",
                                        input {
                                            r#type: "number",
                                            inputmode: "numeric",
                                            min: "0",
                                            max: "{max_stars}",
                                            value: "{stars_val}",
                                            style: "width:80px;font-size:14px;padding:6px 8px;background:#0f0f1a;color:#e8e8e8;border:3px solid #2a2a4a;",
                                            oninput: move |e| {
                                                let v = e.value().parse::<i64>().unwrap_or(0);
                                                stars_to_use.set(v.clamp(0, max_stars));
                                            }
                                        }
                                        span { style: "font-size:12px;color:#7dd3fc;", "{stars_minus_text}" }
                                    }
                                }
                            }
                            div { style: "display: flex; justify-content: space-between; font-size: 15px; font-weight: 800; padding-top: 8px; border-top: 1px solid #2a2a4a; margin-top: 8px;",
                                span { "{total_label}" }
                                span {
                                    style: "font-size: 20px; font-weight: 800; color: #ffe600; text-shadow: 2px 2px 0 #000;",
                                    "{effective_total_str}"
                                }
                            }
                        }
                    }

                    // Customer info
                    div { style: "
                    background: #16213e; border: 4px solid #2a2a4a;
                    border-radius: 0; padding: 14px; margin-bottom: 12px;
                    box-shadow: 4px 4px 0 #000;
                ",
                        h2 { style: "font-size: 13px; font-weight: 700; color: #00e5ff; text-transform: uppercase; letter-spacing: 1px; text-shadow: 2px 2px 0 #000; margin-bottom: 10px;", "{your_info}" }
                        // Telegram already told us the name, so we do not ask
                        // for it. The field only appears when we genuinely have
                        // nothing — asking for data we hold is a field whose
                        // only possible outcome is failing to fill it in.
                        if telegram_name.is_some() && !editing_name() {
                            div { style: "display:flex;align-items:baseline;gap:8px;flex-wrap:wrap;margin-bottom:8px;min-width:0;",
                                span { style: "font-size:13px;color:#8b8b9e;", "{t(lang, T_CHECKOUT_NAME_LABEL)}" }
                                span { style: "font-size:15px;color:#e8e8e8;font-weight:700;min-width:0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;", "{customer_name}" }
                                button {
                                    r#type: "button",
                                    style: "background:transparent;border:none;color:#39ff14;font-size:13px;text-decoration:underline;cursor:pointer;padding:0;min-height:44px;",
                                    onclick: move |_| editing_name.set(true),
                                    "{t(lang, T_CHECKOUT_CHANGE)}"
                                }
                            }
                        } else {
                            div { style: "margin-bottom: 8px;",
                                label {
                                    r#for: "checkout-name",
                                    style: "font-size: 13px; color: #8b8b9e; display: block; margin-bottom: 4px;",
                                    "{t(lang, T_CHECKOUT_NAME_LABEL)}"
                                }
                                input {
                                    id: "checkout-name",
                                    style: "
                                    font-size: 15px; width: 100%; padding: 10px 12px;
                                    background: #0f0f1a; color: #e8e8e8;
                                    border: 4px solid {name_border}; border-radius: 0;
                                    box-sizing: border-box;
                                ",
                                    r#type: "text",
                                    autocomplete: "name",
                                    placeholder: "{t(lang, T_CHECKOUT_NAME_PLACEHOLDER)}",
                                    aria_label: "{t(lang, T_CHECKOUT_NAME_LABEL)}",
                                    value: "{customer_name}",
                                    oninput: move |e| customer_name.set(e.value()),
                                }
                                if let Some(err) = name_error() {
                                    p { style: "font-size: 12px; color: #ff4757; margin-top: 4px;", "{err}" }
                                }
                            }
                        }
                        div { style: "margin-bottom: 8px;",
                            label {
                                r#for: "checkout-phone",
                                style: "font-size: 13px; color: #8b8b9e; display: block; margin-bottom: 4px;",
                                "{t(lang, T_CHECKOUT_PHONE_LABEL)}"
                            }
                            // Telegram never puts the phone number in initData —
                            // it is only handed over if the user agrees. So this
                            // cannot be filled silently like the name; the best
                            // available is one tap instead of typing it out.
                            if can_request_contact && customer_phone().trim().is_empty() {
                                button {
                                    r#type: "button",
                                    style: "width:100%;padding:12px;margin-bottom:6px;background:#39ff14;color:#000;border:3px solid #2d9e0f;font-size:14px;font-weight:700;cursor:pointer;box-shadow:2px 2px 0 #000;",
                                    onclick: move |_| {
                                        TelegramApp::init().request_contact();
                                    },
                                    "{t(lang, T_CHECKOUT_PHONE_FROM_TELEGRAM)}"
                                }
                            }
                            input {
                                id: "checkout-phone",
                                style: "
                                font-size: 15px; width: 100%; padding: 10px 12px;
                                background: #0f0f1a; color: #e8e8e8;
                                border: 4px solid {phone_border}; border-radius: 0;
                                box-sizing: border-box;
                            ",
                                r#type: "tel",
                                autocomplete: "tel",
                                placeholder: "{t(lang, T_CHECKOUT_PHONE_PLACEHOLDER)}",
                                aria_label: "{t(lang, T_CHECKOUT_PHONE_LABEL)}",
                                value: "{customer_phone}",
                                oninput: move |e| customer_phone.set(e.value()),
                            }
                            if let Some(err) = phone_error() {
                                p { style: "font-size: 12px; color: #ff4757; margin-top: 4px;", "{err}" }
                            }
                        }
                        // Age gate: explicit 20+ confirmation required to place an order.
                        div { style: "display: flex; align-items: flex-start; gap: 8px; margin-top: 8px;",
                            input {
                                r#type: "checkbox",
                                id: "age-confirm",
                                checked: "{age_confirmed()}",
                                style: "width: 20px; height: 20px; margin-top: 2px; cursor: pointer; accent-color: #39ff14;",
                                onclick: move |_| {
                                    age_confirmed.set(!age_confirmed());
                                },
                            }
                            label {
                                r#for: "age-confirm",
                                style: "font-size: 13px; color: #8b8b9e; cursor: pointer; line-height: 1.4;",
                                "{t(lang, T_CHECKOUT_AGE_CONFIRM)}"
                            }
                        }
                        p { style: "font-size: 11px; color: #8b8b9e; margin-top: 6px; line-height: 1.4;", "{t(lang, T_CHECKOUT_AGE_NOTICE)}" }
                    }

                    // Shop selection
                    div { style: "
                    background: #16213e; border: 4px solid #2a2a4a;
                    border-radius: 0; padding: 14px; margin-bottom: 12px;
                    box-shadow: 4px 4px 0 #000;
                ",
                        h2 { style: "font-size: 13px; font-weight: 700; color: #00e5ff; text-transform: uppercase; letter-spacing: 1px; text-shadow: 2px 2px 0 #000; margin-bottom: 10px;", "{pickup_location}" }
                        for (idx, (name, address)) in shops.iter().enumerate() {
                            {
                                let is_selected = shop_selected() == idx;
                                let border = if is_selected { "#39ff14" } else { "#2a2a4a" };
                                let bg = if is_selected { "rgba(57,255,20,0.08)" } else { "transparent" };
                                let shop_name = name.to_string();
                                let shop_addr = address.to_string();
                                let idx_val = idx;
                                rsx! {
                                    div {
                                        style: "
                                        background: {bg}; border: 4px solid {border};
                                        border-radius: 0; padding: 10px; margin-bottom: 6px;
                                        cursor: pointer;
                                    ",
                                        onclick: move |_| shop_selected.set(idx_val),
                                        div { style: "font-size: 15px; margin-bottom: 2px;", "{shop_name}" }
                                        div { style: "font-size: 13px; color: #8b8b9e;", "{shop_addr}" }
                                    }
                                }
                            }
                        }
                        div {
                            style: "margin-top:8px;cursor:pointer;font-size:13px;color:#00e5ff;text-decoration:underline;text-align:center;",
                            onclick: move |_| {
                                let _ = window().and_then(|w| w.open_with_url_and_target("https://www.google.com/maps/search/?api=1&query=Kamala+Beach+Phuket", "_blank").ok());
                            },
                            "{t(lang, T_CHECKOUT_OPEN_MAP)}"
                        }
                    }

                    // Fulfillment mode. Pickup drops the address requirement —
                    // before this existed an in-store order could not be placed.
                    div { style: "
                    background: #16213e; border: 4px solid #2a2a4a;
                    border-radius: 0; padding: 14px; margin-bottom: 12px;
                    box-shadow: 4px 4px 0 #000;
                ",
                        h2 { style: "font-size: 13px; font-weight: 700; color: #00e5ff; text-transform: uppercase; letter-spacing: 1px; text-shadow: 2px 2px 0 #000; margin-bottom: 10px;", "{t(lang, T_CHECKOUT_FULFILLMENT)}" }
                        div { style: "display:flex; gap:8px;",
                            for (mode, label) in [
                                (Fulfillment::Delivery, t(lang, T_CHECKOUT_FULFILLMENT_DELIVERY)),
                                (Fulfillment::Pickup, t(lang, T_CHECKOUT_FULFILLMENT_PICKUP)),
                            ] {
                                {
                                    let is_on = fulfillment() == mode;
                                    let border = if is_on { "#39ff14" } else { "#2a2a4a" };
                                    let bg = if is_on { "rgba(57,255,20,0.08)" } else { "transparent" };
                                    let color = if is_on { "#39ff14" } else { "#8b8b9e" };
                                    rsx! {
                                        button {
                                            r#type: "button",
                                            style: "
                                            flex: 1; font-size: 14px; font-weight: 700;
                                            padding: 12px 8px; min-height: 44px;
                                            background: {bg}; color: {color};
                                            border: 4px solid {border}; border-radius: 0;
                                            cursor: pointer;
                                        ",
                                            onclick: move |_| fulfillment.set(mode),
                                            "{label}"
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // Delivery info
                    div { style: "
                    background: #16213e; border: 4px solid #2a2a4a;
                    border-radius: 0; padding: 14px; margin-bottom: 12px;
                    box-shadow: 4px 4px 0 #000;
                ",
                        h2 { style: "font-size: 13px; font-weight: 700; color: #00e5ff; text-transform: uppercase; letter-spacing: 1px; text-shadow: 2px 2px 0 #000; margin-bottom: 10px;", "{delivery}" }
                        div { style: "margin-bottom: 8px;",
                            label {
                                r#for: "checkout-address",
                                style: "font-size: 13px; color: #8b8b9e; display: block; margin-bottom: 4px;",
                                "{t(lang, T_CHECKOUT_ADDRESS_LABEL)}"
                            }
                            textarea {
                                id: "checkout-address",
                                style: "
                                font-size: 15px; width: 100%; min-height: 72px; padding: 10px 12px;
                                background: #0f0f1a; color: #e8e8e8;
                                border: 4px solid {address_border}; border-radius: 0;
                                box-sizing: border-box; resize: vertical;
                            ",
                                autocomplete: "street-address",
                                placeholder: "{t(lang, T_CHECKOUT_ADDRESS_PLACEHOLDER)}",
                                aria_label: "{t(lang, T_CHECKOUT_ADDRESS_LABEL)}",
                                value: "{delivery_address}",
                                oninput: move |e| delivery_address.set(e.value()),
                            }
                            button {
                                style: "
                                margin-top: 6px; font-size: 13px; color: #39ff14;
                                background: transparent; border: none; padding: 0;
                                cursor: pointer; min-width: 44px; min-height: 44px;
                                text-align: left;
                            ",
                                r#type: "button",
                                onclick: move |_| { fill_address_from_geolocation(delivery_address); },
                                "{t(lang, T_CHECKOUT_USE_MY_LOCATION)}"
                            }
                            if let Some(err) = address_error() {
                                p { style: "font-size: 12px; color: #ff4757; margin-top: 4px;", "{err}" }
                            }
                        }
                        div { style: "margin-bottom: 8px;",
                            label {
                                r#for: "checkout-notes",
                                style: "font-size: 13px; color: #8b8b9e; display: block; margin-bottom: 4px;",
                                "{t(lang, T_CHECKOUT_NOTES_LABEL)}"
                            }
                            textarea {
                                id: "checkout-notes",
                                style: "
                                font-size: 15px; width: 100%; min-height: 56px; padding: 10px 12px;
                                background: #0f0f1a; color: #e8e8e8;
                                border: 4px solid #2a2a4a; border-radius: 0;
                                box-sizing: border-box; resize: vertical;
                            ",
                                autocomplete: "off",
                                placeholder: "{t(lang, T_CHECKOUT_NOTES_PLACEHOLDER)}",
                                aria_label: "{t(lang, T_CHECKOUT_NOTES_LABEL)}",
                                value: "{delivery_notes}",
                                oninput: move |e| delivery_notes.set(e.value()),
                            }
                        }
                        // Zone selector: native <select> is faster to tap and scroll than a
                        // stack of custom divs, and it respects the user's keyboard on
                        // devices without touch.
                        if !zones.is_empty() {
                            div { style: "margin-bottom: 8px;",
                                label {
                                    r#for: "checkout-zone",
                                    style: "font-size: 13px; color: #8b8b9e; display: block; margin-bottom: 4px;",
                                    "{delivery_zone_label}"
                                }
                                select {
                                    id: "checkout-zone",
                                    style: "
                                    width: 100%; font-size: 15px; padding: 10px 12px;
                                    background: #0f0f1a; color: #e8e8e8;
                                    border: 4px solid #2a2a4a; border-radius: 0;
                                    box-sizing: border-box; cursor: pointer;
                                ",
                                    aria_label: "{delivery_zone_label}",
                                    onchange: move |e: Event<FormData>| {
                                        let v = e.value();
                                        if !v.is_empty() {
                                            delivery_zone_id.set(Some(v));
                                        }
                                    },
                                    option {
                                        value: "",
                                        disabled: true,
                                        selected: selected_zone.is_none(),
                                        "{t(lang, T_CHECKOUT_SELECT_ZONE)}"
                                    }
                                    {
                                        zones.iter().map(|z| {
                                            let zid = z.id.clone();
                                            let zname = zone_display_name(z);
                                            let is_selected = selected_zone.as_ref().map(|s| s.id == zid).unwrap_or(false);
                                            rsx! {
                                                option {
                                                    key: "{zid}",
                                                    value: "{zid}",
                                                    selected: is_selected,
                                                    "{zname}"
                                                }
                                            }
                                        })
                                    }
                                }
                            }
                        }
                        div { style: "
                        background: rgba(0,229,255,0.05);
                        border: 4px solid rgba(0,229,255,0.2);
                        border-radius: 0; padding: 10px;
                    ",
                            if let Some(eta) = delivery_eta_text.as_ref() { div { style: "font-size: 13px; margin-bottom: 6px;", "{eta}" } }
                            div { style: "font-size: 15px; color: #39ff14;", "{delivery_fee_text}" }
                        }
                    }

                    // Payment method
                    div { style: "
                    background: #16213e; border: 4px solid #2a2a4a;
                    border-radius: 0; padding: 14px; margin-bottom: 16px;
                    box-shadow: 4px 4px 0 #000;
                ",
                        h2 { style: "font-size: 13px; font-weight: 700; color: #00e5ff; text-transform: uppercase; letter-spacing: 1px; text-shadow: 2px 2px 0 #000; margin-bottom: 10px;", "{payment}" }
                        div { style: "
                        display: flex; align-items: center; gap: 8px;
                        background: rgba(57,255,20,0.05);
                        border: 4px solid #39ff14; border-radius: 0; padding: 10px;
                    ",
                            div { style: "font-size: 13px;", "💳" }
                            div { style: "flex: 1;",
                                div { style: "font-size: 15px; color: #39ff14;", "{t(lang, T_CHECKOUT_CASH_ON_DELIVERY)}" }
                                div { style: "font-size: 13px; color: #8b8b9e; margin-top: 2px;", "{t(lang, T_CHECKOUT_PAY_ON_RECEIVE)}" }
                            }
                        }
                    }

                    // Trust micro-copy
                    div { style: "
                    background: rgba(57,255,20,0.05);
                    border: 4px solid rgba(57,255,20,0.2);
                    border-radius: 0; padding: 12px; margin-bottom: 16px;
                ",
                        div { style: "font-size: 13px; color: #39ff14; font-weight: 700; margin-bottom: 8px; text-transform: uppercase; letter-spacing: 1px;", "{t(lang, T_CHECKOUT_TRUST_TITLE)}" }
                        div { style: "display: flex; flex-direction: column; gap: 6px; font-size: 12px; color: #8b8b9e;",
                            div { "{t(lang, T_CHECKOUT_TRUST_VERIFIED)}" }
                            div { "{t(lang, T_CHECKOUT_TRUST_COD)}" }
                            div { "{t(lang, T_CHECKOUT_TRUST_SECURE)}" }
                        }
                    }

                    // Error display
                    ErrorBanner {
                        message: order_error.read().clone().unwrap_or_default(),
                        icon: Some("❌".to_string()),
                    }

                    if order_error.read().is_some() {
                        button {
                            style: "
                            width: 100%; font-size: 14px; font-weight: 700;
                            padding: 12px 20px; margin-top: 10px;
                            background: #2a2a4a; color: #e8e8e8;
                            border: 4px solid #1a1a2e; border-radius: 0;
                            cursor: pointer; box-shadow: 3px 3px 0 #000;
                        ",
                            disabled: is_processing(),
                            onclick: move |_| { submit_order.call(()); },
                            "{t(lang, T_CHECKOUT_RETRY)}"
                        }
                    }

                    // Why the order button is not clickable yet. A disabled
                    // button on its own told the customer nothing, so an
                    // unticked age box or a rejected phone looked like the app
                    // being broken.
                    {
                        let blockers = checkout_blockers(
                            telegram_id.is_some(),
                            &customer_name(),
                            &customer_phone(),
                            &delivery_address(),
                            fulfillment(),
                            cart_items.len(),
                            age_confirmed(),
                        );
                        if blockers.is_empty() {
                            rsx! {}
                        } else {
                            rsx! {
                                div { style: "
                                background: rgba(255,71,87,0.08);
                                border: 4px solid rgba(255,71,87,0.35);
                                border-radius: 0; padding: 12px; margin-bottom: 12px;
                            ",
                                    div { style: "font-size: 12px; color: #ff4757; font-weight: 700; margin-bottom: 6px; text-transform: uppercase; letter-spacing: 1px;", "{t(lang, T_CHECKOUT_BLOCKED_TITLE)}" }
                                    ul { style: "margin: 0; padding-left: 18px; display: flex; flex-direction: column; gap: 4px;",
                                        for b in blockers.iter() {
                                            li { style: "font-size: 13px; color: #e8e8e8;", "{t(lang, b.message_key())}" }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // Actions
                    div { style: "display: flex; gap: 10px;",
                        Link { to: Route::Cart {},
                            button { style: "
                            font-size: 14px; font-weight: 700; flex: 1; padding: 12px 20px;
                            background: transparent; color: #e8e8e8;
                            border: 4px solid #2a2a4a; border-radius: 0;
                            cursor: pointer; box-shadow: 3px 3px 0 #000;
                            transition: transform 0.1s, box-shadow 0.1s;
                        ", "{back}" }
                        }
                        {
                            let blockers = checkout_blockers(
                                telegram_id.is_some(),
                                &customer_name(),
                                &customer_phone(),
                                &delivery_address(),
                                fulfillment(),
                                cart_items.len(),
                                age_confirmed(),
                            );
                            let can_order = blockers.is_empty() && !is_processing();
                            let btn_bg = if can_order { "#39ff14" } else { "#2a2a4a" };
                            let btn_color = if can_order { "#000" } else { "#8b8b9e" };
                            let btn_cursor = if can_order { "pointer" } else { "not-allowed" };
                            let processing = is_processing();
                            let opacity = if processing { "0.7" } else { "1.0" };
                            rsx! {
                                button {
                                    style: "
                                    font-size: 14px; font-weight: 700; flex: 2; padding: 12px 20px;
                                    background: {btn_bg};
                                    color: {btn_color};
                                    border: 4px solid #2d9e0f; border-radius: 0;
                                    cursor: {btn_cursor};
                                    box-shadow: 3px 3px 0 #000;
                                    transition: transform 0.1s, box-shadow 0.1s;
                                    opacity: {opacity};
                                ",
                                    disabled: !can_order,
                                    onclick: move |_| { submit_order.call(()); },
                                    if processing { "{t(lang, T_CHECKOUT_PROCESSING)}" } else { "{place_order}" }
                                }
                            }
                        }
                    }
                }
            }
        }
}

/// The zone id an order carries: the saved one only while the picker shows
/// that same zone (`served_zone_id`). A returning customer's saved id can name
/// a zone migration 087 deactivated; the picker shows the first served zone in
/// its place, and the raw id used to be submitted anyway, for a 422.
fn zone_to_submit(stored: Option<String>, shown: Option<&DeliveryZone>) -> Option<String> {
    crate::trios::store::served_zone_id(stored, shown.map(|z| z.id.as_str()))
}
