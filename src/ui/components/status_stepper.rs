// Shared order-status stepper used by the orders list and order detail screen.
//
// Renders a compact horizontal pipeline (pending → confirmed → preparing → ready
// → out_for_delivery → delivered) with a localized status label. Terminal or
// cancelled orders render a single status-coloured bar instead of the pipeline.

use crate::trios::i18n::{
    t,
    T_ORDERS_STATUS_CANCELLED, T_ORDERS_STATUS_CONFIRMED, T_ORDERS_STATUS_DELIVERED,
    T_ORDERS_STATUS_OUT_FOR_DELIVERY, T_ORDERS_STATUS_PENDING, T_ORDERS_STATUS_PREPARING,
    T_ORDERS_STATUS_READY, T_ORDERS_STATUS_UNKNOWN, T_ORDERS_STEP_CONFIRMED,
    T_ORDERS_STEP_DELIVERED, T_ORDERS_STEP_ON_THE_WAY, T_ORDERS_STEP_PREPARING,
    T_ORDERS_STEP_READY, T_ORDERS_STEP_RECEIVED,
};
use dioxus::prelude::*;

const ORDER_PIPELINE: &[&str] = &[
    "pending",
    "confirmed",
    "preparing",
    "ready",
    "out_for_delivery",
    "delivered",
];

fn pipeline_index(status: &str) -> usize {
    ORDER_PIPELINE
        .iter()
        .position(|&s| s == status)
        .unwrap_or(ORDER_PIPELINE.len())
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

fn step_label(status: &str, lang: crate::trios::core::Lang) -> (String, &'static str) {
    let (key, emoji) = match status {
        "pending" => (T_ORDERS_STEP_RECEIVED, "📥"),
        "confirmed" => (T_ORDERS_STEP_CONFIRMED, "✅"),
        "preparing" => (T_ORDERS_STEP_PREPARING, "🔥"),
        "ready" => (T_ORDERS_STEP_READY, "📦"),
        "out_for_delivery" => (T_ORDERS_STEP_ON_THE_WAY, "🚗"),
        "delivered" | "completed" => (T_ORDERS_STEP_DELIVERED, "🎉"),
        _ => return (status.to_string(), ""),
    };
    (t(lang, key).to_string(), emoji)
}

#[component]
fn PipelineSegment(i: usize, current: usize, last: bool) -> Element {
    let active = i <= current;
    let bg = if active { "#39ff14" } else { "#2a2a4a" };
    let flex = if last { "0 0 8px" } else { "1" };
    let shape = if last {
        "border-radius: 50%;"
    } else {
        "border-radius: 2px;"
    };
    rsx! {
        div { style: "{shape} height: 4px; background: {bg}; flex: {flex}; min-width: 8px;" }
        if !last {
            div { style: "width: 4px; height: 4px; background: {bg};" }
        }
    }
}

/// Compact horizontal order-status tracker.
///
/// `margin_bottom` lets callers tune spacing for the list vs detail contexts.
#[component]
pub fn StatusStepper(
    status: String,
    #[props(default = 8)] margin_bottom: u32,
) -> Element {
    let lang = crate::ui::lang::current_lang();
    let current = pipeline_index(&status);
    let cancelled = status == "cancelled" || status == "rejected";
    let (label, emoji) = step_label(&status, lang);
    let color = status_color(&status);
    let status_label = t(lang, status_label_key(&status));

    rsx! {
        div { style: "margin-bottom: {margin_bottom}px;",
            div { style: "display: flex; justify-content: space-between; align-items: center; margin-bottom: 6px;",
                span { style: "font-size: 13px; color: #8b8b9e;",
                    "{emoji} {label}"
                }
                span { style: "font-size: 12px; color: {color};",
                    "{status_label}"
                }
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
