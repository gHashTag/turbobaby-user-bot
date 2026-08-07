use crate::trios::core::Lang;
use crate::trios::i18n::{
    t, tf,
    T_BACK, T_CHECKOUT_AGE_CONFIRM, T_CHECKOUT_AGE_NOTICE, T_CHECKOUT_CART_EMPTY,
    T_CHECKOUT_CASH_ON_DELIVERY, T_CHECKOUT_ERR_400, T_CHECKOUT_ERR_ADDRESS,
    T_CHECKOUT_ERR_ADDRESS_LONG, T_CHECKOUT_ERR_ITEMS, T_CHECKOUT_ERR_NAME,
    T_CHECKOUT_ERR_NAME_LONG, T_CHECKOUT_ERR_NETWORK, T_CHECKOUT_ERR_NO_TELEGRAM,
    T_CHECKOUT_ERR_PARSE, T_CHECKOUT_ERR_PHONE, T_CHECKOUT_ERR_PHONE_INVALID,
    T_CHECKOUT_ERR_PHONE_LONG, T_CHECKOUT_GARDEN_DISCOUNT, T_CHECKOUT_GARDEN_DISCOUNT_PCT,
    T_CHECKOUT_NAME_LABEL, T_CHECKOUT_NAME_PLACEHOLDER, T_CHECKOUT_NOTES_LABEL,
    T_CHECKOUT_NOTES_PLACEHOLDER, T_CHECKOUT_OPEN_MAP, T_CHECKOUT_PAY_ON_RECEIVE,
    T_CHECKOUT_PHONE_LABEL, T_CHECKOUT_PHONE_PLACEHOLDER, T_CHECKOUT_PROCESSING,
    T_CHECKOUT_SELECT_ZONE, T_CHECKOUT_STARS, T_CHECKOUT_STARS_AVAILABLE,
    T_CHECKOUT_STARS_MINUS, T_CHECKOUT_STEP_CART, T_CHECKOUT_STEP_DETAILS,
    T_CHECKOUT_STEP_CONFIRM, T_CHECKOUT_TITLE, T_CHECKOUT_TRUST_COD,
    T_CHECKOUT_TRUST_SECURE, T_CHECKOUT_TRUST_TITLE, T_CHECKOUT_TRUST_VERIFIED,
    T_CHECKOUT_ADDRESS_LABEL, T_CHECKOUT_ADDRESS_PLACEHOLDER,
    T_DELIVERY, T_DELIVERY_ETA, T_DELIVERY_FEE, T_DELIVERY_ZONE, T_PAYMENT,
    T_PICKUP_LOCATION, T_PLACE_ORDER, T_TOTAL, T_YOUR_INFO, T_YOUR_ORDER,
};
use crate::trios::store::validate_checkout;
use crate::ui::api::context::api_base_url;
use crate::ui::api::types::{DeliveryZone, DeliveryZonesResponse};
use crate::ui::components::error_banner::ErrorBanner;
use crate::ui::routes::Route;
use crate::ui::state::{Cart, CartItem, CartItemType};
use crate::ui::telegram::{use_telegram_id, use_telegram_init_data, use_telegram_username, use_main_button_click, TelegramApp, HapticNotification};
use dioxus::prelude::*;
use serde_json::json;
use web_sys::window;

/// B4: a garden reward the customer can apply at checkout (product-scoped).
#[derive(Clone, serde::Deserialize)]
struct ApiReward {
    id: String,
    #[serde(default)]
    discount_percent: u32,
    #[serde(default)]
    is_active: bool,
    #[serde(default)]
    scope: String,
    #[serde(default)]
    target_product_id: Option<String>,
}

#[derive(serde::Deserialize)]
struct RewardsResp {
    #[serde(default)]
    rewards: Vec<ApiReward>,
}

#[derive(serde::Deserialize)]
struct StarsBalanceResp {
    balance: i64,
}

fn zone_display_name(zone: &DeliveryZone) -> String {
    if crate::ui::lang::current_lang() == Lang::English {
        zone.name_en.clone().unwrap_or_else(|| zone.name.clone())
    } else {
        zone.name.clone()
    }
}

