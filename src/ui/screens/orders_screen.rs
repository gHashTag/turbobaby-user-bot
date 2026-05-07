use dioxus::prelude::*;
use serde::Deserialize;
use crate::ui::routes::Route;
use crate::ui::api::context::api_base_url;
use crate::trios::core::Lang;
use crate::trios::i18n::{t, T_ORDERS_TITLE, T_LOADING};
use crate::ui::components::bottom_nav::BottomNav;

#[derive(Debug, Clone, Deserialize)]
struct ApiOrder {
    id: String,
    items: Vec<ApiOrderItem>,
    total: f64,
    status: String,
    created_at: String,
    shop_name: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct ApiOrderItem {
    name: String,
    quantity: u32,
    price: f64,
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

    let orders_resource = use_resource(|| async move {
        let base = api_base_url();
        let url = format!("{}/api/orders?limit=20", base);
        reqwest::Client::new()
            .get(&url)
            .send()
            .await
            .map_err(|e| e.to_string())?
            .json::<OrdersResponse>()
            .await
            .map(|r| r.orders)
            .map_err(|e| e.to_string())
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
            font-family: 'Press Start 2P', monospace;
            padding-bottom: 80px;
        ",
            div { style: "padding: 20px 16px 12px; text-align: center;",
                h1 { style: "font-size: 18px; color: #39ff14; text-shadow: 0 0 8px rgba(57,255,20,0.5);", "{orders_title}" }
                p { style: "font-size: 18px; color: #8b8b9e; margin-top: 4px;", "Your order history" }
            }

            // Status filters
            div { style: "display: flex; gap: 6px; padding: 0 16px 12px; overflow-x: auto;",
                for filter in [StatusFilter::All, StatusFilter::Pending, StatusFilter::Confirmed, StatusFilter::Ready, StatusFilter::Completed] {
                    {
                        let is_active = active_filter() == filter;
                        let bg = if is_active { "#39ff14" } else { "transparent" };
                        let color = if is_active { "#0f0f1a" } else { "#8b8b9e" };
                        let border = if is_active { "#39ff14" } else { "#2a2a4a" };
                        let label = filter.label();
                        rsx! {
                            button {
                                style: "
                                    font-family: 'Press Start 2P', monospace;
                                    font-size: 10px; padding: 6px 10px;
                                    background: {bg}; color: {color};
                                    border: 2px solid {border}; border-radius: 8px;
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
                                div { style: "font-size: 36px; margin-bottom: 12px;", "📦" }
                                p { style: "font-size: 12px; color: #8b8b9e; margin-bottom: 16px;", "No orders yet" }
                                Link { to: Route::Sets {},
                                    button { style: "
                                        font-family: 'Press Start 2P', monospace;
                                        font-size: 10px; padding: 10px 20px;
                                        background: #39ff14; color: #0f0f1a;
                                        border: none; border-radius: 6px; cursor: pointer;
                                    ", "Browse Sets 🎁" }
                                }
                            }
                        },
                        Some(Ok(_)) => rsx! {
                            div { style: "display: flex; flex-direction: column; gap: 10px;",
                                for order in filtered.iter() {
                                    {
                                        let o = order.clone();
                                        let short_id = if o.id.len() > 6 { o.id[o.id.len()-6..].to_string() } else { o.id.clone() };
                                        let (status_color, status_label) = status_style(&o.status);
                                        let date_str = o.created_at.split('T').next().unwrap_or(&o.created_at).to_string();
                                        let shop = o.shop_name.as_deref().unwrap_or("Woody Shop");
                                        let total_str = format!("฿{}", o.total as i32);
                                        let is_cancelled = o.status == "cancelled";
                                        let opacity = if is_cancelled { "0.7" } else { "1" };
                                        let border_color = if is_cancelled { "#2a2a4a" } else { status_color };
                                        let is_active = o.status == "confirmed" || o.status == "processing";
                                        let shadow = if is_active { "4px 4px 0 #000, 0 0 12px rgba(0,229,255,0.1)" } else { "4px 4px 0 #000" };

                                        rsx! {
                                            div { style: "
                                                background: #1a1a2e; border: 2px solid {border_color};
                                                border-radius: 8px; padding: 14px;
                                                box-shadow: {shadow};
                                                opacity: {opacity};
                                            ",
                                                div { style: "display: flex; justify-content: space-between; align-items: center; margin-bottom: 8px;",
                                                    span { style: "font-size: 12px; color: #e8e8e8;", "Order #{short_id}" }
                                                    span { style: "
                                                        font-size: 10px; padding: 3px 8px;
                                                        border-radius: 8px;
                                                        background: {status_color}22; color: {status_color};
                                                    ", "{status_label}" }
                                                }
                                                div { style: "margin-bottom: 8px;",
                                                    for item in o.items.iter() {
                                                        div { style: "display: flex; justify-content: space-between; font-size: 10px; margin-bottom: 3px;",
                                                            span { style: "color: #8b8b9e;", "{item.name} x{item.quantity}" }
                                                            span { "฿{(item.price * item.quantity as f64) as i32}" }
                                                        }
                                                    }
                                                }
                                                div { style: "display: flex; justify-content: space-between; padding-top: 8px; border-top: 1px solid #2a2a4a; font-size: 10px;",
                                                    span { style: "color: #8b8b9e;", "📍 {shop} · {date_str}" }
                                                    span { style: "color: {status_color}; font-weight: bold;", "{total_str}" }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        },
                        Some(Err(e)) => rsx! {
                            div { style: "text-align: center; padding: 40px 16px;",
                                div { style: "font-size: 36px; margin-bottom: 12px;", "⚠️" }
                                p { style: "font-size: 12px; color: #ff4757;", "Error: {e}" }
                            }
                        },
                        None => rsx! {
                            div { style: "display: flex; flex-direction: column; gap: 10px;",
                                for _ in 0..3 {
                                    div { style: "
                                        background: #1a1a2e; border: 2px solid #2a2a4a;
                                        border-radius: 8px; padding: 20px;
                                        box-shadow: 4px 4px 0 #000;
                                    ",
                                        div { style: "font-size: 12px; color: #8b8b9e;", "{loading_text}" }
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
