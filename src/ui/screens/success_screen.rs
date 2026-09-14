use crate::trios::core::Lang;
use crate::trios::i18n::{
    t, tf, T_ORDERS_STATUS_CANCELLED, T_ORDERS_STATUS_CONFIRMED, T_ORDERS_STATUS_DELIVERED,
    T_ORDERS_STATUS_OUT_FOR_DELIVERY, T_ORDERS_STATUS_PENDING, T_ORDERS_STATUS_PREPARING,
    T_ORDERS_STATUS_READY, T_ORDERS_STATUS_UNKNOWN, T_SUCCESS_BACK_MENU, T_SUCCESS_CASHBACK_EARNED,
    T_SUCCESS_CASHBACK_ERROR, T_SUCCESS_CASH_ON_DELIVERY, T_SUCCESS_CONFIRMED,
    T_SUCCESS_CONTACT_SHORTLY, T_SUCCESS_DELIVERY_ESTIMATE, T_SUCCESS_ETA, T_SUCCESS_ETA_VALUE,
    T_SUCCESS_MY_ORDERS, T_SUCCESS_ORDER_RECEIVED, T_SUCCESS_PAYMENT, T_SUCCESS_PUSH_REASSURANCE,
    T_SUCCESS_REORDER, T_SUCCESS_RETRY, T_SUCCESS_REWARDS_BONUS, T_SUCCESS_REWARDS_GARDEN,
    T_SUCCESS_REWARDS_TITLE, T_SUCCESS_SHARE_REFERRAL, T_SUCCESS_STATUS, T_SUCCESS_STATUS_ERROR,
    T_SUCCESS_STATUS_LOADING, T_SUCCESS_TITLE, T_SUCCESS_TRACK_ORDER,
};
use crate::ui::api::context::api_base_url;
use crate::ui::api::http::{fetch_text_authed, merge_server_cart};
use crate::ui::routes::Route;
use crate::ui::share::{open_telegram_link, order_deep_link};
use crate::ui::state::{Cart, CartItem};
use crate::ui::telegram::{
    use_telegram_id, use_telegram_init_data, HapticNotification, TelegramApp,
};
use dioxus::prelude::*;

/// Backend metrics are unavailable in the WASM build; this wrapper no-ops there
/// and delegates to `crate::metrics` when the backend feature is compiled.
#[cfg(target_arch = "wasm32")]
fn track_event(_name: &str, _detail: &str) {}

#[cfg(all(not(target_arch = "wasm32"), feature = "backend"))]
fn track_event(name: &str, detail: &str) {
    match name {
        "order_tracked" => crate::metrics::order_tracked(),
        "referral_prompt_clicked" => crate::metrics::referral_prompt_clicked(detail),
        "reorder_clicked" => crate::metrics::reorder_clicked(detail),
        _ => {}
    }
}

fn order_status_label(lang: Lang, status: &str) -> String {
    let key = match status {
        "pending" => T_ORDERS_STATUS_PENDING,
        "confirmed" => T_ORDERS_STATUS_CONFIRMED,
        "preparing" => T_ORDERS_STATUS_PREPARING,
        "ready" => T_ORDERS_STATUS_READY,
        "out_for_delivery" => T_ORDERS_STATUS_OUT_FOR_DELIVERY,
        "delivered" | "completed" => T_ORDERS_STATUS_DELIVERED,
        "cancelled" | "rejected" => T_ORDERS_STATUS_CANCELLED,
        _ => T_ORDERS_STATUS_UNKNOWN,
    };
    t(lang, key).to_string()
}

