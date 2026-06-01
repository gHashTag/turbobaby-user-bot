//! Compact red banner for surfaced errors.
//!
//! Used by data-driven screens when a fetch/action fails — keeps the visual
//! grammar consistent across orders, profile, treasure_hunt, tech_tree, etc.
//! Replaces the inline `div { style: "...red background..." }` blocks that
//! cycles #19-29 kept duplicating.
//!
//! For success/neutral toasts use the existing `Toast` component or a local
//! transient signal — this one is intentionally bound to error-state visuals.

use dioxus::prelude::*;

#[derive(Props, PartialEq, Clone)]
pub struct ErrorBannerProps {
    /// Message to display. Empty string renders nothing.
    pub message: String,
    /// Optional emoji or short prefix; defaults to "⚠️".
    #[props(default)]
    pub icon: Option<String>,
    /// Override outer margin (rare; pass e.g. "0 16px 12px"). Default matches
    /// the orders/treasure_hunt screens we already shipped.
    #[props(default)]
    pub margin: Option<String>,
}

#[component]
pub fn ErrorBanner(props: ErrorBannerProps) -> Element {
    if props.message.is_empty() {
        return rsx! {};
    }
    let icon = props.icon.as_deref().unwrap_or("⚠️");
    let margin = props.margin.as_deref().unwrap_or("0 16px 12px");
    let msg = props.message.clone();
    rsx! {
        div { style: "
            margin: {margin}; padding: 8px 12px;
            background: rgba(255,71,87,0.1); border: 4px solid #ff4757;
            color: #ff4757; font-size: 13px; text-align: center;
            box-shadow: 3px 3px 0 #000;
        ",
            "{icon} {msg}"
        }
    }
}
