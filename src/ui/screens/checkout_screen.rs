use crate::trios::core::Lang;
use crate::trios::i18n::{
    t, T_BACK, T_CHECKOUT_AGE_DOB, T_CHECKOUT_AGE_ERROR, T_CHECKOUT_AGE_REQUIRED,
    T_CHECKOUT_AGE_UNDERAGE, T_CHECKOUT_AGE_VERIFY, T_CHECKOUT_AGE_VERIFYING,
    T_CHECKOUT_TITLE, T_DELIVERY, T_DELIVERY_ETA, T_DELIVERY_FEE, T_DELIVERY_ZONE, T_PAYMENT,
    T_PICKUP_LOCATION, T_PLACE_ORDER, T_TOTAL, T_YOUR_INFO, T_YOUR_ORDER,
};
use crate::trios::store::validate_checkout;
use crate::ui::api::context::api_base_url;
use crate::ui::api::types::{DeliveryZone, DeliveryZonesResponse};
use crate::ui::routes::Route;
use crate::ui::state::{Cart, CartItem, CartItemType};
use crate::ui::telegram::{use_telegram_id, use_telegram_init_data, use_telegram_username};
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

#[derive(serde::Deserialize)]
struct ProfileResp {
    #[serde(default)]
    age_verified: bool,
}

fn zone_display_name(zone: &DeliveryZone) -> String {
    if crate::ui::lang::current_lang() == Lang::English {
        zone.name_en.clone().unwrap_or_else(|| zone.name.clone())
    } else {
        zone.name.clone()
    }
}

