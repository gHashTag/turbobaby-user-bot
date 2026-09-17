// Order detail screen — deep-link destination for order notifications.
//
// Shows the full order (items, totals, delivery info, status stepper) and is
// reachable both from the `/orders/:id` route and from taps on order cards in
// the orders list.

use crate::trios::i18n::{
    t, tf, T_CART_DELIVERY, T_CART_SUBTOTAL, T_MODAL_CANCEL, T_MODAL_CONFIRM, T_ORDERS_ORDER,
    T_ORDERS_STATUS_CANCELLED, T_ORDERS_STATUS_CONFIRMED, T_ORDERS_STATUS_DELIVERED,
    T_ORDERS_STATUS_OUT_FOR_DELIVERY, T_ORDERS_STATUS_PENDING, T_ORDERS_STATUS_PREPARING,
    T_ORDERS_STATUS_READY, T_ORDERS_STATUS_UNKNOWN, T_ORDERS_TITLE, T_ORDER_DETAIL_BACK,
    T_ORDER_DETAIL_BONUS, T_ORDER_DETAIL_CANCEL, T_ORDER_DETAIL_CANCEL_CONFIRM,
    T_ORDER_DETAIL_LIVE, T_ORDER_DETAIL_NOT_FOUND, T_ORDER_DETAIL_STARS, T_ORDER_DETAIL_TOTAL,
    T_ORDER_REORDER,
};
use crate::ui::api::context::api_base_url;
use crate::ui::api::http::{merge_server_cart, post_client_event};
use crate::ui::components::bottom_nav::BottomNav;
use crate::ui::components::skeleton::{Skeleton, SkeletonShape};
use crate::ui::components::StatusStepper;
use crate::ui::routes::Route;
use crate::ui::state::{Cart, CartItem, CartItemType};
use crate::ui::telegram::{
    use_telegram_id, use_telegram_init_data, HapticNotification, TelegramApp,
};
use dioxus::prelude::*;
use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Deserialize)]
struct ApiOrderDetail {
    id: String,
    items: Vec<ApiOrderItem>,
    subtotal: f64,
    bonus_used: f64,
    stars_used: i64,
    total: f64,
    status: String,
    created_at: String,
    shop_id: Option<String>,
    delivery_address: Option<String>,
    delivery_notes: Option<String>,
    customer_phone: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
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

#[derive(Debug, Deserialize)]
struct OrderDetailResponse {
    order: ApiOrderDetail,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[allow(dead_code)]
struct OrderStatusResp {
    status: String,
    #[serde(default)]
    delivery_zone_name: Option<String>,
    #[serde(default)]
    min_eta_minutes: Option<u32>,
    #[serde(default)]
    max_eta_minutes: Option<u32>,
}

/// Loop #12: convert a backend order item into a local [`CartItem`] so it can
/// be merged into the server-side cart with current DB prices.
fn api_order_item_to_cart_item(item: &ApiOrderItem) -> Option<CartItem> {
    let (id, name, item_type, price_hint) = if let Some(ref sid) = item.strain_id {
        (
            sid.clone(),
            // The *field* keeps its name — it is the wire contract for orders
            // already in the database, and renaming it would make historical
            // orders unreadable. The *label* does not: this fallback is what a
            // customer sees when an old item carries an id but no name, and
            // «Strain» is exactly the vocabulary #2 removes. "Товар" says the
            // same thing without naming the previous shop's goods.
            item.strain_name.clone().unwrap_or_else(|| "Товар".into()),
            CartItemType::Strain,
            item.unit_price.unwrap_or(0.0),
        )
    } else if let Some(ref set_id) = item.set_id {
        (
            set_id.clone(),
            item.set_name.clone().unwrap_or_else(|| "Set".into()),
            CartItemType::Set,
            item.unit_price.unwrap_or(0.0),
        )
    } else if let Some(ref aid) = item.accessory_id {
        (
            aid.clone(),
            item.accessory_name
                .clone()
                .unwrap_or_else(|| "Accessory".into()),
            CartItemType::Accessory,
            item.unit_price.unwrap_or(0.0),
        )
    } else {
        let tid = item.tea_id.as_ref()?;
        (
            tid.clone(),
            item.tea_name.clone().unwrap_or_else(|| "Drink".into()),
            CartItemType::Tea,
            item.unit_price.unwrap_or(0.0),
        )
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

fn item_name(item: &ApiOrderItem) -> String {
    item.strain_name
        .clone()
        .or_else(|| item.accessory_name.clone())
        .or_else(|| item.tea_name.clone())
        .or_else(|| item.set_name.clone())
        .unwrap_or_else(|| "Unknown".to_string())
}

fn is_terminal_status(status: &str) -> bool {
    matches!(status, "delivered" | "completed" | "rejected" | "cancelled")
}

fn status_color(status: &str) -> &'static str {
    match status.to_lowercase().as_str() {
        "pending" => "#ffe600",
        "confirmed" => "#00e5ff",
        "preparing" => "#ff9d00",
        "ready" => "#39ff14",
        "out_for_delivery" => "#00e5ff",
        "completed" | "delivered" => "#39ff14",
        "cancelled" | "rejected" => "#ff4757",
        _ => "#8b8b9e",
    }
}

fn status_label_key(status: &str) -> crate::trios::i18n::Key {
    match status.to_lowercase().as_str() {
        "pending" => T_ORDERS_STATUS_PENDING,
        "confirmed" => T_ORDERS_STATUS_CONFIRMED,
        "preparing" => T_ORDERS_STATUS_PREPARING,
        "ready" => T_ORDERS_STATUS_READY,
        "out_for_delivery" => T_ORDERS_STATUS_OUT_FOR_DELIVERY,
        "completed" | "delivered" => T_ORDERS_STATUS_DELIVERED,
        "cancelled" | "rejected" => T_ORDERS_STATUS_CANCELLED,
        _ => T_ORDERS_STATUS_UNKNOWN,
    }
}

/// Render the body of an order detail card. Extracted into its own component so
/// the screen-level resource read does not borrow across Dioxus event handlers
/// (which must be `'static`).
#[component]
fn OrderDetailCard(
    order: ApiOrderDetail,
    telegram_id: i64,
    init_data: String,
    live_status_res: Resource<Option<OrderStatusResp>>,
    cart: Signal<Cart>,
    lang: crate::trios::core::Lang,
) -> Element {
    let nav = navigator();
    let short_id: String = order
        .id
        .chars()
        .rev()
        .take(6)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    let live_status = live_status_res
        .read()
        .as_ref()
        .and_then(|o| o.as_ref())
        .map(|s| s.status.clone());
    let display_status = live_status.as_deref().unwrap_or(&order.status);
    let status_color = status_color(display_status);
    let status_label = t(lang, status_label_key(display_status));
    let date_str = order
        .created_at
        .split('T')
        .next()
        .unwrap_or(&order.created_at)
        .to_string();
    let shop = order.shop_id.as_deref().unwrap_or("TurboBaby");
    let subtotal_str = crate::trios::pricing::format_baht(order.subtotal);
    let total_str = crate::trios::pricing::format_baht(order.total);
    let bonus_str = crate::trios::pricing::format_baht(order.bonus_used);
    let is_terminal = is_terminal_status(display_status);
    let is_pending = display_status == "pending";
    let order_for_reorder = order.clone();
    let reorder_nav = nav;
    let reorder_cart = cart;
    let init_data_for_reorder = init_data.clone();
    let init_data_for_cancel = init_data.clone();
    let mut show_cancel_confirm = use_signal(|| false);
    let mut cancelling = use_signal(|| false);
    let reorder_loading = use_signal(|| false);
    let reorder_label = t(lang, T_ORDER_REORDER);

    rsx! {
        div { style: "
            background: #16213e; border: 4px solid {status_color};
            border-radius: 0; padding: 16px;
            box-shadow: 4px 4px 0 #000;
        ",
            div { style: "display: flex; justify-content: space-between; align-items: center; margin-bottom: 12px;",
                span { style: "font-size: 16px; color: #e8e8e8; font-weight: 700;",
                    "{tf(lang, T_ORDERS_ORDER, std::slice::from_ref(&short_id))}"
                }
                span { style: "
                    font-size: 13px; padding: 4px 10px;
                    background: {status_color}22; color: {status_color};
                ", "{status_label}" }
            }

            StatusStepper { status: display_status.to_string(), margin_bottom: 16 }

            if live_status.is_some() {
                div { style: "display: flex; align-items: center; gap: 6px; font-size: 11px; color: #39ff14; margin-bottom: 10px;",
                    span { style: "width: 6px; height: 6px; border-radius: 50%; background: #39ff14; display: inline-block;" }
                    "{t(lang, T_ORDER_DETAIL_LIVE)}"
                }
            }

            div { style: "margin-bottom: 12px;",
                div { style: "font-size: 12px; color: #8b8b9e; margin-bottom: 6px;",
                    "{tf(lang, crate::trios::i18n::T_CART_ITEMS, &[order.items.len().to_string()])}"
                }
                for item in order.items.iter() {
                    div { style: "display: flex; justify-content: space-between; font-size: 14px; margin-bottom: 4px;",
                        span { style: "color: #e8e8e8;", "{item_name(item)} x{item.quantity as i32}" }
                        span { style: "color: #8b8b9e;",
                            "{item.unit_price.map(|p| crate::trios::pricing::format_baht(p * item.quantity.max(0.0))).unwrap_or_default()}"
                        }
                    }
                }
            }

            div { style: "border-top: 1px dashed #2a2a4a; padding-top: 12px; margin-bottom: 12px;",
                div { style: "display: flex; justify-content: space-between; font-size: 14px; margin-bottom: 4px;",
                    span { style: "color: #8b8b9e;", "{t(lang, T_CART_SUBTOTAL)}" }
                    span { style: "color: #e8e8e8;", "{subtotal_str}" }
                }
                if order.bonus_used > 0.0 {
                    div { style: "display: flex; justify-content: space-between; font-size: 14px; margin-bottom: 4px;",
                        span { style: "color: #8b8b9e;", "{t(lang, T_ORDER_DETAIL_BONUS)}" }
                        span { style: "color: #ff4757;", "-{bonus_str}" }
                    }
                }
                if order.stars_used > 0 {
                    div { style: "display: flex; justify-content: space-between; font-size: 14px; margin-bottom: 4px;",
                        span { style: "color: #8b8b9e;", "{t(lang, T_ORDER_DETAIL_STARS)}" }
                        span { style: "color: #ff4757;", "-{order.stars_used} ⭐" }
                    }
                }
                div { style: "display: flex; justify-content: space-between; font-size: 14px; margin-bottom: 4px;",
                    span { style: "color: #8b8b9e;", "{t(lang, T_CART_DELIVERY)}" }
                    span { style: "color: #e8e8e8;", "{shop}" }
                }
                div { style: "display: flex; justify-content: space-between; font-size: 18px; margin-top: 8px; padding-top: 8px; border-top: 1px solid #2a2a4a;",
                    span { style: "font-weight: 800; color: #e8e8e8;", "{t(lang, T_ORDER_DETAIL_TOTAL)}" }
                    span { style: "font-weight: 800; color: #ffe600; text-shadow: 2px 2px 0 #000;", "{total_str}" }
                }
            }

            div { style: "border-top: 1px dashed #2a2a4a; padding-top: 12px; margin-bottom: 12px; font-size: 13px; color: #8b8b9e;",
                if let Some(addr) = &order.delivery_address {
                    div { style: "margin-bottom: 4px;", "📍 {addr.clone()}" }
                }
                if let Some(notes) = &order.delivery_notes {
                    div { style: "margin-bottom: 4px;", "📝 {notes.clone()}" }
                }
                if let Some(phone) = &order.customer_phone {
                    div { style: "margin-bottom: 4px;", "📞 {phone.clone()}" }
                }
                div { style: "margin-top: 4px;", "🕒 {date_str}" }
            }

            if is_pending {
                if show_cancel_confirm() {
                    div { style: "border: 3px solid #ff4757; background: #2a0a0a; padding: 12px; margin-bottom: 12px; box-shadow: 2px 2px 0 #000;",
                        div { style: "font-size: 13px; color: #e8e8e8; margin-bottom: 10px;", "{t(lang, T_ORDER_DETAIL_CANCEL_CONFIRM)}" }
                        div { style: "display: flex; gap: 8px;",
                            button {
                                style: "flex: 1; font-size: 13px; font-weight: 700; padding: 10px; background: #ff4757; color: #fff; border: 3px solid #b92b3a; cursor: pointer;",
                                disabled: cancelling(),
                                onclick: move |_| {
                                    let oid = order.id.clone();
                                    let init = init_data_for_cancel.clone();
                                    let tid = telegram_id;
                                    cancelling.set(true);
                                    spawn(async move {
                                        let base = api_base_url();
                                        let url = format!("{}/api/orders/{}/cancel?telegram_id={}", base, oid, tid);
                                        let _resp = crate::ui::api::local_client::LocalClient::new()
                                            .post(&url)
                                            .header("X-Telegram-Init-Data", init)
                                            .send()
                                            .await;
                                        cancelling.set(false);
                                        show_cancel_confirm.set(false);
                                        // Force a detail refresh by bumping the resource signal is hard;
                                        // instead the next poll tick will refresh status via live_status_res.
                                    });
                                },
                                "{t(lang, T_MODAL_CONFIRM)}"
                            }
                            button {
                                style: "flex: 1; font-size: 13px; font-weight: 700; padding: 10px; background: #2a2a4a; color: #e8e8e8; border: 3px solid #1a1a2e; cursor: pointer;",
                                onclick: move |_| { show_cancel_confirm.set(false); },
                                "{t(lang, T_MODAL_CANCEL)}"
                            }
                        }
                    }
                } else {
                    button {
                        style: "font-size:13px;font-weight:700;width:100%;padding:12px;background:#ff4757;color:#fff;border:3px solid #b92b3a;box-shadow:2px 2px 0 #000;cursor:pointer;margin-bottom:12px;",
                        onclick: move |_| { show_cancel_confirm.set(true); },
                        "{t(lang, T_ORDER_DETAIL_CANCEL)}"
                    }
                }
            }

            if is_terminal {
                button {
                    style: "font-size:14px;font-weight:700;width:100%;padding:12px;background:#39ff14;color:#000;border:3px solid #2d9e0f;box-shadow:2px 2px 0 #000;cursor:pointer;",
                    disabled: reorder_loading(),
                    onclick: move |_| {
                        let order_items = order_for_reorder.items.clone();
                        let init = init_data_for_reorder.clone();
                        let tid = telegram_id;
                        let mut cart_sig = reorder_cart;
                        let nav = reorder_nav;
                        let mut loading = reorder_loading;
                        spawn(async move {
                            loading.set(true);
                            let local_items: Vec<CartItem> = order_items
                                .iter()
                                .filter_map(api_order_item_to_cart_item)
                                .collect();
                            match merge_server_cart(&api_base_url(), &init, tid, &local_items,
                            ).await {
                                Ok(fresh_cart) => {
                                    cart_sig.set(fresh_cart);
                                    TelegramApp::init().haptic_notification(HapticNotification::Success);
                                    let _ = post_client_event(&api_base_url(), "reorder_clicked", "order_detail").await;
                                    nav.push(Route::Cart {});
                                }
                                Err(_) => {
                                    // Fallback: load items with the stale price hint rather than leaving
                                    // the user without a reorder path.
                                    cart_sig.write().clear();
                                    for item in local_items {
                                        cart_sig.write().add_item(item);
                                    }
                                    TelegramApp::init().haptic_notification(HapticNotification::Success);
                                    let _ = post_client_event(&api_base_url(), "reorder_clicked", "order_detail").await;
                                    nav.push(Route::Cart {});
                                }
                            }
                            loading.set(false);
                        });
                    },
                    "{reorder_label}"
                }
            }
        }
    }
}

#[component]
pub fn OrderDetailScreen(id: String) -> Element {
    let telegram_id = use_telegram_id().unwrap_or(0);
    let init_data = use_telegram_init_data();
    let init_data_for_detail = init_data.clone();
    let order_id = id.clone();

    let detail_resource = use_resource(move || {
        let init = init_data_for_detail.clone();
        let oid = order_id.clone();
        async move {
            if telegram_id == 0 {
                return Err("No telegram_id".to_string());
            }
            let base = api_base_url();
            let url = format!(
                "{}/api/orders/{}/details?telegram_id={}",
                base, oid, telegram_id
            );
            crate::ui::api::local_client::LocalClient::new()
                .get(&url)
                .header("X-Telegram-Init-Data", init)
                .send()
                .await
                .map_err(|e| e.to_string())?
                .json::<OrderDetailResponse>()
                .await
                .map(|r| r.order)
                .map_err(|e| e.to_string())
        }
    });

    // Cycle #90: live status polling so the detail screen stays current after
    // a Telegram push notification deep-links the user here.
    let mut poll_tick = use_signal(|| 0u32);
    use_effect(move || {
        spawn(async move {
            for tick in 0..360u32 {
                gloo_timers::future::sleep(core::time::Duration::from_millis(10000)).await;
                poll_tick.set(tick + 1);
            }
        });
    });
    let live_status_res = {
        let id = id.clone();
        let init = init_data.clone();
        use_resource(move || {
            let id = id.clone();
            let init = init.clone();
            let _ = poll_tick();
            async move {
                if telegram_id == 0 {
                    return None;
                }
                let base = api_base_url();
                let url = format!(
                    "{}/api/orders/{}/status?telegram_id={}",
                    base, id, telegram_id
                );
                crate::ui::api::local_client::LocalClient::new()
                    .get(&url)
                    .header("X-Telegram-Init-Data", init)
                    .send()
                    .await
                    .ok()?
                    .json::<OrderStatusResp>()
                    .await
                    .ok()
            }
        })
    };

    let nav = navigator();
    let lang = crate::ui::lang::current_lang();
    let cart = use_context::<Signal<Cart>>();
    let title = t(lang, T_ORDERS_TITLE);
    let back_label = t(lang, T_ORDER_DETAIL_BACK);

    rsx! {
        div { style: "
            min-height: 100vh;
            background: #0f0f1a;
            color: #e8e8e8;
            padding-bottom: calc(96px + env(safe-area-inset-bottom));
        ",
            div { style: "padding: 16px; display: flex; align-items: center; gap: 12px;",
                button {
                    style: "font-size: 13px; padding: 8px 12px; background: #2a2a4a; color: #e8e8e8; border: 3px solid #1a1a2e; box-shadow: 2px 2px 0 #000; cursor: pointer;",
                    onclick: move |_| { nav.push(Route::Orders {}); },
                    "{back_label}"
                }
                h1 { style: "font-size: 18px; font-weight: 800; color: #39ff14; text-shadow: 2px 2px 0 #000;", "{title}" }
            }

            div { style: "padding: 0 16px;",
                {
                    let detail_opt = detail_resource.read().clone();
                    match detail_opt {
                        Some(Ok(order)) => rsx! {
                            OrderDetailCard {
                                order,
                                telegram_id,
                                init_data: init_data.clone(),
                                live_status_res,
                                cart,
                                lang,
                            }
                        },
                        Some(Err(e)) => rsx! {
                            div { style: "text-align: center; padding: 40px 16px;",
                                div { style: "font-size: 70px; margin-bottom: 12px;", "⚠️" }
                                p { style: "font-size: 13px; color: #ff4757;", "{t(lang, T_ORDER_DETAIL_NOT_FOUND)}" }
                                p { style: "font-size: 12px; color: #8b8b9e; margin-top: 8px;", "{e}" }
                            }
                        },
                        None => rsx! {
                            div { style: "display:flex;flex-direction:column;gap:12px;",
                                Skeleton { shape: SkeletonShape::Orders }
                                Skeleton { shape: SkeletonShape::Orders }
                            }
                        },
                    }
                }
            }

            BottomNav {}
        }
    }
}
