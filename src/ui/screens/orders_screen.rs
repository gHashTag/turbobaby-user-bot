use crate::trios::i18n::{
    t, T_LOADING, T_ORDERS_TITLE, T_REVIEW_COMMENT, T_REVIEW_LEAVE, T_REVIEW_RATING,
    T_REVIEW_SUBMIT, T_REVIEW_THANKS,
};
use crate::ui::api::context::{api_base_url, use_api_client};
use crate::ui::components::bottom_nav::BottomNav;
use crate::ui::routes::Route;
use crate::ui::telegram::{use_telegram_id, use_telegram_init_data};
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
    accessory_name: Option<String>,
    tea_name: Option<String>,
    set_name: Option<String>,
    quantity: f64,
}

fn item_name(item: &ApiOrderItem) -> String {
    item.strain_name
        .clone()
        .or_else(|| item.accessory_name.clone())
        .or_else(|| item.tea_name.clone())
        .or_else(|| item.set_name.clone())
        .unwrap_or_else(|| "Unknown".to_string())
}

#[derive(Debug, Deserialize)]
struct OrdersResponse {
    orders: Vec<ApiOrder>,
}

const ORDER_PIPELINE: &[&str] = &[
    "pending",
    "confirmed",
    "preparing",
    "ready",
    "out_for_delivery",
    "delivered",
];