fn today_iso() -> String {
    chrono::Local::now()
        .naive_local()
        .date()
        .format("%Y-%m-%d")
        .to_string()
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
    let mut shop_selected = use_signal(|| 0usize);
    let mut is_processing = use_signal(|| false);
    let mut order_error = use_signal(|| Option::<String>::None);
    // Cycle #57: stable idempotency key per logical submit. Lazy-init on the
    // first click and reused for retries within this mount so server
    // collapses them into one order (migration 029). Navigating away or
    // unmounting the screen resets — exactly the boundary we want.
    let mut idempotency_key = use_signal(|| Option::<String>::None);
    // Thailand cannabis compliance: checkout must confirm the user is 20+
    // before the backend will accept the order. We fetch the profile once,
    // show an inline date-of-birth form when missing, and block the submit
    // button until verified.
    let mut age_verified = use_signal(|| false);
    let mut age_dob = use_signal(String::new);
    let mut age_verifying = use_signal(|| false);
    let mut age_error = use_signal(|| Option::<String>::None);
    let nav = navigator();
    let telegram_id = use_telegram_id();
    let telegram_username = use_telegram_username();
    let init_data = use_telegram_init_data();

    // Cycle #73 / A: lang is now resolved once at WASM startup
    // (lib.rs::run via pick_lang) and stored in OnceLock; just read it.
    // No more per-screen inline pick_lang dance.
    let lang = crate::ui::lang::current_lang();

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
    // Fetch profile to know whether age verification is already satisfied.
    let profile_res = {
        let init = init_data.clone();
        use_resource(move || {
            let init = init.clone();
            async move {
                let tid = telegram_id?;
                let url = format!("{}/api/users/me/{}", api_base_url(), tid);
                let text = crate::ui::api::http::fetch_text_authed(&url, &init)
                    .await
                    .ok()?;
                serde_json::from_str::<ProfileResp>(&text).ok()
            }
        })
    };
    let profile_age_verified = profile_res
        .read()
        .as_ref()
        .and_then(|opt| opt.as_ref())
        .map(|p| p.age_verified)
        .unwrap_or(false);
    let age_ok = *age_verified.read() || profile_age_verified;
    // Inline age-verification banner for checkout.
    let age_gate = {
        let init_data = init_data.clone();
        move || -> Element {
            if telegram_id.is_none() || age_ok {
                return rsx! {};
            }
            let required_text = t(lang, T_CHECKOUT_AGE_REQUIRED).to_string();
            let dob_label = t(lang, T_CHECKOUT_AGE_DOB).to_string();
            let verify_label = t(lang, T_CHECKOUT_AGE_VERIFY).to_string();
            let verifying_label = t(lang, T_CHECKOUT_AGE_VERIFYING).to_string();
            let dob = age_dob.read().clone();
            let loading = *age_verifying.read();
            let err = age_error.read().clone();
            rsx! {
                div { style: "background:#2a1a0f;border:4px solid #ff9d00;padding:14px;margin-bottom:12px;box-shadow:4px 4px 0 #000;",
                    div { style: "font-size:14px;color:#ff9d00;font-weight:700;margin-bottom:8px;", "{required_text}" }
                    div { style: "display:flex;gap:8px;align-items:center;margin-bottom:8px;",
                        div { style: "font-size:13px;color:#8b8b9e;min-width:max-content;", "{dob_label}" }
                        input {
                            r#type: "date",
                            style: "flex:1;padding:8px;background:#0f0f1a;color:#e8e8e8;border:3px solid #444;font-size:14px;",
                            value: "{dob}",
                            max: "{today_iso()}",
                            disabled: loading,
                            oninput: move |e| age_dob.set(e.value()),
                        }
                        button {
                            style: "padding:8px 14px;background:#ff9d00;color:#000;border:none;font-size:14px;font-weight:700;cursor:pointer;min-width:44px;min-height:44px;",
                            disabled: loading || dob.is_empty(),
                            onclick: move |_| {
                                if dob.is_empty() { return; }
                                age_verifying.set(true);
                                age_error.set(None);
                                let init = init_data.clone();
                                let tid = telegram_id.unwrap_or(0);
                                let dob_val = dob.clone();
                                spawn(async move {
                                    let url = format!("{}/api/users/me/{}/verify-age", api_base_url(), tid);
                                    let client = crate::ui::api::local_client::LocalClient::new();
                                    let body = serde_json::json!({ "dob": dob_val });
                                    match client.post(&url)
                                        .header("X-Telegram-Init-Data", init)
                                        .json(&body)
                                        .send()
                                        .await
                                    {
                                        Ok(r) if r.status().is_success() => {
                                            if let Ok(resp) = r.json::<serde_json::Value>().await {
                                                if let Some(true) = resp.get("age_verified").and_then(|v| v.as_bool()) {
                                                    age_verified.set(true);
                                                } else {
                                                    age_error.set(Some(t(lang, T_CHECKOUT_AGE_UNDERAGE).to_string()));
                                                }
                                            } else {
                                                age_error.set(Some(t(lang, T_CHECKOUT_AGE_ERROR).to_string()));
                                            }
                                        }
                                        Ok(_) => {
                                            age_error.set(Some(t(lang, T_CHECKOUT_AGE_ERROR).to_string()));
                                        }
                                        Err(_) => {
                                            age_error.set(Some(t(lang, T_CHECKOUT_AGE_ERROR).to_string()));
                                        }
                                    }
                                    age_verifying.set(false);
                                });
                            },
                            if loading { "{verifying_label}" } else { "{verify_label}" }
                        }
                    }
                    if let Some(ref e) = err {
                        div { style: "font-size:13px;color:#ff4757;text-align:center;", "{e}" }
                    }
                }
            }
        }
    };
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
            "⏰ 30-45 min delivery".to_string(),
            "💰 Free delivery over ฿1,000".to_string(),
        ),
    };

    let submit_cart_items = cart_items.clone();
    let submit_order = move |_| {
        if is_processing() {
            return;
        }
        if telegram_id.is_none() {
            order_error.set(Some(
                "Откройте приложение в Telegram, чтобы оформить заказ".into(),
            ));
            return;
        }
        let trios_items = to_trios_items(&submit_cart_items);
        if validate_checkout(&customer_name(), &customer_phone(), &trios_items).is_err() {
            return;
        }
        is_processing.set(true);
        order_error.set(None);

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
                }),
                CartItemType::Accessory => json!({
                    "accessory_id": item.id,
                    "accessory_name": item.name,
                    "quantity": item.quantity,
                }),
                CartItemType::Tea => json!({
                    "tea_id": item.id,
                    "tea_name": item.name,
                    "quantity": item.quantity,
                    // A3: carry the drink's dine-in/takeaway choice (default
                    // takeaway if the customer never toggled it).
                    "fulfillment": item.fulfillment.clone().unwrap_or_else(|| "takeaway".to_string()),
                }),
                CartItemType::Set => json!({
                    "set_id": item.id,
                    "set_name": item.name,
                    "quantity": item.quantity,
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
                            // Navigate to success and clear cart
                            cart.write().clear();
                            nav.push(Route::Success { id: order_id });
                            return;
                        }
                    }
                    order_error.set(Some("Не удалось обработать ответ сервера".into()));
                }
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    // Cycle #65/#69 friendly per-status; cycle #70 sources
                    // `lang` from `?lang=xx` so a non-RU Telegram client gets
                    // localised "Account restricted" instead of Cyrillic.
                    order_error.set(Some(crate::trios::checkout_errors::friendly_order_error(
                        lang, status,
                    )));
                }
                Err(e) => {
                    order_error.set(Some(format!("Ошибка сети: {}", e)));
                }
            }
            is_processing.set(false);
        });
    };

    rsx! {
        div { style: "
            min-height: 100vh;
            background: #0f0f1a;
            color: #e8e8e8;
            padding-bottom: 80px;
        ",
            div { style: "padding: 20px 16px 16px; text-align: center;",
                h1 { style: "font-size: 24px; font-weight: 800; color: #39ff14; text-shadow: 3px 3px 0 #000, 0 0 10px rgba(57,255,20,0.5); letter-spacing: 2px;", "{checkout_title}" }
            }

            div { style: "padding: 0 16px;",
                // Age verification gate (Thailand cannabis compliance).
                {age_gate()}
                // Order summary from cart
                div { style: "
                    background: #16213e; border: 4px solid #2a2a4a;
                    border-radius: 0; padding: 14px; margin-bottom: 12px;
                    box-shadow: 4px 4px 0 #000;
                ",
                    h2 { style: "font-size: 13px; font-weight: 700; color: #00e5ff; text-transform: uppercase; letter-spacing: 1px; text-shadow: 2px 2px 0 #000; margin-bottom: 10px;", "{your_order}" }
                    if cart_items.is_empty() {
                        p { style: "font-size: 15px; color: #8b8b9e; text-align: center; padding: 10px;", "Cart is empty" }
                    } else {
                        for item in cart_items.iter() {
                            div { style: "display: flex; justify-content: space-between; margin-bottom: 6px; font-size: 13px;",
                                span { "{item.name}" }
                                span { style: "color: #8b8b9e;", "x{item.quantity}" }
                                {
                                    let line_str = crate::trios::pricing::format_baht(
                                        item.price * item.quantity as f64,
                                    );
                                    rsx! { span { "{line_str}" } }
                                }
                            }
                        }
                        // B4: garden discount picker — toggle a product-scoped reward.
                        if !applicable_rewards.is_empty() {
                            div { style: "border-top:1px solid #2a2a4a;margin-top:8px;padding-top:8px;",
                                div { style: "font-size:12px;color:#39ff14;font-weight:700;margin-bottom:6px;", "🌱 Скидка из сада" }
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
                                                span { style: "font-size:12px;color:#e8e8e8;", "{pct}% на {tname}" }
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
                                    div { style: "font-size:12px;color:#7dd3fc;font-weight:700;", "⭐ Stars" }
                                    div { style: "font-size:12px;color:#8b8b9e;", "доступно {stars_balance}" }
                                }
                                div { style: "display:flex;align-items:center;gap:8px;",
                                    input {
                                        r#type: "number",
                                        min: "0",
                                        max: "{max_stars}",
                                        value: "{stars_val}",
                                        style: "width:80px;font-size:14px;padding:6px 8px;background:#0f0f1a;color:#e8e8e8;border:3px solid #2a2a4a;",
                                        oninput: move |e| {
                                            let v = e.value().parse::<i64>().unwrap_or(0);
                                            stars_to_use.set(v.clamp(0, max_stars));
                                        }
                                    }
                                    span { style: "font-size:12px;color:#7dd3fc;", "−{stars_val} ฿" }
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
                        label { style: "font-size: 13px; color: #8b8b9e; display: block; margin-bottom: 4px;", "Name *" }
                        input {
                            style: "
                                font-size: 15px; width: 100%; padding: 10px 12px;
                                background: #0f0f1a; color: #e8e8e8;
                                border: 4px solid #2a2a4a; border-radius: 0;
                                box-sizing: border-box;
                            ",
                            r#type: "text",
                            placeholder: "Enter your name",
                            value: "{customer_name}",
                            oninput: move |e| customer_name.set(e.value()),
                        }
                    }
                    div { style: "margin-bottom: 8px;",
                        label { style: "font-size: 13px; color: #8b8b9e; display: block; margin-bottom: 4px;", "Phone *" }
                        input {
                            style: "
                                font-size: 15px; width: 100%; padding: 10px 12px;
                                background: #0f0f1a; color: #e8e8e8;
                                border: 4px solid #2a2a4a; border-radius: 0;
                                box-sizing: border-box;
                            ",
                            r#type: "tel",
                            placeholder: "+66 xxx xxx xxxx",
                            value: "{customer_phone}",
                            oninput: move |e| customer_phone.set(e.value()),
                        }
                    }
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
                        "📍 Открыть на карте"
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
                        label { style: "font-size: 13px; color: #8b8b9e; display: block; margin-bottom: 4px;", "Delivery address *" }
                        input {
                            style: "
                                font-size: 15px; width: 100%; padding: 10px 12px;
                                background: #0f0f1a; color: #e8e8e8;
                                border: 4px solid #2a2a4a; border-radius: 0;
                                box-sizing: border-box;
                            ",
                            r#type: "text",
                            placeholder: "Hotel / condo / street address",
                            value: "{delivery_address}",
                            oninput: move |e| delivery_address.set(e.value()),
                        }
                    }
                    div { style: "margin-bottom: 8px;",
                        label { style: "font-size: 13px; color: #8b8b9e; display: block; margin-bottom: 4px;", "Notes" }
                        input {
                            style: "
                                font-size: 15px; width: 100%; padding: 10px 12px;
                                background: #0f0f1a; color: #e8e8e8;
                                border: 4px solid #2a2a4a; border-radius: 0;
                                box-sizing: border-box;
                            ",
                            r#type: "text",
                            placeholder: "Room number, lobby, meet at gate…",
                            value: "{delivery_notes}",
                            oninput: move |e| delivery_notes.set(e.value()),
                        }
                    }
                    // Zone selector: drives ETA / fee display from backend config.
                    if !zones.is_empty() {
                        div { style: "margin-bottom: 8px;",
                            label { style: "font-size: 13px; color: #8b8b9e; display: block; margin-bottom: 4px;", "{delivery_zone_label}" }
                            for z in zones.iter() {
                                {
                                    let zid = z.id.clone();
                                    let zname = zone_display_name(z);
                                    let is_selected = selected_zone.as_ref().map(|s| s.id == zid).unwrap_or(false);
                                    let border = if is_selected { "#39ff14" } else { "#2a2a4a" };
                                    let bg = if is_selected { "rgba(57,255,20,0.08)" } else { "transparent" };
                                    rsx! {
                                        div {
                                            style: "
                                                background: {bg}; border: 3px solid {border};
                                                border-radius: 0; padding: 8px;
                                                margin-bottom: 6px; cursor: pointer;
                                            ",
                                            onclick: move |_| delivery_zone_id.set(Some(zid.clone())),
                                            div { style: "font-size: 14px;", "{zname}" }
                                        }
                                    }
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
                            div { style: "font-size: 15px; color: #39ff14;", "Cash on Delivery" }
                            div { style: "font-size: 13px; color: #8b8b9e; margin-top: 2px;", "Pay when you receive" }
                        }
                    }
                }

                // Error display
                if let Some(ref err) = order_error() {
                    div { style: "
                        background: rgba(255,71,87,0.1);
                        border: 4px solid rgba(255,71,87,0.4);
                        border-radius: 0; padding: 12px; margin-bottom: 12px;
                        color: #ff4757; font-size: 14px; text-align: center;
                    ", "❌ {err}" }
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
                            && age_ok
                            && validate_checkout(&customer_name(), &customer_phone(), &trios_items).is_ok()
                            && !is_processing();
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
                                onclick: submit_order,
                                if processing { "⏳ Оформление..." } else { "{place_order}" }
                            }
                        }
                    }
                }
            }
        }
    }
}
