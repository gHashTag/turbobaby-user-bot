use dioxus::prelude::*;
use serde::Deserialize;
use crate::ui::routes::Route;
use crate::ui::api::context::api_base_url;
use crate::trios::core::Lang;
use crate::trios::i18n::{t, T_ORDERS_TITLE, T_LOADING};
use crate::ui::components::bottom_nav::BottomNav;
use crate::ui::telegram::{use_telegram_id, use_telegram_init_data};

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
    #[allow(dead_code)]
    strain_id: Option<String>,
    strain_name: Option<String>,
    #[allow(dead_code)]
    accessory_id: Option<String>,
    accessory_name: Option<String>,
    #[allow(dead_code)]
    tea_id: Option<String>,
    tea_name: Option<String>,
    #[allow(dead_code)]
    set_id: Option<String>,
    set_name: Option<String>,
    quantity: f64,
}

fn item_name(item: &ApiOrderItem) -> String {
    item.strain_name.clone()
        .or_else(|| item.accessory_name.clone())
        .or_else(|| item.tea_name.clone())
        .or_else(|| item.set_name.clone())
        .unwrap_or_else(|| "Unknown".to_string())
}

#[derive(Debug, Deserialize)]
struct OrdersResponse {
    orders: Vec<ApiOrder>,
}