fn status_style(status: &str) -> (&'static str, &'static str) {
    match status.to_lowercase().as_str() {
        "pending" => ("#ffe600", "⏳ Pending"),
        "confirmed" => ("#00e5ff", "✅ Confirmed"),
        "preparing" => ("#ff9d00", "🔥 Preparing"),
        "ready" => ("#39ff14", "📦 Ready"),
        "out_for_delivery" => ("#00e5ff", "🚗 Out for Delivery"),
        "completed" | "delivered" => ("#39ff14", "✅ Delivered"),
        "cancelled" | "rejected" => ("#ff4757", "❌ Cancelled"),
        _ => ("#8b8b9e", "📋 Unknown"),
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
    fn label(&self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Active => "🔄 Active",
            Self::Completed => "✅ Completed",
            Self::Cancelled => "❌ Cancelled",
        }
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

fn pipeline_index(status: &str) -> usize {
    ORDER_PIPELINE
        .iter()
        .position(|&s| s == status)
        .unwrap_or(ORDER_PIPELINE.len())
}

fn status_progress(status: &str) -> (&str, &str) {
    // (label for step, emoji)
    match status {
        "pending" => ("Received", "📥"),
        "confirmed" => ("Confirmed", "✅"),
        "preparing" => ("Preparing", "🔥"),
        "ready" => ("Ready", "📦"),
        "out_for_delivery" => ("On the way", "🚗"),
        "delivered" | "completed" => ("Delivered", "🎉"),
        _ => (status, ""),
    }
}

#[component]
fn PipelineSegment(i: usize, current: usize, last: bool) -> Element {
    let active = i <= current;
    let bg = if active { "#39ff14" } else { "#2a2a4a" };
    let flex = if last { "0 0 8px" } else { "1" };
    let shape = if last { "border-radius: 50%;" } else { "border-radius: 2px;" };
    rsx! {
        div { style: "{shape} height: 4px; background: {bg}; flex: {flex}; min-width: 8px;" }
        if !last {
            div { style: "width: 4px; height: 4px; background: {bg};" }
        }
    }
}

/// Compact horizontal progress tracker for the delivery pipeline.
#[component]
fn StatusProgress(status: String) -> Element {
    let current = pipeline_index(&status);
    let _terminal = is_terminal_status(&status);
    let cancelled = status == "cancelled" || status == "rejected";
    let (step_label, emoji) = status_progress(&status);
    rsx! {
        div { style: "margin-bottom: 8px;",
            div { style: "font-size: 12px; color: #8b8b9e; margin-bottom: 4px;",
                "{emoji} {step_label}"
            }
            div { style: "display: flex; align-items: center; gap: 4px;",
                if cancelled {
                    div { style: "flex:1;height:4px;background:#ff4757;border-radius:2px;" }
                } else {
                    for i in 0..ORDER_PIPELINE.len() {
                        PipelineSegment { i, current, last: i == ORDER_PIPELINE.len() - 1 }
                    }
                }
            }
        }
    }
}

#[derive(Props, PartialEq, Clone)]
struct ReviewFormProps {
    order_id: String,
    strain_id: String,
    strain_name: String,
    on_close: EventHandler<()>,
    on_submitted: EventHandler<()>,
}

#[component]
fn ReviewForm(props: ReviewFormProps) -> Element {
    let client = use_api_client();
    let mut rating = use_signal(|| 5i32);
    let mut comment = use_signal(|| String::new());
    let mut submitting = use_signal(|| false);
    let mut done = use_signal(|| false);
    let lang = crate::ui::lang::current_lang();

    rsx! {
        div {
            style: "position:fixed;inset:0;background:rgba(0,0,0,0.85);display:flex;align-items:center;justify-content:center;z-index:1100;padding:16px;",
            onclick: move |_| props.on_close.call(()),
            div {
                style: "background:#16213e;border:4px solid #2a2a4a;box-shadow:4px 4px 0 #000;max-width:420px;width:100%;padding:16px;",
                onclick: move |e: Event<MouseData>| e.stop_propagation(),
                div { style: "font-size:18px;font-weight:800;color:#39ff14;margin-bottom:4px;text-shadow:1px 1px 0 #000;",
                    {t(lang, T_REVIEW_LEAVE)}
                }
                div { style: "font-size:14px;color:#8b8b9e;margin-bottom:12px;", "{props.strain_name}" }

                if done() {
                    div { style: "text-align:center;padding:24px 0;",
                        div { style: "font-size:40px;margin-bottom:8px;", "🙌" }
                        div { style: "font-size:15px;color:#39ff14;", {t(lang, T_REVIEW_THANKS)} }
                    }
                } else {
                    div { style: "margin-bottom:12px;",
                        div { style: "font-size:13px;color:#8b8b9e;margin-bottom:6px;", {t(lang, T_REVIEW_RATING)} }
                        div { style: "display:flex;gap:8px;",
                            for star in 1..=5 {
                                {
                                    let selected = rating() >= star;
                                    let color = if selected { "#ffe600" } else { "#2a2a4a" };
                                    rsx! {
                                        button {
                                            style: "width:44px;height:44px;font-size:24px;background:{color};border:4px solid #1a1a2e;box-shadow:2px 2px 0 #000;cursor:pointer;",
                                            onclick: move |_| rating.set(star),
                                            "★"
                                        }
                                    }
                                }
                            }
                        }
                    }

                    div { style: "margin-bottom:12px;",
                        div { style: "font-size:13px;color:#8b8b9e;margin-bottom:6px;", {t(lang, T_REVIEW_COMMENT)} }
                        textarea {
                            style: "width:100%;min-height:80px;background:#1a1a2e;border:2px solid #2a2a4a;color:#e8e8e8;padding:8px;font-size:14px;resize:vertical;",
                            value: "{comment()}",
                            oninput: move |e: Event<FormData>| comment.set(e.value().clone()),
                        }
                    }

                    button {
                        style: "font-size:14px;font-weight:700;width:100%;padding:12px 20px;margin-bottom:8px;background:#39ff14;color:#000;border:4px solid #2d9e0f;box-shadow:3px 3px 0 #000;cursor:pointer;",
                        disabled: submitting(),
                        onclick: move |e: Event<MouseData>| {
                            e.stop_propagation();
                            let req = crate::ui::api::types::CreateReviewRequest {
                                order_id: props.order_id.clone(),
                                strain_id: props.strain_id.clone(),
                                rating: rating(),
                                comment: comment().trim().to_string(),
                            };
                            let client = client.clone();
                            spawn(async move {
                                submitting.set(true);
                                let _ = client.create_review(&req).await;
                                submitting.set(false);
                                done.set(true);
                            });
                        },
                        {t(lang, T_REVIEW_SUBMIT)}
                    }
                }

                button {
                    style: "font-size:14px;font-weight:700;width:100%;padding:12px 20px;background:#2a2a4a;color:#e8e8e8;border:4px solid #1a1a2e;box-shadow:3px 3px 0 #000;cursor:pointer;",
                    onclick: move |e: Event<MouseData>| { e.stop_propagation(); props.on_close.call(()); },
                    "Закрыть"
                }
            }
        }
    }
}

#[component]
pub fn OrdersScreen() -> Element {
    let mut active_filter = use_signal(|| StatusFilter::All);
    let mut review_target = use_signal(|| None::<(ApiOrder, ApiOrderItem)>);
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

    let orders_title = t(crate::ui::lang::current_lang(), T_ORDERS_TITLE);
    let loading_text = t(crate::ui::lang::current_lang(), T_LOADING);

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
                for filter in [StatusFilter::All, StatusFilter::Active, StatusFilter::Completed, StatusFilter::Cancelled] {
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
                                        let total_str = crate::trios::pricing::format_baht(o.total);
                                        let is_cancelled = o.status == "cancelled" || o.status == "rejected";
                                        let opacity = if is_cancelled { "0.7" } else { "1" };
                                        let border_color = if is_cancelled { "#2a2a4a" } else { status_color };
                                        let is_active = !is_terminal_status(&o.status);
                                        let shadow = if is_active { "4px 4px 0 #000, 0 0 12px rgba(0,229,255,0.1)" } else { "4px 4px 0 #000" };

                                        rsx! {
                                            div { style: "
                                                background: #16213e; border: 4px solid {border_color};
                                                border-radius: 0; padding: 14px;
                                                box-shadow: {shadow};
                                                opacity: {opacity};
                                            ",
                                                StatusProgress { status: o.status.clone() }
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
                                                {(o.status == "delivered" || o.status == "completed").then(|| {
                                                    let reviewables: Vec<&ApiOrderItem> = o.items.iter().filter(|i| i.strain_id.is_some()).collect();
                                                    if reviewables.is_empty() {
                                                        return rsx! {};
                                                    }
                                                    let lang = crate::ui::lang::current_lang();
                                                    let order_for_review = o.clone();
                                                    rsx! {
                                                        div { style: "margin-bottom: 8px; padding-top: 8px; border-top: 1px dashed #2a2a4a;",
                                                            div { style: "font-size: 12px; color: #8b8b9e; margin-bottom: 6px;", {t(lang, T_REVIEW_LEAVE)} }
                                                            div { style: "display: flex; flex-wrap: wrap; gap: 6px;",
                                                                for item in reviewables {{
                                                                    let item_clone = item.clone();
                                                                    let order_clone = order_for_review.clone();
                                                                    let name = item_name(&item_clone);
                                                                    rsx! {
                                                                        button {
                                                                            style: "font-size: 12px; padding: 6px 10px; background: #ffe600; color: #000; border: 3px solid #bfa600; box-shadow: 2px 2px 0 #000; cursor: pointer;",
                                                                            onclick: move |_| review_target.set(Some((order_clone.clone(), item_clone.clone()))),
                                                                            "★ {name}"
                                                                        }
                                                                    }
                                                                }}
                                                            }
                                                        }
                                                    }
                                                })}
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

            {review_target().map(|(order, item)| {
                let strain_id = item.strain_id.clone().unwrap_or_default();
                let strain_name = item_name(&item);
                rsx! {
                    ReviewForm {
                        order_id: order.id.clone(),
                        strain_id,
                        strain_name,
                        on_close: move |_| review_target.set(None),
                        on_submitted: move |_| review_target.set(None),
                    }
                }
            })}

            BottomNav {}
        }
    }
}
