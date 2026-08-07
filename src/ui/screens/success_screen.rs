use crate::trios::core::Lang;
use crate::trios::i18n::{
    t, tf, T_ORDERS_STATUS_CANCELLED, T_ORDERS_STATUS_CONFIRMED, T_ORDERS_STATUS_DELIVERED,
    T_ORDERS_STATUS_OUT_FOR_DELIVERY, T_ORDERS_STATUS_PENDING, T_ORDERS_STATUS_PREPARING,
    T_ORDERS_STATUS_READY, T_ORDERS_STATUS_UNKNOWN, T_SUCCESS_BACK_MENU,
    T_SUCCESS_CASH_ON_DELIVERY, T_SUCCESS_CONFIRMED, T_SUCCESS_CONTACT_SHORTLY,
    T_SUCCESS_DELIVERY_ESTIMATE, T_SUCCESS_ETA, T_SUCCESS_ETA_VALUE, T_SUCCESS_MY_ORDERS,
    T_SUCCESS_ORDER_RECEIVED, T_SUCCESS_PAYMENT, T_SUCCESS_STATUS, T_SUCCESS_TITLE,
};
use crate::ui::api::context::api_base_url;
use crate::ui::api::http::fetch_text_authed;
use crate::ui::routes::Route;
use crate::ui::telegram::{use_telegram_id, use_telegram_init_data, TelegramApp};
use dioxus::prelude::*;

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

#[component]
pub fn SuccessScreen(id: String) -> Element {
    let lang = crate::ui::lang::current_lang();
    let title = t(lang, T_SUCCESS_TITLE).to_string();
    let received = tf(lang, T_SUCCESS_ORDER_RECEIVED, &[id.clone()]);
    let contact = t(lang, T_SUCCESS_CONTACT_SHORTLY).to_string();
    let delivery_estimate = t(lang, T_SUCCESS_DELIVERY_ESTIMATE).to_string();
    let status_label = t(lang, T_SUCCESS_STATUS).to_string();
    let eta_label = t(lang, T_SUCCESS_ETA).to_string();
    let payment_label = t(lang, T_SUCCESS_PAYMENT).to_string();
    let cash_on_delivery = t(lang, T_SUCCESS_CASH_ON_DELIVERY).to_string();
    let back_menu = t(lang, T_SUCCESS_BACK_MENU).to_string();
    let my_orders = t(lang, T_SUCCESS_MY_ORDERS).to_string();

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
    let status_res = {
        let id = id.clone();
        let init = init_data.clone();
        use_resource(move || {
            let id = id.clone();
            let init = init.clone();
            let _ = poll_tick();
            async move {
                let tid = telegram_id?;
                let url = format!(
                    "{}/api/orders/{id}/status?telegram_id={tid}",
                    api_base_url()
                );
                let text = fetch_text_authed(&url, &init).await.ok()?;
                serde_json::from_str::<OrderStatusResp>(&text).ok()
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

    let status_ref = status_res.read();
    let status_opt = status_ref.as_ref().and_then(|opt| opt.as_ref());
    let status_text = status_opt
        .map(|s| order_status_label(lang, &s.status))
        .unwrap_or_else(|| t(lang, T_SUCCESS_CONFIRMED).to_string());
    let eta_range = status_opt
        .and_then(|s| match (s.min_eta_minutes, s.max_eta_minutes) {
            (Some(min), Some(max)) => Some(format!("{min}-{max}")),
            _ => None,
        })
        .or_else(|| zone_eta().as_ref().map(Clone::clone))
        .unwrap_or_else(|| "30-45".to_string());
    let zone_display = status_opt
        .and_then(|s| s.delivery_zone_name.clone())
        .or_else(|| zone_name().as_ref().map(Clone::clone));

    // Hide native Telegram chrome on this terminal screen; all navigation is
    // handled by the two large in-app CTAs.
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
                if let Some(name) = zone_display.as_deref() {
                    div { style: "font-size: 12px; color: #8b8b9e; margin-bottom: 8px;", "{name}" }
                }
                div { style: "display: flex; justify-content: space-between; margin-bottom: 6px; font-size: 13px;",
                    span { style: "color: #8b8b9e;", "{status_label}" }
                    span { style: "color: #39ff14;", "{status_text}" }
                }
                div { style: "display: flex; justify-content: space-between; margin-bottom: 6px; font-size: 13px;",
                    span { style: "color: #8b8b9e;", "{eta_label}" }
                    span { "{tf(lang, T_SUCCESS_ETA_VALUE, &[eta_range.clone()])}" }
                }
                div { style: "display: flex; justify-content: space-between; font-size: 13px;",
                    span { style: "color: #8b8b9e;", "{payment_label}" }
                    span { "{cash_on_delivery}" }
                }
            }

            // Actions
            div { style: "display: flex; gap: 10px; width: 100%; max-width: 320px;",
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
        }
    }
}