/// Loop #13: convert an order detail item into a local cart item so the
/// success screen can offer one-tap reorder with current DB prices.
fn api_order_item_to_cart_item(item: &ApiOrderItem) -> Option<CartItem> {
    let (id, name, item_type, price_hint) = if let Some(ref sid) = item.strain_id {
        (
            sid.clone(),
            item.strain_name.clone().unwrap_or_else(|| "Strain".into()),
            crate::ui::state::CartItemType::Strain,
            item.unit_price.unwrap_or(0.0),
        )
    } else if let Some(ref set_id) = item.set_id {
        (
            set_id.clone(),
            item.set_name.clone().unwrap_or_else(|| "Set".into()),
            crate::ui::state::CartItemType::Set,
            item.unit_price.unwrap_or(0.0),
        )
    } else if let Some(ref aid) = item.accessory_id {
        (
            aid.clone(),
            item.accessory_name
                .clone()
                .unwrap_or_else(|| "Accessory".into()),
            crate::ui::state::CartItemType::Accessory,
            item.unit_price.unwrap_or(0.0),
        )
    } else if let Some(ref tid) = item.tea_id {
        (
            tid.clone(),
            item.tea_name.clone().unwrap_or_else(|| "Drink".into()),
            crate::ui::state::CartItemType::Tea,
            item.unit_price.unwrap_or(0.0),
        )
    } else {
        return None;
    };
    Some(CartItem {
        id,
        name,
        price: price_hint,
        quantity: item.quantity.max(1.0) as u32,
        image_url: None,
        item_type,
        fulfillment: None,
    })
}

#[derive(Clone, serde::Deserialize)]
struct OrderStatusResp {
    status: String,
    #[serde(default)]
    delivery_zone_name: Option<String>,
    #[serde(default)]
    min_eta_minutes: Option<u32>,
    #[serde(default)]
    max_eta_minutes: Option<u32>,
}

#[derive(Clone, serde::Deserialize)]
struct LoyaltyProfileWrapper {
    profile: LoyaltyProfile,
}

#[derive(Clone, serde::Deserialize)]
struct LoyaltyProfile {
    bonus_balance: f64,
}

#[derive(Clone, serde::Deserialize)]
struct ApiOrderItem {
    strain_id: Option<String>,
    strain_name: Option<String>,
    accessory_id: Option<String>,
    accessory_name: Option<String>,
    tea_id: Option<String>,
    tea_name: Option<String>,
    set_id: Option<String>,
    set_name: Option<String>,
    quantity: f64,
    #[serde(default)]
    unit_price: Option<f64>,
}

#[derive(Clone, serde::Deserialize)]
struct OrderDetailResponse {
    order: OrderDetailOrder,
}

#[derive(Clone, serde::Deserialize)]
struct OrderDetailOrder {
    items: Vec<ApiOrderItem>,
    #[serde(default)]
    cashback_credited: Option<f64>,
}