fn phone_valid(phone: &str) -> bool {
    let digits = phone.chars().filter(|c| c.is_ascii_digit()).count();
    // Allow any non-empty string that starts with + and has at least 5 digits.
    phone.starts_with('+') && digits >= 5
}

/// Real-time, per-field validation used for inline checkout feedback.
/// Returns the translation key for the first problem, or None if the field
/// looks acceptable so far (empty fields are reported so the user sees the
/// required indicator while typing).
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
    let cart_total = cart.read().total;
    let mut customer_name = use_signal(String::new);
    let mut customer_phone = use_signal(String::new);
    let mut delivery_address = use_signal(String::new);
    let mut delivery_notes = use_signal(String::new);
    let mut delivery_zone_id = use_signal(|| Option::<String>::None);
    // Restore the customer's last used delivery details so repeat buyers don't
    // retype name/phone/address every order.
    use_effect(move || {
        if let Some(window) = web_sys::window() {
            if let Ok(Some(storage)) = window.local_storage() {
                if let Ok(Some(v)) = storage.get_item("woody_last_name") {
                    customer_name.set(v);
                }
                if let Ok(Some(v)) = storage.get_item("woody_last_phone") {
                    customer_phone.set(v);
                }
                if let Ok(Some(v)) = storage.get_item("woody_last_address") {
                    delivery_address.set(v);
                }
                if let Ok(Some(v)) = storage.get_item("woody_last_zone_id") {
                    delivery_zone_id.set(Some(v));
                }
            }
        }
    });
    let mut shop_selected = use_signal(|| 0usize);
    let mut is_processing = use_signal(|| false);
    let mut order_error = use_signal(|| Option::<String>::None);
    // Cycle #57: stable idempotency key per logical submit. Lazy-init on the
    // first click and reused for retries within this mount so server
    // collapses them into one order (migration 029). Navigating away or
    // unmounting the screen resets — exactly the boundary we want.
    let mut idempotency_key = use_signal(|| Option::<String>::None);
    // Age gate: the user must explicitly confirm they are 20+ before placing
    // an order. This drives both the in-app primary button and Telegram
    // MainButton enabled state.
    let mut age_confirmed = use_signal(|| false);
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
    let name_error = use_memo(move || validate_checkout_field(&customer_name(), "name").map(|k| t(lang, k).to_string()));
    let phone_error = use_memo(move || validate_checkout_field(&customer_phone(), "phone").map(|k| t(lang, k).to_string()));
    let address_error = use_memo(move || validate_checkout_field(&delivery_address(), "address").map(|k| t(lang, k).to_string()));
    let name_border = if name_error().is_some() { "#ff4757" } else { "#2a2a4a" };
    let phone_border = if phone_error().is_some() { "#ff4757" } else { "#2a2a4a" };
    let address_border = if address_error().is_some() { "#ff4757" } else { "#2a2a4a" };

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
    let shops = [(
        "🏠 Woody Weed Pecker",
        "44, 129, Koh Phangan, Surat Thani 84280",
    )];

    // B4: fetch the user's garden rewards; show a toggle for any product-scoped,
    // still-active reward whose target product is in this cart.
    let mut applied_reward = use_signal(|| Option::<(String, f64)>::None);
    // Stars (⭐) the user wants to spend as internal-currency discount.
    let mut stars_to_use = use_signal(|| 0i64);
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
    let rewards_res = {
        let init = init_data.clone();
        use_resource(move || {
            let init = init.clone();
            async move {
                let tid = telegram_id?;
                let url = format!("{}/api/garden/rewards?telegram_id={}", api_base_url(), tid);
                let text = crate::ui::api::http::fetch_text_authed(&url, &init)
                    .await
                    .ok()?;
                serde_json::from_str::<RewardsResp>(&text)
                    .ok()
                    .map(|r| r.rewards)
            }
        })
    };
    // Delivery zones/ETA are public; no auth header needed.
    let zones_res = use_resource(move || async move {
        let url = format!("{}/api/delivery/zones", api_base_url());
        let text = crate::ui::api::http::fetch_text(&url).await.ok()?;
        serde_json::from_str::<DeliveryZonesResponse>(&text)
            .ok()
            .map(|r| r.zones)
    });
    // (reward, discount_amount) pairs applicable to this cart.
    let applicable_rewards: Vec<(ApiReward, f64, String)> = match &*rewards_res.read() {
        Some(Some(list)) => list
            .iter()
            .filter(|r| r.is_active && r.scope == "product")
            .filter_map(|r| {
                let tpid = r.target_product_id.as_deref()?;
                let item = cart_items.iter().find(|i| i.id == tpid)?;
                let unit = if item.price.is_finite() {
                    item.price.max(0.0)
                } else {
                    0.0
                };
                let disc =
                    (unit * item.quantity as f64 * r.discount_percent as f64 / 100.0).max(0.0);
                Some((r.clone(), disc, item.name.clone()))
            })
            .collect(),
        _ => Vec::new(),
    };
    let applied_discount = applied_reward
        .read()
        .as_ref()
        .map(|(_, d)| *d)
        .unwrap_or(0.0);
    let pre_stars_total = (cart_total - applied_discount).max(0.0);
    let max_stars = (pre_stars_total.floor() as i64).min(stars_balance).max(0);
    let stars_val = (*stars_to_use.read()).clamp(0, max_stars.max(0));
    let effective_total = (pre_stars_total - stars_val as f64).max(0.0);

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
    let (delivery_eta_text, delivery_fee_text) = match selected_zone.as_ref() {
        Some(z) => {
            let eta = format!("{}-{}", z.min_eta_minutes, z.max_eta_minutes);
            let eta_text = t(crate::ui::lang::current_lang(), T_DELIVERY_ETA).replace("{0}", &eta);
            let fee_text = t(crate::ui::lang::current_lang(), T_DELIVERY_FEE)
                .replace("{0}", &format!("{:.0}", z.delivery_fee_baht));
            (eta_text, fee_text)
        }
        None => (
            tf(lang, T_DELIVERY_ETA, &["30-45".to_string()]),
            tf(lang, T_DELIVERY_FEE, &["0".to_string()]),
        ),
    };

    // Sync the native Telegram MainButton with the live total and form validity.
    use_effect(move || {
        let lang = crate::ui::lang::current_lang();
        let total_str = crate::trios::pricing::format_baht(effective_total);
        tg.set_main_button_text(&format!("{} — {}", t(lang, T_PLACE_ORDER), total_str));
        let valid = telegram_id.is_some()
            && age_confirmed()
            && !customer_name().trim().is_empty()
            && name_error().is_none()
            && !customer_phone().trim().is_empty()
            && phone_error().is_none()
            && !delivery_address().trim().is_empty()
            && address_error().is_none()
            && !is_processing();
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
        if let Err(e) = validate_checkout(&customer_name(), &customer_phone(), &delivery_address(), &trios_items) {
            let key = match e {
                crate::trios::core::Error::Validation(msg) if msg.contains("Name is required") => T_CHECKOUT_ERR_NAME,
                crate::trios::core::Error::Validation(msg) if msg.contains("Name is too long") => T_CHECKOUT_ERR_NAME_LONG,
                crate::trios::core::Error::Validation(msg) if msg.contains("Phone is required") => T_CHECKOUT_ERR_PHONE,
                crate::trios::core::Error::Validation(msg) if msg.contains("Phone is too long") => T_CHECKOUT_ERR_PHONE_LONG,
                crate::trios::core::Error::Validation(msg) if msg.contains("Invalid phone number") => T_CHECKOUT_ERR_PHONE_INVALID,
                crate::trios::core::Error::Validation(msg) if msg.contains("Delivery address is required") => T_CHECKOUT_ERR_ADDRESS,
                crate::trios::core::Error::Validation(msg) if msg.contains("Delivery address is too long") => T_CHECKOUT_ERR_ADDRESS_LONG,
                crate::trios::core::Error::Validation(msg) if msg.contains("Cart cannot be empty") => T_CHECKOUT_ERR_ITEMS,
                _ => T_CHECKOUT_ERR_400,
            };
            order_error.set(Some(t(lang, key).to_string()));
            tg.haptic_notification(HapticNotification::Warning);
            return;
        }
        is_processing.set(true);
        order_error.set(None);

        // Show the native Telegram MainButton spinner while the network request runs.
        tg.show_main_button_progress(&t(lang, T_CHECKOUT_PROCESSING), true);
        let restore_text = format!(
            "{} — {}",
            t(lang, T_PLACE_ORDER),
            crate::trios::pricing::format_baht(effective_total)
        );
        let zone_info = submit_selected_zone.clone();

        let base = api_base_url();
        let client = crate::ui::api::local_client::LocalClient::new();
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

        // B4: apply the selected garden reward (server re-verifies the amount).
        let (garden_reward_id, garden_discount) = match applied_reward.read().clone() {
            Some((rid, d)) => (Some(rid), d),
            None => (None, 0.0),
        };
        let submit_stars = (*stars_to_use.read()).clamp(0, max_stars);
        let order_total = (cart_total - garden_discount - submit_stars as f64).max(0.0);

        let body = json!({
            "telegram_id": telegram_id,
            "customer_name": customer_name(),
            "customer_phone": customer_phone(),
            "customer_telegram": telegram_username.clone(),
            "items": items_json,
            "subtotal": cart_total,
            "bonus_used": null,
            "stars_used": submit_stars,
            "total": order_total,
            "garden_reward_id": garden_reward_id,
            "shop_id": shops[shop_selected()].0,
            "delivery_address": delivery_address(),
            "delivery_notes": delivery_notes(),
        });

        let init_data_clone = init_data.clone();
        spawn(async move {
            let res = client
                .post(&url)
                .header("Content-Type", "application/json")
                .header("X-Telegram-Init-Data", init_data_clone)
                .header("X-Idempotency-Key", key)
                .json(&body)
                .send()
                .await;

            match res {
                Ok(resp) if resp.status().is_success() => {
                    if let Ok(val) = resp.json::<serde_json::Value>().await {
                        if let Some(id) = val.get("order_id").and_then(|v| v.as_str()) {
                            let order_id = id.to_string();
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
                                        let _ = storage.set_item(
                                            "woody_last_zone_name",
                                            &zone_display_name(z),
                                        );
                                        let _ = storage.set_item(
                                            "woody_last_zone_eta",
                                            &format!("{}-{}", z.min_eta_minutes, z.max_eta_minutes),
                                        );
                                    }
                                }
                            }
                            // Stop the spinner and hide the native button before leaving the screen.
                            tg.hide_main_button_progress(&restore_text);
                            tg.hide_main_button();
                            // Navigate to success and clear cart
                            cart.write().clear();
                            tg.haptic_notification(HapticNotification::Success);
                            nav.push(Route::Success { id: order_id });
                            return;
                        }
                    }
                    tg.haptic_notification(HapticNotification::Error);
                    order_error.set(Some(t(lang, T_CHECKOUT_ERR_PARSE).to_string()));
                }
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    tg.haptic_notification(HapticNotification::Error);
                    // Cycle #65/#69 friendly per-status; cycle #70 sources
                    // `lang` from `?lang=xx` so a non-RU Telegram client gets
                    // localised "Account restricted" instead of Cyrillic.
                    order_error.set(Some(crate::trios::checkout_errors::friendly_order_error(
                        lang, status,
                    )));
                }
                Err(_) => {
                    tg.haptic_notification(HapticNotification::Error);
                    order_error.set(Some(t(lang, T_CHECKOUT_ERR_NETWORK).to_string()));
                }
            }
            // Restore the MainButton label and stop the spinner on any error.
            tg.hide_main_button_progress(&restore_text);
            is_processing.set(false);
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
            padding-bottom: 80px;
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
                                if let Some(url) = item.image_url.as_ref() {
                                    img {
                                        src: "{url}",
                                        alt: "{item.name}",
                                        style: "width:44px; height:44px; object-fit:cover; border:2px solid #2a2a4a; flex-shrink:0;",
                                        loading: "lazy",
                                    }
                                }
                                div { style: "flex:1; min-width:0;",
                                    div { style: "font-size:13px; color:#e8e8e8; white-space:nowrap; overflow:hidden; text-overflow:ellipsis;", "{item.name}" }
                                    div { style: "font-size:12px; color:#8b8b9e;", "x{item.quantity}" }
                                }
                                div { style: "font-size:13px; font-weight:700; color:#e8e8e8;",
                                    { crate::trios::pricing::format_baht(item.price * item.quantity as f64) }
                                }
                            }
                        }
                        // B4: garden discount picker — toggle a product-scoped reward.
                        if !applicable_rewards.is_empty() {
                            div { style: "border-top:1px solid #2a2a4a;margin-top:8px;padding-top:8px;",
                                div { style: "font-size:12px;color:#39ff14;font-weight:700;margin-bottom:6px;", "{t(lang, T_CHECKOUT_GARDEN_DISCOUNT)}" }
                                for (r, disc, tname) in applicable_rewards.iter() {
                                    {
                                        let rid = r.id.clone();
                                        let tname = tname.clone();
                                        let pct = r.discount_percent;
                                        let d = *disc;
                                        let is_on = applied_reward.read().as_ref().map(|(id, _)| id == &rid).unwrap_or(false);
                                        let disc_str = crate::trios::pricing::format_baht(d);
                                        let border = if is_on { "#39ff14" } else { "#2a2a4a" };
                                        let amt_style = if is_on { "font-size:12px;color:#39ff14;font-weight:700;" } else { "font-size:12px;color:#8b8b9e;" };
                                        let reward_label = tf(lang, T_CHECKOUT_GARDEN_DISCOUNT_PCT, &[pct.to_string(), tname]);
                                        rsx! {
                                            div {
                                                style: "display:flex;justify-content:space-between;align-items:center;gap:8px;padding:6px;border:3px solid {border};cursor:pointer;margin-bottom:6px;",
                                                onclick: move |_| {
                                                    if applied_reward.read().as_ref().map(|(id, _)| id == &rid).unwrap_or(false) {
                                                        applied_reward.set(None);
                                                    } else {
                                                        applied_reward.set(Some((rid.clone(), d)));
                                                    }
                                                },
                                                span { style: "font-size:12px;color:#e8e8e8;", "{reward_label}" }
                                                span { style: "{amt_style}", "−{disc_str}" }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        // Stars discount picker.
                        if stars_balance > 0 && pre_stars_total > 0.0 {
                            div { style: "border-top:1px solid #2a2a4a;margin-top:8px;padding-top:8px;",
                                div { style: "display:flex;justify-content:space-between;align-items:center;margin-bottom:6px;",
                                    div { style: "font-size:12px;color:#7dd3fc;font-weight:700;", "{t(lang, T_CHECKOUT_STARS)}" }
                                    div { style: "font-size:12px;color:#8b8b9e;", "{tf(lang, T_CHECKOUT_STARS_AVAILABLE, &[stars_balance.to_string()])}" }
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
                                    span { style: "font-size:12px;color:#7dd3fc;", "{tf(lang, T_CHECKOUT_STARS_MINUS, &[stars_val.to_string()])}" }
                                }
                            }
                        }
                        div { style: "display: flex; justify-content: space-between; font-size: 15px; font-weight: 800; padding-top: 8px; border-top: 1px solid #2a2a4a; margin-top: 8px;",
                            span { "{total_label}" }
                            {
                                let total_str = crate::trios::pricing::format_baht(effective_total);
                                rsx! { span { style: "font-size: 20px; font-weight: 800; color: #ffe600; text-shadow: 2px 2px 0 #000;", "{total_str}" } }
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
                    div { style: "margin-bottom: 8px;",
                        label { style: "font-size: 13px; color: #8b8b9e; display: block; margin-bottom: 4px;", "{t(lang, T_CHECKOUT_NAME_LABEL)}" }
                        input {
                            style: "
                                font-size: 15px; width: 100%; padding: 10px 12px;
                                background: #0f0f1a; color: #e8e8e8;
                                border: 4px solid {name_border}; border-radius: 0;
                                box-sizing: border-box;
                            ",
                            r#type: "text",
                            autocomplete: "name",
                            placeholder: "{t(lang, T_CHECKOUT_NAME_PLACEHOLDER)}",
                            value: "{customer_name}",
                            oninput: move |e| customer_name.set(e.value()),
                        }
                        if let Some(err) = name_error() {
                            p { style: "font-size: 12px; color: #ff4757; margin-top: 4px;", "{err}" }
                        }
                    }
                    div { style: "margin-bottom: 8px;",
                        label { style: "font-size: 13px; color: #8b8b9e; display: block; margin-bottom: 4px;", "{t(lang, T_CHECKOUT_PHONE_LABEL)}" }
                        input {
                            style: "
                                font-size: 15px; width: 100%; padding: 10px 12px;
                                background: #0f0f1a; color: #e8e8e8;
                                border: 4px solid {phone_border}; border-radius: 0;
                                box-sizing: border-box;
                            ",
                            r#type: "tel",
                            autocomplete: "tel",
                            placeholder: "{t(lang, T_CHECKOUT_PHONE_PLACEHOLDER)}",
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
                            let _ = window().and_then(|w| w.open_with_url_and_target("https://www.google.com/maps/place/Woody+Weed+Pecker/@9.7124562,99.9877309,17z/data=!3m1!4b1!4m6!3m5!1s0x3054ffe9f6df4edf:0xf8735a84f5193e1a!8m2!3d9.7124562!4d99.9877309!16s%2Fg%2F11x314fym6!18m1!1e1?entry=ttu&g_ep=EgoyMDI2MDUxMy4wIKXMDSoASAFQAw%3D%3D", "_blank").ok());
                        },
                        "{t(lang, T_CHECKOUT_OPEN_MAP)}"
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
                        label { style: "font-size: 13px; color: #8b8b9e; display: block; margin-bottom: 4px;", "{t(lang, T_CHECKOUT_ADDRESS_LABEL)}" }
                        input {
                            style: "
                                font-size: 15px; width: 100%; padding: 10px 12px;
                                background: #0f0f1a; color: #e8e8e8;
                                border: 4px solid {address_border}; border-radius: 0;
                                box-sizing: border-box;
                            ",
                            r#type: "text",
                            autocomplete: "street-address",
                            placeholder: "{t(lang, T_CHECKOUT_ADDRESS_PLACEHOLDER)}",
                            value: "{delivery_address}",
                            oninput: move |e| delivery_address.set(e.value()),
                        }
                        if let Some(err) = address_error() {
                            p { style: "font-size: 12px; color: #ff4757; margin-top: 4px;", "{err}" }
                        }
                    }
                    div { style: "margin-bottom: 8px;",
                        label { style: "font-size: 13px; color: #8b8b9e; display: block; margin-bottom: 4px;", "{t(lang, T_CHECKOUT_NOTES_LABEL)}" }
                        input {
                            style: "
                                font-size: 15px; width: 100%; padding: 10px 12px;
                                background: #0f0f1a; color: #e8e8e8;
                                border: 4px solid #2a2a4a; border-radius: 0;
                                box-sizing: border-box;
                            ",
                            r#type: "text",
                            autocomplete: "off",
                            placeholder: "{t(lang, T_CHECKOUT_NOTES_PLACEHOLDER)}",
                            value: "{delivery_notes}",
                            oninput: move |e| delivery_notes.set(e.value()),
                        }
                    }
                    // Zone selector: native <select> is faster to tap and scroll than a
                    // stack of custom divs, and it respects the user's keyboard on
                    // devices without touch.
                    if !zones.is_empty() {
                        div { style: "margin-bottom: 8px;",
                            label { style: "font-size: 13px; color: #8b8b9e; display: block; margin-bottom: 4px;", "{delivery_zone_label}" }
                            select {
                                style: "
                                    width: 100%; font-size: 15px; padding: 10px 12px;
                                    background: #0f0f1a; color: #e8e8e8;
                                    border: 4px solid #2a2a4a; border-radius: 0;
                                    box-sizing: border-box; cursor: pointer;
                                ",
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
                        div { style: "font-size: 13px; margin-bottom: 6px;", "{delivery_eta_text}" }
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
                        let trios_items = to_trios_items(&cart_items);
                        let can_order = telegram_id.is_some()
                            && validate_checkout(&customer_name(), &customer_phone(), &delivery_address(), &trios_items).is_ok()
                            && age_confirmed()
                            && !is_processing();
                        if can_order {
                            tg.enable_main_button();
                        } else {
                            tg.disable_main_button();
                        }
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
