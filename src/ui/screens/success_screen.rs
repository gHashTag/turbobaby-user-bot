use crate::trios::i18n::{
    t, tf,
    T_SUCCESS_BACK_MENU, T_SUCCESS_CASH_ON_DELIVERY, T_SUCCESS_CONFIRMED,
    T_SUCCESS_CONTACT_SHORTLY, T_SUCCESS_DELIVERY_ESTIMATE, T_SUCCESS_ETA,
    T_SUCCESS_ETA_VALUE, T_SUCCESS_MY_ORDERS, T_SUCCESS_ORDER_RECEIVED,
    T_SUCCESS_PAYMENT, T_SUCCESS_STATUS, T_SUCCESS_TITLE,
};
use crate::ui::routes::Route;
use crate::ui::telegram::TelegramApp;
use dioxus::prelude::*;

#[component]
pub fn SuccessScreen(id: String) -> Element {
    let lang = crate::ui::lang::current_lang();
    let title = t(lang, T_SUCCESS_TITLE).to_string();
    let received = tf(lang, T_SUCCESS_ORDER_RECEIVED, &[id.clone()]);
    let contact = t(lang, T_SUCCESS_CONTACT_SHORTLY).to_string();
    let delivery_estimate = t(lang, T_SUCCESS_DELIVERY_ESTIMATE).to_string();
    let status_label = t(lang, T_SUCCESS_STATUS).to_string();
    let confirmed = t(lang, T_SUCCESS_CONFIRMED).to_string();
    let eta_label = t(lang, T_SUCCESS_ETA).to_string();
    let payment_label = t(lang, T_SUCCESS_PAYMENT).to_string();
    let cash_on_delivery = t(lang, T_SUCCESS_CASH_ON_DELIVERY).to_string();
    let back_menu = t(lang, T_SUCCESS_BACK_MENU).to_string();
    let my_orders = t(lang, T_SUCCESS_MY_ORDERS).to_string();

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
                if let Some(name) = zone_name().as_deref() {
                    div { style: "font-size: 12px; color: #8b8b9e; margin-bottom: 8px;", "{name}" }
                }
                div { style: "display: flex; justify-content: space-between; margin-bottom: 6px; font-size: 13px;",
                    span { style: "color: #8b8b9e;", "{status_label}" }
                    span { style: "color: #39ff14;", "{confirmed}" }
                }
                div { style: "display: flex; justify-content: space-between; margin-bottom: 6px; font-size: 13px;",
                    span { style: "color: #8b8b9e;", "{eta_label}" }
                    span { "{zone_eta().as_deref().map(|eta| tf(lang, T_SUCCESS_ETA_VALUE, &[eta.to_string()])).unwrap_or_else(|| \"30-45 min\".to_string())}" }
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
