use crate::trios::i18n::{
    t, tf, T_ORDERS_BROWSE_BIKES, T_ORDERS_FILTER_ACTIVE, T_ORDERS_FILTER_ALL,
    T_ORDERS_FILTER_CANCELLED, T_ORDERS_FILTER_COMPLETED, T_ORDERS_HISTORY, T_ORDERS_NO_ORDERS,
    T_ORDERS_ORDER, T_ORDERS_STATUS_CANCELLED, T_ORDERS_STATUS_CONFIRMED,
    T_ORDERS_STATUS_DELIVERED, T_ORDERS_STATUS_OUT_FOR_DELIVERY, T_ORDERS_STATUS_PENDING,
    T_ORDERS_STATUS_PREPARING, T_ORDERS_STATUS_READY, T_ORDERS_STATUS_UNKNOWN, T_ORDERS_TITLE,
    T_REORDER,
};
use crate::ui::api::context::api_base_url;
use crate::ui::api::http::merge_server_cart;
use crate::ui::components::bottom_nav::BottomNav;
use crate::ui::components::open_in_telegram::OpenInTelegramNotice;
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
struct ApiOrder {
    id: String,
    items: Vec<ApiOrderItem>,
    total: f64,
    status: String,
    created_at: String,
    shop_id: Option<String>,
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

fn item_name(item: &ApiOrderItem) -> String {
    item.strain_name
        .clone()
        .or_else(|| item.accessory_name.clone())
        .or_else(|| item.tea_name.clone())
        .or_else(|| item.set_name.clone())
        .unwrap_or_else(|| "Unknown".to_string())
}

/// Loop #12: convert a backend order item into a local [`CartItem`] so it can
/// be merged into the server-side cart with current DB prices.
fn api_order_item_to_cart_item(item: &ApiOrderItem) -> Option<CartItem> {
    let (id, name, item_type, price_hint) = if let Some(ref sid) = item.strain_id {
        (
            sid.clone(),
            item.strain_name.clone().unwrap_or_else(|| "Strain".into()),
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

#[derive(Debug, Deserialize)]
struct OrdersResponse {
    orders: Vec<ApiOrder>,
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

#[derive(Debug, Clone, Copy, PartialEq)]
enum StatusFilter {
    All,
    Active,
    Completed,
    Cancelled,
}

impl StatusFilter {
    fn label(&self, lang: crate::trios::core::Lang) -> String {
        let key = match self {
            Self::All => T_ORDERS_FILTER_ALL,
            Self::Active => T_ORDERS_FILTER_ACTIVE,
            Self::Completed => T_ORDERS_FILTER_COMPLETED,
            Self::Cancelled => T_ORDERS_FILTER_CANCELLED,
        };
        t(lang, key).to_string()
    }
    fn matches(&self, status: &str) -> bool {
        match self {
            Self::All => true,
            Self::Active => !is_terminal_status(status),
            Self::Completed => status == "completed" || status == "delivered",
            Self::Cancelled => status == "cancelled" || status == "rejected",
        }
    }
}

fn is_terminal_status(status: &str) -> bool {
    matches!(status, "delivered" | "completed" | "rejected" | "cancelled")
}

// Здесь жили ReviewFormProps и ReviewForm — модальная форма отзыва на сорт.
// Единственный вызов, client.create_review, бил в /api/reviews поверх
// strain_reviews, удалённой миграцией 083.

#[component]
pub fn OrdersScreen() -> Element {
    let mut active_filter = use_signal(|| StatusFilter::All);
    let reorder_loading = use_signal(|| false);
    let telegram_id = use_telegram_id().unwrap_or(0);
    let init_data = use_telegram_init_data();
    let init_data_for_orders = init_data.clone();

    let orders_resource = use_resource(move || {
        let init = init_data_for_orders.clone();
        async move {
            if telegram_id == 0 {
                return Err("No telegram_id".to_string());
            }
            let base = api_base_url();
            let url = format!("{}/api/orders/user/{}", base, telegram_id);
            crate::ui::api::local_client::LocalClient::new()
                .get(&url)
                .header("X-Telegram-Init-Data", init)
                .send()
                .await
                .map_err(|e| e.to_string())?
                .json::<OrdersResponse>()
                .await
                .map(|r| r.orders)
                .map_err(|e| e.to_string())
        }
    });

    let cart = use_context::<Signal<Cart>>();
    let nav = navigator();
    let reorder_label = t(crate::ui::lang::current_lang(), T_REORDER);

    let filtered = match &*orders_resource.read() {
        Some(Ok(orders)) => {
            let f = active_filter();
            orders
                .iter()
                .filter(|o| f.matches(&o.status))
                .cloned()
                .collect::<Vec<_>>()
        }
        _ => Vec::new(),
    };

    let lang = crate::ui::lang::current_lang();
    let orders_title = t(lang, T_ORDERS_TITLE);

    rsx! {
        div { style: "
            min-height: 100vh;
            background: #0f0f1a;
            color: #e8e8e8;
            padding-bottom: calc(96px + env(safe-area-inset-bottom));
        ",
            div { style: "padding: 20px 16px 16px; text-align: center;",
                h1 { style: "font-size: 24px; font-weight: 800; color: #39ff14; text-shadow: 3px 3px 0 #000, 0 0 10px rgba(57,255,20,0.5); letter-spacing: 2px;", "{orders_title}" }
                p { style: "font-size: 15px; color: #8b8b9e; margin-top: 4px;", "{t(lang, T_ORDERS_HISTORY)}" }
            }

            // Status filters (pill chips)
            div { style: "display: flex; gap: 6px; padding: 0 16px 12px; overflow-x: auto;",
                for filter in [StatusFilter::All, StatusFilter::Active, StatusFilter::Completed, StatusFilter::Cancelled] {
                    {
                        let is_active = active_filter() == filter;
                        let bg = if is_active { "#39ff14" } else { "transparent" };
                        let color = if is_active { "#000" } else { "#8b8b9e" };
                        let border = if is_active { "#39ff14" } else { "#2a2a4a" };
                        let label = filter.label(lang);
                        rsx! {
                            button {
                                style: "
                                    font-size: 13px; padding: 6px 10px;
                                    background: {bg}; color: {color};
                                    border: 4px solid {border}; border-radius: 20px;
                                    cursor: pointer; white-space: nowrap;
                                ",
                                onclick: move |_| active_filter.set(filter),
                                "{label}"
                            }
                        }
                    }
                }
            }

            div { style: "padding: 0 16px;",
                {
                    match &*orders_resource.read() {
                        Some(Ok(orders)) if orders.is_empty() => rsx! {
                            div { style: "text-align: center; padding: 40px 16px;",
                                div { style: "font-size: 70px; margin-bottom: 12px;", "📦" }
                                p { style: "font-size: 13px; color: #8b8b9e; margin-bottom: 16px;", "{t(lang, T_ORDERS_NO_ORDERS)}" }
                                // Пустой список заказов — первое, что видит новый
                                // арендатор. Кнопка вела в `Route::Sets` (наборы
                                // каннабиса, снятые с витрины миграцией 085);
                                // теперь она ведёт в каталог байков.
                                Link { to: Route::Menu {},
                                    button { style: "
                                        font-size: 14px; font-weight: 700; padding: 12px 20px;
                                        background: #39ff14; color: #000;
                                        border: 4px solid #2d9e0f; border-radius: 0; cursor: pointer;
                                        box-shadow: 3px 3px 0 #000;
                                    ", "{t(lang, T_ORDERS_BROWSE_BIKES)}" }
                                }
                            }
                        },
                        Some(Ok(_)) => rsx! {
                            div { style: "display: flex; flex-direction: column; gap: 10px;",
                                for order in filtered.iter() {
                                    {
                                        let o = order.clone();
                                        let short_id: String = o.id.chars().rev().take(6).collect::<Vec<_>>().into_iter().rev().collect();
                                        let status_color = status_color(&o.status);
                                        let status_label = t(lang, status_label_key(&o.status));
                                        let date_str = o.created_at.split('T').next().unwrap_or(&o.created_at).to_string();
                                        let shop = o.shop_id.as_deref().unwrap_or("TurboBaby");
                                        let total_str = crate::trios::pricing::format_baht(o.total);
                                        let is_cancelled = o.status == "cancelled" || o.status == "rejected";
                                        let opacity = if is_cancelled { "0.7" } else { "1" };
                                        let border_color = if is_cancelled { "#2a2a4a" } else { status_color };
                                        let is_active = !is_terminal_status(&o.status);
                                        let shadow = if is_active { "4px 4px 0 #000, 0 0 12px rgba(0,229,255,0.1)" } else { "4px 4px 0 #000" };
                                        let order_nav = nav;
                                        let order_id_for_card = o.id.clone();

                                        rsx! {
                                            div { style: "
                                                background: #16213e; border: 4px solid {border_color};
                                                border-radius: 0; padding: 14px;
                                                box-shadow: {shadow};
                                                opacity: {opacity};
                                                cursor: pointer;
                                            ",
                                                onclick: move |_| { order_nav.push(Route::OrderDetail { id: order_id_for_card.clone() }); },
                                                StatusStepper { status: o.status.clone() }
                                                div { style: "display: flex; justify-content: space-between; align-items: center; margin-bottom: 8px;",
                                                    span { style: "font-size: 15px; color: #e8e8e8;", "{tf(lang, T_ORDERS_ORDER, std::slice::from_ref(&short_id))}" }
                                                    span { style: "
                                                        font-size: 13px; padding: 3px 8px;
                                                        border-radius: 0;
                                                        background: {status_color}22; color: {status_color};
                                                    ", "{status_label}" }
                                                }
                                                div { style: "margin-bottom: 8px;",
                                                    for item in o.items.iter() {
                                                        div { style: "display: flex; justify-content: space-between; font-size: 13px; margin-bottom: 3px;",
                                                            span { style: "color: #8b8b9e;", "{item_name(item)} x{item.quantity as i32}" }
                                                        }
                                                    }
                                                }
                                                // Здесь стояла панель «Оставить отзыв»: она показывалась под каждым
                                                // доставленным заказом, у которого есть позиция со strain_id, то есть
                                                // под историческими заказами каннабиса. Кнопка открывала форму, форма
                                                // слала POST /api/reviews, маршрут читал strain_reviews — таблицу,
                                                // удалённую миграцией 083. Ответ был 500 всегда.
                                                //
                                                // Хуже самого 500: форма писала `let _ = client.create_review(&req).await;`
                                                // и сразу ставила done = true. Клиент видел «🙌 Спасибо за отзыв!»
                                                // на отзыв, который сервер отказался принять и нигде не сохранил.
                                                // Соврать об успехе дороже, чем показать ошибку.
                                                div { style: "display: flex; justify-content: space-between; padding-top: 8px; border-top: 1px solid #2a2a4a; font-size: 13px;",
                                                    span { style: "color: #8b8b9e;", "📍 {shop} · {date_str}" }
                                                    span { style: "font-size: 20px; font-weight: 800; color: #ffe600; text-shadow: 2px 2px 0 #000;", "{total_str}" }
                                                }
                                                if is_terminal_status(&o.status) {
                                                    {
                                                        let order_for_reorder = o.clone();
                                                        let reorder_nav = nav;
                                                        let reorder_cart = cart;
                                                        let reorder_label2 = reorder_label;
                                                        let loading = reorder_loading;
                                                        let init = init_data.clone();
                                                        let tid = telegram_id;
                                                        rsx! {
                                                            div { style: "padding-top: 8px;",
                                                                button {
                                                                    style: "font-size:13px;font-weight:700;width:100%;padding:10px;background:#39ff14;color:#000;border:3px solid #2d9e0f;box-shadow:2px 2px 0 #000;cursor:pointer;",
                                                                    disabled: loading(),
                                                                    onclick: move |e: Event<MouseData>| {
                                                                        e.stop_propagation();
                                                                        let order_items = order_for_reorder.items.clone();
                                                                        let mut cart_sig = reorder_cart;
                                                                        let nav = reorder_nav;
                                                                        let mut loading_inner = loading;
                                                                        let init = init.clone();
                                                                        spawn(async move {
                                                                            loading_inner.set(true);
                                                                            let local_items: Vec<CartItem> = order_items
                                                                                .iter()
                                                                                .filter_map(api_order_item_to_cart_item)
                                                                                .collect();
                                                                            match merge_server_cart(
                                                                                &api_base_url(), &init, tid, &local_items,
                                                                            ).await {
                                                                                Ok(fresh_cart) => {
                                                                                    cart_sig.set(fresh_cart);
                                                                                    TelegramApp::init().haptic_notification(HapticNotification::Success);
                                                                                    nav.push(Route::Cart {});
                                                                                }
                                                                                Err(_) => {
                                                                                    cart_sig.write().clear();
                                                                                    for item in local_items {
                                                                                        cart_sig.write().add_item(item);
                                                                                    }
                                                                                    TelegramApp::init().haptic_notification(HapticNotification::Success);
                                                                                    nav.push(Route::Cart {});
                                                                                }
                                                                            }
                                                                            loading_inner.set(false);
                                                                        });
                                                                    },
                                                                    "{reorder_label2}"
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        },
                        Some(Err(e)) => rsx! {
                            // Outside Telegram there is no identity to list orders
                            // for; that is a supported state, not an error.
                            if telegram_id == 0 {
                                OpenInTelegramNotice {}
                            } else {
                                div { style: "text-align: center; padding: 40px 16px;",
                                    div { style: "font-size: 70px; margin-bottom: 12px;", "⚠️" }
                                    p { style: "font-size: 13px; color: #ff4757;", "Error: {e}" }
                                }
                            }
                        },
                        None => rsx! {
                            div { style: "display:flex;flex-direction:column;gap:12px;",
                                Skeleton { shape: SkeletonShape::Orders }
                                Skeleton { shape: SkeletonShape::Orders }
                                Skeleton { shape: SkeletonShape::Orders }
                            }
                        },
                    }
                }
            }

            // Здесь рисовалась ReviewForm. Удалена вместе с эндпоинтом.

            BottomNav {}
        }
    }
}