fn status_style(status: &str) -> (&'static str, &'static str) {
    match status.to_lowercase().as_str() {
        "pending" => ("#ffe600", "⏳ Pending"),
        "confirmed" => ("#00e5ff", "✅ Confirmed"),
        "ready" => ("#39ff14", "📦 Ready"),
        "completed" | "delivered" => ("#39ff14", "✅ Delivered"),
        "processing" => ("#00e5ff", "🔄 In Progress"),
        "cancelled" => ("#ff4757", "❌ Cancelled"),
        _ => ("#8b8b9e", "📋 Unknown"),
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum StatusFilter {
    All,
    Pending,
    Confirmed,
    Ready,
    Completed,
}

impl StatusFilter {
    fn label(&self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Pending => "⏳ Pending",
            Self::Confirmed => "✅ Confirmed",
            Self::Ready => "📦 Ready",
            Self::Completed => "✅ Completed",
        }
    }
    fn matches(&self, status: &str) -> bool {
        match self {
            Self::All => true,
            Self::Pending => status == "pending",
            Self::Confirmed => status == "confirmed",
            Self::Ready => status == "ready",
            Self::Completed => status == "completed" || status == "delivered",
        }
    }
}

#[component]
pub fn OrdersScreen() -> Element {
    let mut active_filter = use_signal(|| StatusFilter::All);
    let telegram_id = use_telegram_id().unwrap_or(0);
    let init_data = use_telegram_init_data();

    let orders_resource = use_resource(move || {
        let init = init_data.clone();
        async move {
            if telegram_id == 0 {
                return Err("No telegram_id".to_string());
            }
            let base = api_base_url();
            let url = format!("{}/api/orders/user/{}", base, telegram_id);
            reqwest::Client::new()
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

    let filtered = match &*orders_resource.read() {
        Some(Ok(orders)) => {
            let f = active_filter();
            orders.iter()
                .filter(|o| f.matches(&o.status))
                .cloned()
                .collect::<Vec<_>>()
        }
        _ => Vec::new(),
    };

    let orders_title = t(Lang::Russian, T_ORDERS_TITLE);
    let loading_text = t(Lang::Russian, T_LOADING);


    rsx! {
        div { style: "
            min-height: 100vh;
            background: #0f0f1a;
            color: #e8e8e8;
            padding-bottom: 80px;
        ",
            div { style: "padding: 20px 16px 16px; text-align: center;",
                h1 { style: "font-size: 24px; font-weight: 800; color: #39ff14; text-shadow: 3px 3px 0 #000, 0 0 10px rgba(57,255,20,0.5); letter-spacing: 2px;", "{orders_title}" }
                p { style: "font-size: 15px; color: #8b8b9e; margin-top: 4px;", "Your order history" }
            }

            // Status filters (pill chips)
            div { style: "display: flex; gap: 6px; padding: 0 16px 12px; overflow-x: auto;",
                for filter in [StatusFilter::All, StatusFilter::Pending, StatusFilter::Confirmed, StatusFilter::Ready, StatusFilter::Completed] {
                    {
                        let is_active = active_filter() == filter;
                        let bg = if is_active { "#39ff14" } else { "transparent" };
                        let color = if is_active { "#000" } else { "#8b8b9e" };
                        let border = if is_active { "#39ff14" } else { "#2a2a4a" };
                        let label = filter.label();
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
                                p { style: "font-size: 13px; color: #8b8b9e; margin-bottom: 16px;", "No orders yet" }
                                Link { to: Route::Sets {},
                                    button { style: "
                                        font-size: 14px; font-weight: 700; padding: 12px 20px;
                                        background: #39ff14; color: #000;
                                        border: 4px solid #2d9e0f; border-radius: 0; cursor: pointer;
                                        box-shadow: 3px 3px 0 #000;
                                    ", "Browse Sets 🎁" }
                                }
                            }
                        },
                        Some(Ok(_)) => rsx! {
                            div { style: "display: flex; flex-direction: column; gap: 10px;",
                                for order in filtered.iter() {
                                    {
                                        let o = order.clone();
                                        let short_id: String = o.id.chars().rev().take(6).collect::<Vec<_>>().into_iter().rev().collect();
                                        let (status_color, status_label) = status_style(&o.status);
                                        let date_str = o.created_at.split('T').next().unwrap_or(&o.created_at).to_string();
                                        let shop = o.shop_id.as_deref().unwrap_or("Woody Shop");
                                        let total_str = format!("฿{}", o.total as i32);
                                        let is_cancelled = o.status == "cancelled";
                                        let opacity = if is_cancelled { "0.7" } else { "1" };
                                        let border_color = if is_cancelled { "#2a2a4a" } else { status_color };
                                        let is_active = o.status == "confirmed" || o.status == "processing";
                                        let shadow = if is_active { "4px 4px 0 #000, 0 0 12px rgba(0,229,255,0.1)" } else { "4px 4px 0 #000" };

                                        rsx! {
                                            div { style: "
                                                background: #16213e; border: 4px solid {border_color};
                                                border-radius: 0; padding: 14px;
                                                box-shadow: {shadow};
                                                opacity: {opacity};
                                            ",
                                                div { style: "display: flex; justify-content: space-between; align-items: center; margin-bottom: 8px;",
                                                    span { style: "font-size: 15px; color: #e8e8e8;", "Order #{short_id}" }
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
                                                div { style: "display: flex; justify-content: space-between; padding-top: 8px; border-top: 1px solid #2a2a4a; font-size: 13px;",
                                                    span { style: "color: #8b8b9e;", "📍 {shop} · {date_str}" }
                                                    span { style: "font-size: 20px; font-weight: 800; color: #ffe600; text-shadow: 2px 2px 0 #000;", "{total_str}" }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        },
                        Some(Err(e)) => rsx! {
                            div { style: "text-align: center; padding: 40px 16px;",
                                div { style: "font-size: 70px; margin-bottom: 12px;", "⚠️" }
                                p { style: "font-size: 13px; color: #ff4757;", "Error: {e}" }
                            }
                        },
                        None => rsx! {
                            div { style: "display: flex; flex-direction: column; gap: 10px;",
                                for _ in 0..3 {
                                    div { style: "
                                        background: #16213e; border: 4px solid #2a2a4a;
                                        border-radius: 0; padding: 20px;
                                        box-shadow: 4px 4px 0 #000;
                                    ",
                                        div { style: "font-size: 13px; color: #8b8b9e;", "{loading_text}" }
                                    }
                                }
                            }
                        },
                    }
                }
            }

            BottomNav {}
        }
    }
}