#[component]
pub fn SuccessScreen(id: String) -> Element {
    let lang = crate::ui::lang::current_lang();
    let title = t(lang, T_SUCCESS_TITLE).to_string();
    let received = tf(lang, T_SUCCESS_ORDER_RECEIVED, std::slice::from_ref(&id));
    let contact = t(lang, T_SUCCESS_CONTACT_SHORTLY).to_string();
    let delivery_estimate = t(lang, T_SUCCESS_DELIVERY_ESTIMATE).to_string();
    let status_label = t(lang, T_SUCCESS_STATUS).to_string();
    let eta_label = t(lang, T_SUCCESS_ETA).to_string();
    let payment_label = t(lang, T_SUCCESS_PAYMENT).to_string();
    let cash_on_delivery = t(lang, T_SUCCESS_CASH_ON_DELIVERY).to_string();
    let back_menu = t(lang, T_SUCCESS_BACK_MENU).to_string();
    let my_orders = t(lang, T_SUCCESS_MY_ORDERS).to_string();
    let track_order = t(lang, T_SUCCESS_TRACK_ORDER).to_string();
    let rewards_title = t(lang, T_SUCCESS_REWARDS_TITLE).to_string();
    let rewards_garden = t(lang, T_SUCCESS_REWARDS_GARDEN).to_string();
    let share_referral = t(lang, T_SUCCESS_SHARE_REFERRAL).to_string();
    let push_reassurance = t(lang, T_SUCCESS_PUSH_REASSURANCE).to_string();
    let reorder = t(lang, T_SUCCESS_REORDER).to_string();

    let telegram_id = use_telegram_id();
    let init_data = use_telegram_init_data();

    // Poll the server for live order status + ETA. Stop after 60 seconds so
    // we don't keep pinging a closed order forever.
    let mut poll_tick = use_signal(|| 0u32);
    use_effect(move || {
        spawn(async move {
            for tick in 0..12u32 {
                poll_tick.set(tick + 1);
                gloo_timers::future::sleep(core::time::Duration::from_millis(5000)).await;
            }
        });
    });
    let mut status_retry = use_signal(|| 0u32);
    let status_res = {
        let id = id.clone();
        let init = init_data.clone();
        use_resource(move || {
            let id = id.clone();
            let init = init.clone();
            let _ = poll_tick();
            let _ = status_retry();
            async move {
                let tid = telegram_id.ok_or_else(|| {
                    crate::trios::api_errors::friendly_response_error(
                        crate::ui::lang::current_lang(),
                        0,
                    )
                })?;
                let url = format!(
                    "{}/api/orders/{id}/status?telegram_id={tid}",
                    api_base_url()
                );
                match fetch_text_authed(&url, &init).await {
                    Ok(text) => serde_json::from_str::<OrderStatusResp>(&text)
                        .map_err(|e| format!("parse: {e}")),
                    Err(e) => Err(e),
                }
            }
        })
    };

    // Load the delivery zone written by checkout so the ETA is real, not a
    // hard-coded "30-45 min" fallback.
    let mut zone_name = use_signal(|| Option::<String>::None);
    let mut zone_eta = use_signal(|| Option::<String>::None);
    use_effect(move || {
        if let Some(window) = web_sys::window() {
            if let Ok(Some(storage)) = window.local_storage() {
                if let Ok(Some(name)) = storage.get_item("woody_last_zone_name") {
                    zone_name.set(Some(name));
                }
                if let Ok(Some(eta)) = storage.get_item("woody_last_zone_eta") {
                    zone_eta.set(Some(eta));
                }
            }
        }
    });

    let profile_res = {
        let init = init_data.clone();
        use_resource(move || {
            let init = init.clone();
            async move {
                let tid = telegram_id?;
                let url = format!("{}/api/loyalty/{tid}", api_base_url());
                let text = fetch_text_authed(&url, &init).await.ok()?;
                serde_json::from_str::<LoyaltyProfileWrapper>(&text).ok()
            }
        })
    };

    // Loop #14: fetch order details to show the cashback earned on this order.
    let mut details_retry = use_signal(|| 0u32);
    let details_res = {
        let init = init_data.clone();
        let oid = id.clone();
        use_resource(move || {
            let init = init.clone();
            let oid = oid.clone();
            let _ = details_retry();
            async move {
                let tid = telegram_id.ok_or_else(|| {
                    crate::trios::api_errors::friendly_response_error(
                        crate::ui::lang::current_lang(),
                        0,
                    )
                })?;
                let url = format!(
                    "{}/api/orders/{oid}/details?telegram_id={tid}",
                    api_base_url()
                );
                match fetch_text_authed(&url, &init).await {
                    Ok(text) => serde_json::from_str::<OrderDetailResponse>(&text)
                        .map_err(|e| format!("parse: {e}")),
                    Err(e) => Err(e),
                }
            }
        })
    };

    let status_result = status_res
        .read()
        .as_ref()
        .and_then(|r| r.as_ref().ok().cloned());
    let status_error = status_res
        .read()
        .as_ref()
        .and_then(|r| r.as_ref().err().cloned());
    let status_text = status_result
        .as_ref()
        .map(|s| order_status_label(lang, &s.status))
        .unwrap_or_else(|| t(lang, T_SUCCESS_CONFIRMED).to_string());
    let eta_range = status_result
        .as_ref()
        .and_then(|s| match (s.min_eta_minutes, s.max_eta_minutes) {
            (Some(min), Some(max)) => Some(format!("{min}-{max}")),
            _ => None,
        })
        .or_else(|| zone_eta().clone())
        .unwrap_or_else(|| "30-45".to_string());
    let zone_display = status_result
        .as_ref()
        .and_then(|s| s.delivery_zone_name.clone())
        .or_else(|| zone_name().clone());
    let status_loading = status_res.read().is_none();

    let bonus_balance = profile_res
        .read()
        .as_ref()
        .and_then(|opt| opt.as_ref())
        .map(|w| w.profile.bonus_balance)
        .unwrap_or(0.0);
    let bonus_text = tf(
        lang,
        T_SUCCESS_REWARDS_BONUS,
        &[format!("{bonus_balance:.0}")],
    );
    let details_result = details_res
        .read()
        .as_ref()
        .and_then(|r| r.as_ref().ok().cloned());
    let details_error = details_res
        .read()
        .as_ref()
        .and_then(|r| r.as_ref().err().cloned());
    let details_loading = details_res.read().is_none();
    let cashback_earned = details_result
        .as_ref()
        .and_then(|r| r.order.cashback_credited)
        .filter(|v| v.is_finite() && *v > 0.01);
    let cashback_text =
        cashback_earned.map(|c| tf(lang, T_SUCCESS_CASHBACK_EARNED, &[format!("{c:.0}")]));

    let track_link = order_deep_link(&id);
    let on_track_order = move |_| {
        track_event("order_tracked", "");
        let link = track_link.clone();
        spawn(async move {
            // Ask Telegram for write-access permission so the bot can send
            // status-milestone pushes. The deep-link is opened either way.
            TelegramApp::init().request_write_access().await;
            open_telegram_link(&link);
        });
    };

    let nav = use_navigator();
    let on_share_referral = move |_| {
        track_event("referral_prompt_clicked", "success_screen");
        nav.push(Route::Referrals {});
    };

    // Loop #13: one-tap reorder from the success screen. Fetch the order
    // details, merge its items into the server-side cart with current prices,
    // then navigate to the cart.
    let reorder_id = id.clone();
    let reorder_init = init_data.clone();
    let reorder_tid = telegram_id;
    let cart_for_reorder = use_context::<Signal<Cart>>();
    let reorder_nav = nav;
    let on_reorder = move |_| {
        track_event("reorder_clicked", "success_screen");
        let oid = reorder_id.clone();
        let init = reorder_init.clone();
        let tid = reorder_tid.unwrap_or(0);
        let mut cart_sig = cart_for_reorder;
        let nav = reorder_nav;
        spawn(async move {
            if tid == 0 {
                return;
            }
            let url = format!(
                "{}/api/orders/{oid}/details?telegram_id={tid}",
                api_base_url()
            );
            if let Ok(text) = fetch_text_authed(&url, &init).await {
                if let Ok(resp) = serde_json::from_str::<OrderDetailResponse>(&text) {
                    let items: Vec<CartItem> = resp
                        .order
                        .items
                        .iter()
                        .filter_map(api_order_item_to_cart_item)
                        .collect();
                    match merge_server_cart(&api_base_url(), &init, tid, &items).await {
                        Ok(fresh_cart) => {
                            cart_sig.set(fresh_cart);
                        }
                        Err(_) => {
                            cart_sig.write().clear();
                            for item in items {
                                cart_sig.write().add_item(item);
                            }
                        }
                    }
                    TelegramApp::init().haptic_notification(HapticNotification::Success);
                    nav.push(Route::Cart {});
                }
            }
        });
    };

    // Hide native Telegram chrome on this terminal screen; all navigation is
    // handled by the large in-app CTAs.
    let tg = TelegramApp::init();
    tg.hide_main_button();
    tg.hide_back_button();

    rsx! {
        div { style: "
            min-height: 100vh;
            background: #0f0f1a;
            color: #e8e8e8;
            display: flex;
            flex-direction: column;
            align-items: center;
            justify-content: center;
            padding: 24px;
            text-align: center;
        ",
            // Success icon
            div { style: "
                font-size: 70px;
                margin-bottom: 20px;
                animation: pulse 1s ease-in-out infinite alternate;
            ", "✅" }

            // Title
            h1 { style: "
                font-size: 24px;
                font-weight: 800;
                color: #39ff14;
                text-shadow: 3px 3px 0 #000, 0 0 10px rgba(57,255,20,0.5);
                letter-spacing: 2px;
                margin-bottom: 12px;
            ", "{title}" }

            // Order ID
            p { style: "
                font-size: 15px;
                color: #8b8b9e;
                margin-bottom: 6px;
            ", "{received}" }

            p { style: "
                font-size: 13px;
                color: #8b8b9e;
                margin-bottom: 32px;
            ", "{contact}" }

            // Delivery estimate card
            div { style: "
                background: #16213e;
                border: 4px solid #2a2a4a;
                border-radius: 0;
                padding: 16px;
                width: 100%;
                max-width: 320px;
                margin-bottom: 24px;
                box-shadow: 4px 4px 0 #000;
            ",
                div { style: "font-size: 13px; font-weight: 700; color: #00e5ff; text-transform: uppercase; letter-spacing: 1px; text-shadow: 2px 2px 0 #000; margin-bottom: 4px;", "{delivery_estimate}" }
                if status_loading {
                    div { style: "font-size: 12px; color: #8b8b9e; margin-bottom: 8px;", "{t(lang, T_SUCCESS_STATUS_LOADING)}" }
                } else if let Some(ref _err) = status_error {
                    div { style: "font-size: 12px; color: #ff4757; margin-bottom: 6px;",
                        "{t(lang, T_SUCCESS_STATUS_ERROR)}"
                    }
                    button {
                        style: "font-size: 12px; font-weight: 700; padding: 4px 10px; background: #16213e; color: #00e5ff; border: 3px solid #2a2a4a; cursor: pointer;",
                        onclick: move |_| { status_retry.set(status_retry() + 1); },
                        "{t(lang, T_SUCCESS_RETRY)}"
                    }
                }
                if let Some(name) = zone_display.as_deref() {
                    div { style: "font-size: 12px; color: #8b8b9e; margin-bottom: 8px;", "{name}" }
                }
                div { style: "display: flex; justify-content: space-between; margin-bottom: 6px; font-size: 13px;",
                    span { style: "color: #8b8b9e;", "{status_label}" }
                    span { style: "color: #39ff14;", "{status_text}" }
                }
                div { style: "display: flex; justify-content: space-between; margin-bottom: 6px; font-size: 13px;",
                    span { style: "color: #8b8b9e;", "{eta_label}" }
                    span { "{tf(lang, T_SUCCESS_ETA_VALUE, std::slice::from_ref(&eta_range))}" }
                }
                div { style: "display: flex; justify-content: space-between; font-size: 13px;",
                    span { style: "color: #8b8b9e;", "{payment_label}" }
                    span { "{cash_on_delivery}" }
                }
            }

            // Track Order CTA opens the order deep-link so the customer gets
            // live Telegram push updates for every status milestone.
            button {
                style: "
                    font-size: 14px; font-weight: 700; width: 100%; max-width: 320px;
                    padding: 12px 20px; margin-bottom: 12px;
                    background: #00e5ff; color: #000;
                    border: 4px solid #008db1; border-radius: 0;
                    cursor: pointer; box-shadow: 3px 3px 0 #000;
                    transition: transform 0.1s, box-shadow 0.1s;
                ",
                onclick: on_track_order,
                "{track_order}"
            }

            // Push reassurance: remind the customer that tracking the order
            // in Telegram turns on a push for every status milestone.
            div { style: "
                font-size: 12px; color: #8b8b9e;
                width: 100%; max-width: 320px;
                margin-bottom: 16px; text-align: center;
            ", "{push_reassurance}" }

            // Actions
            div { style: "display: flex; gap: 10px; width: 100%; max-width: 320px; margin-bottom: 16px;",
                Link { to: Route::Menu {},
                    button { style: "
                        font-size: 14px; font-weight: 700; flex: 1; padding: 12px 20px;
                        background: #39ff14; color: #000;
                        border: 4px solid #2d9e0f; border-radius: 0;
                        cursor: pointer; box-shadow: 3px 3px 0 #000;
                        transition: transform 0.1s, box-shadow 0.1s;
                    ", "{back_menu}" }
                }
                Link { to: Route::Orders {},
                    button { style: "
                        font-size: 14px; font-weight: 700; flex: 1; padding: 12px 20px;
                        background: transparent; color: #e8e8e8;
                        border: 4px solid #2a2a4a; border-radius: 0;
                        cursor: pointer; box-shadow: 3px 3px 0 #000;
                        transition: transform 0.1s, box-shadow 0.1s;
                    ", "{my_orders}" }
                }
            }

            // Rewards card: surface garden seed progress and current bonus
            // balance right after checkout to reinforce retention value.
            div { style: "
                background: #16213e;
                border: 4px solid #2a2a4a;
                border-radius: 0;
                padding: 14px 16px;
                width: 100%;
                max-width: 320px;
                margin-bottom: 12px;
                box-shadow: 4px 4px 0 #000;
                text-align: left;
            ",
                div { style: "font-size: 13px; font-weight: 700; color: #39ff14; text-transform: uppercase; letter-spacing: 1px; text-shadow: 2px 2px 0 #000; margin-bottom: 8px;", "{rewards_title}" }
                div { style: "font-size: 13px; color: #e8e8e8; margin-bottom: 6px;", "{rewards_garden}" }
                div { style: "font-size: 13px; color: #e8e8e8; margin-bottom: 6px;", "{bonus_text}" }
                if details_loading {
                    div { style: "font-size: 12px; color: #8b8b9e;", "{t(lang, T_SUCCESS_STATUS_LOADING)}" }
                } else if let Some(ref _err) = details_error {
                    div { style: "display:flex; gap:8px; align-items:center;",
                        span { style: "font-size: 12px; color: #ff4757;", "{t(lang, T_SUCCESS_CASHBACK_ERROR)}" }
                        button {
                            style: "font-size: 12px; font-weight: 700; padding: 4px 10px; background: #16213e; color: #00e5ff; border: 3px solid #2a2a4a; cursor: pointer;",
                            onclick: move |_| { details_retry.set(details_retry() + 1); },
                            "{t(lang, T_SUCCESS_RETRY)}"
                        }
                    }
                } else if let Some(ref text) = cashback_text {
                    div { style: "font-size: 14px; font-weight: 800; color: #39ff14;", "{text}" }
                }
            }

            // Share / referral prompt.
            button {
                style: "
                    font-size: 13px; font-weight: 700; width: 100%; max-width: 320px;
                    padding: 10px 16px;
                    background: transparent; color: #00e5ff;
                    border: 3px solid #2a2a4a; border-radius: 0;
                    cursor: pointer; box-shadow: 3px 3px 0 #000;
                    transition: transform 0.1s, box-shadow 0.1s;
                ",
                onclick: on_share_referral,
                "{share_referral}"
            }

            // Reorder CTA: one-tap repeat of the just-placed order, merged with
            // current DB prices via the server cart endpoint.
            button {
                style: "
                    font-size: 14px; font-weight: 700; width: 100%; max-width: 320px;
                    padding: 12px 20px; margin-bottom: 16px;
                    background: #39ff14; color: #000;
                    border: 4px solid #2d9e0f; border-radius: 0;
                    cursor: pointer; box-shadow: 3px 3px 0 #000;
                    transition: transform 0.1s, box-shadow 0.1s;
                ",
                onclick: on_reorder,
                "{reorder}"
            }
        }
    }
}
