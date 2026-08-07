// Order detail screen — deep-link destination for order notifications.
//
// Shows the full order (items, totals, delivery info, status stepper) and is
// reachable both from the `/orders/:id` route and from taps on order cards in
// the orders list.

use crate::trios::i18n::{
    t, tf, T_CART_DELIVERY, T_CART_SUBTOTAL, T_ORDERS_ORDER, T_ORDERS_STATUS_CANCELLED,
    T_ORDERS_STATUS_CONFIRMED, T_ORDERS_STATUS_DELIVERED, T_ORDERS_STATUS_OUT_FOR_DELIVERY,
    T_ORDERS_STATUS_PENDING, T_ORDERS_STATUS_PREPARING, T_ORDERS_STATUS_READY,
    T_ORDERS_STATUS_UNKNOWN, T_ORDERS_TITLE, T_ORDER_DETAIL_BACK, T_ORDER_DETAIL_BONUS,
    T_ORDER_DETAIL_NOT_FOUND, T_ORDER_DETAIL_STARS, T_ORDER_DETAIL_TOTAL, T_REORDER,
};
use crate::ui::api::context::api_base_url;
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

#[derive(Debug, Clone, Deserialize)]
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

#[derive(Debug, Clone, Deserialize)]
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

#[component]
pub fn OrderDetailScreen(id: String) -> Element {
    let telegram_id = use_telegram_id().unwrap_or(0);
    let init_data = use_telegram_init_data();
    let order_id = id.clone();

    let detail_resource = use_resource(move || {
        let init = init_data.clone();
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

    let nav = navigator();
    let lang = crate::ui::lang::current_lang();
    let cart = use_context::<Signal<Cart>>();
    let reorder_label = t(lang, T_REORDER);
    let title = t(lang, T_ORDERS_TITLE);
    let back_label = t(lang, T_ORDER_DETAIL_BACK);

    rsx! {
        div { style: "
            min-height: 100vh;
            background: #0f0f1a;
            color: #e8e8e8;
            padding-bottom: 80px;
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
                    match &*detail_resource.read() {
                        Some(Ok(order)) => {
                            let short_id: String = order.id.chars().rev().take(6).collect::<Vec<_>>().into_iter().rev().collect();
                            let status_color = status_color(&order.status);
                            let status_label = t(lang, status_label_key(&order.status));
                            let date_str = order.created_at.split('T').next().unwrap_or(&order.created_at).to_string();
                            let shop = order.shop_id.as_deref().unwrap_or("Woody Shop");
                            let subtotal_str = crate::trios::pricing::format_baht(order.subtotal);
                            let total_str = crate::trios::pricing::format_baht(order.total);
                            let bonus_str = crate::trios::pricing::format_baht(order.bonus_used);
                            let is_terminal = is_terminal_status(&order.status);
                            let order_for_reorder = order.clone();
                            let reorder_nav = nav.clone();
                            let mut reorder_cart = cart.clone();

                            rsx! {
                                div { style: "
                                    background: #16213e; border: 4px solid {status_color};
                                    border-radius: 0; padding: 16px;
                                    box-shadow: 4px 4px 0 #000;
                                ",
                                    div { style: "display: flex; justify-content: space-between; align-items: center; margin-bottom: 12px;",
                                        span { style: "font-size: 16px; color: #e8e8e8; font-weight: 700;",
                                            "{tf(lang, T_ORDERS_ORDER, &[short_id])}"
                                        }
                                        span { style: "
                                            font-size: 13px; padding: 4px 10px;
                                            background: {status_color}22; color: {status_color};
                                        ", "{status_label}" }
                                    }

                                    StatusStepper { status: order.status.clone(), margin_bottom: 16 }

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

                                    if is_terminal {
                                        button {
                                            style: "font-size:14px;font-weight:700;width:100%;padding:12px;background:#39ff14;color:#000;border:3px solid #2d9e0f;box-shadow:2px 2px 0 #000;cursor:pointer;",
                                            onclick: move |_| {
                                                reorder_cart.write().clear();
                                                for item in order_for_reorder.items.iter() {
                                                    let (id, name, item_type, price_hint) = if let Some(ref sid) = item.strain_id {
                                                        (sid.clone(), item.strain_name.clone().unwrap_or_else(|| "Strain".into()), CartItemType::Strain, item.unit_price.unwrap_or(0.0))
                                                    } else if let Some(ref set_id) = item.set_id {
                                                        (set_id.clone(), item.set_name.clone().unwrap_or_else(|| "Set".into()), CartItemType::Set, item.unit_price.unwrap_or(0.0))
                                                    } else if let Some(ref aid) = item.accessory_id {
                                                        (aid.clone(), item.accessory_name.clone().unwrap_or_else(|| "Accessory".into()), CartItemType::Accessory, item.unit_price.unwrap_or(0.0))
                                                    } else if let Some(ref tid) = item.tea_id {
                                                        (tid.clone(), item.tea_name.clone().unwrap_or_else(|| "Drink".into()), CartItemType::Tea, item.unit_price.unwrap_or(0.0))
                                                    } else {
                                                        continue;
                                                    };
                                                    let qty = item.quantity.max(1.0) as u32;
                                                    reorder_cart.write().add_item(CartItem {
                                                        id,
                                                        name,
                                                        price: price_hint,
                                                        quantity: qty,
                                                        image_url: None,
                                                        item_type,
                                                        fulfillment: None,
                                                    });
                                                }
                                                TelegramApp::init().haptic_notification(HapticNotification::Success);
                                                reorder_nav.push(Route::Cart {});
                                            },
                                            "{reorder_label}"
                                        }
                                    }
                                }
                            }
                        }
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
