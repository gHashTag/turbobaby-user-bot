//! Full-screen product detail popup, opened by tapping a catalog card.
//!
//! Catalog cards (bikes/accessories/tea/sets) only had room for a truncated
//! one-line description (or, for bikes, none at all) — tapping a card did
//! nothing. This modal surfaces the **full** description plus the key meta
//! fields when a card is tapped.
//!
//! Structurally it mirrors the per-card video popup that already ships in
//! `catalog`/`accessories_screen` (fixed `inset:0` overlay at z-index
//! 1000, tap-the-backdrop-to-close, inner container that stops propagation,
//! explicit close button) — that pattern is proven inside the Telegram
//! WebView, so we reuse its shape here instead of inventing a new one. All
//! meta fields are optional because accessories/tea/sets have no spec line.
//!
//! The per-product reviews and lab-certificate sections were removed with the
//! old catalog: their tables are dropped, and the motorbike counterpart —
//! service records — is admin-only by decision (D6), because a public
//! "serviced" badge we cannot keep accurate is exactly the kind of number the
//! data-honesty rule forbids.

use crate::trios::i18n::{
    t, T_ADD_TO_CART, T_FULFILLMENT_DINE_IN, T_FULFILLMENT_LABEL, T_FULFILLMENT_TAKEAWAY,
    T_MODAL_CLOSE, T_MODAL_DECREASE_QTY, T_MODAL_INCREASE_QTY, T_SHARE,
};
use crate::ui::components::image_lightbox::ImageLightbox;
use crate::ui::lang::current_lang;
use crate::ui::telegram::TelegramApp;
use dioxus::prelude::*;

#[derive(Props, PartialEq, Clone)]
pub struct ProductDetailModalProps {
    /// Product name (modal title).
    pub name: String,
    /// Optional hero image URL. Rendered only when it looks like a usable
    /// http(s) or root-relative path (same guard the cards use).
    #[props(default)]
    pub image_url: Option<String>,
    /// Full, untruncated description. Empty string => description block is
    /// omitted (the modal still opens — name/price are useful on their own).
    pub description: String,
    /// Preformatted category badge text, e.g. "🛵 Scooter".
    #[props(default)]
    pub category_badge: Option<String>,
    /// Preformatted primary spec, e.g. "155 cc".
    #[props(default)]
    pub spec_primary: Option<String>,
    /// Preformatted secondary spec, e.g. "Automatic".
    #[props(default)]
    pub spec_secondary: Option<String>,
    /// Effect line (already localized / extracted by the caller).
    #[props(default)]
    pub effect: Option<String>,
    /// Extra detail line (caller does not prefix the emoji).
    #[props(default)]
    pub flavor: Option<String>,
    /// Preformatted price line, e.g. "฿500/day" — a round placeholder, not a
    /// tariff: the caller formats whatever the shop published, and a dash when
    /// it published nothing.
    #[props(default)]
    pub price_line: Option<String>,
    /// When true (default), show an "add to cart" button that calls
    /// `on_add_to_cart` then closes the modal. Pass `false` for sold-out
    /// items so the modal stays informational.
    #[props(default = true)]
    pub can_add: bool,
    /// Localized label for the add-to-cart button. Defaults to RU.
    #[props(default)]
    pub add_to_cart_label: Option<String>,
    /// Called when the user taps the add-to-cart button with the chosen
    /// quantity (caller pushes the item into the cart). The modal closes
    /// itself afterwards.
    pub on_add_to_cart: EventHandler<u32>,
    /// Optional drink fulfillment selector. Pass `["dine_in", "takeaway"]` for
    /// tea/drink items; omit (empty) for bikes/accessories/sets.
    #[props(default)]
    pub fulfillment_options: Vec<String>,
    /// Pre-selected fulfillment value. Falls back to `"takeaway"` when omitted.
    #[props(default)]
    pub initial_fulfillment: Option<String>,
    /// Called whenever the user changes the fulfillment toggle.
    /// The parent can mirror the choice into its own add-to-cart state.
    #[props(default)]
    pub on_fulfillment_change: Option<EventHandler<String>>,
    /// When provided, render a "Share" button that calls `on_share`.
    /// Pass `None` for non-admin users so the button is hidden.
    #[props(default)]
    pub on_share: Option<EventHandler<()>>,
    /// Localized label for the share button.
    #[props(default)]
    pub share_label: Option<String>,
    /// Called when the user taps the backdrop or the close button.
    pub on_close: EventHandler<()>,
}

/// `true` when `url` is a usable image/video src (matches the card guards).
fn is_usable_src(url: &str) -> bool {
    !url.is_empty()
        && (url.starts_with("http://")
            || url.starts_with("https://")
            || (url.starts_with('/') && !url.starts_with("//")))
}

#[component]
pub fn ProductDetailModal(props: ProductDetailModalProps) -> Element {
    let on_close = props.on_close;
    let on_add_to_cart = props.on_add_to_cart;
    let can_add = props.can_add;
    let add_label = props
        .add_to_cart_label
        .clone()
        .unwrap_or_else(|| t(current_lang(), T_ADD_TO_CART).to_string());
    let share_label = props
        .share_label
        .clone()
        .unwrap_or_else(|| t(current_lang(), T_SHARE).to_string());
    let on_share = props.on_share;
    let img = props.image_url.clone().unwrap_or_default();
    let has_image = is_usable_src(&img);
    let name = props.name.clone();
    let alt = props.name.clone();
    let description = props.description.clone();
    // Quantity selector state (1..=99). Only meaningful when `can_add`.
    let mut qty = use_signal(|| 1u32);
    let initial_fulfillment = props.initial_fulfillment.clone();
    let mut fulfillment = use_signal(move || {
        initial_fulfillment
            .clone()
            .unwrap_or_else(|| "takeaway".to_string())
    });
    let fulfillment_options = props.fulfillment_options.clone();
    let on_fulfillment_change = props.on_fulfillment_change;
    let mut lightbox_open = use_signal(|| false);
    let lightbox_src = img.clone();
    let lightbox_alt = alt.clone();

    rsx! {
        div {
            style: "position:fixed;inset:0;background:rgba(0,0,0,0.85);display:flex;align-items:center;justify-content:center;z-index:1000;padding:16px;",
            onclick: move |_| on_close.call(()),
            div {
                style: "background:#16213e;border:4px solid #2a2a4a;box-shadow:4px 4px 0 #000;max-width:480px;width:100%;max-height:85vh;overflow:auto;position:relative;",
                role: "dialog",
                "aria-modal": "true",
                "aria-label": "{name}",
                // WCAG 2.4.3: move focus into the dialog on open.
                tabindex: "-1",
                onmounted: move |e: Event<MountedData>| {
                    spawn(async move {
                        let _ = e.set_focus(true).await;
                    });
                },
                // WCAG 2.1.2: Escape closes the dialog (keyboard parity with the ✕).
                onkeydown: move |e: Event<KeyboardData>| {
                    if e.key() == Key::Escape {
                        on_close.call(());
                    }
                },
                onclick: move |e: Event<MouseData>| e.stop_propagation(),

                // Close (✕) — top-right, always reachable while scrolling.
                button {
                    style: "position:absolute;top:8px;right:8px;width:44px;height:44px;border-radius:50%;background:rgba(0,0,0,0.6);border:1px solid #fff;color:#fff;font-size:18px;line-height:1;display:flex;align-items:center;justify-content:center;cursor:pointer;z-index:2;",
                    "aria-label": "{t(current_lang(), T_MODAL_CLOSE)}",
                    onclick: move |e: Event<MouseData>| { e.stop_propagation(); on_close.call(()); },
                    "✕"
                }

                {has_image.then(|| rsx! {
                    // Full image, natural aspect ratio — promo art is tall,
                    // so the old square crop (aspect-ratio:1/1 + object-fit:
                    // cover) cut off the top/bottom. width:100% + height:auto shows
                    // the WHOLE image; the dialog (max-height:85vh, overflow:auto)
                    // scrolls. Tapping opens the app-controlled lightbox because the
                    // mini-app viewport disables native pinch-zoom.
                    div {
                        style: "width:100%;background:linear-gradient(135deg,#1a1a2e,#16213e);cursor:pointer;position:relative;",
                        onclick: move |_| lightbox_open.set(true),
                        img {
                            src: "{img}",
                            alt: "{alt}",
                            loading: "lazy",
                            style: "width:100%;height:auto;object-fit:contain;display:block;"
                        }
                        span {
                            style: "position:absolute;bottom:8px;right:8px;width:40px;height:40px;border-radius:50%;background:rgba(0,0,0,0.55);border:1px solid #fff;color:#fff;font-size:18px;display:flex;align-items:center;justify-content:center;pointer-events:none;",
                            "🔍"
                        }
                    }
                })}

                div { style: "padding:16px;",
                    div { style: "font-size:20px;font-weight:800;margin-bottom:8px;color:#fff;line-height:1.2;text-shadow:2px 2px 0 #000;",
                        "{name}"
                    }

                    div { style: "display:flex;gap:8px;align-items:center;margin-bottom:10px;flex-wrap:wrap;",
                        {props.category_badge.as_ref().filter(|s| !s.is_empty()).map(|b| rsx! {
                            span { style: "font-size:13px;color:#b388ff;font-weight:700;", "{b}" }
                        })}
                        {props.spec_primary.as_ref().filter(|s| !s.is_empty()).map(|s| rsx! {
                            span { style: "font-size:13px;color:#39ff14;font-weight:700;", "{s}" }
                        })}
                        {props.spec_secondary.as_ref().filter(|s| !s.is_empty()).map(|s| rsx! {
                            span { style: "font-size:13px;color:#00e5ff;font-weight:600;", "{s}" }
                        })}
                    }

                    {props.effect.as_ref().filter(|s| !s.is_empty()).map(|s| rsx! {
                        div { style: "font-size:14px;color:#aaa;margin-bottom:6px;line-height:1.4;", "{s}" }
                    })}
                    {props.flavor.as_ref().filter(|s| !s.is_empty()).map(|s| rsx! {
                        div { style: "font-size:14px;color:#888;margin-bottom:10px;line-height:1.4;", "⚙️ {s}" }
                    })}

                    {(!description.is_empty()).then(|| rsx! {
                        div { style: "font-size:14px;color:#ddd;line-height:1.55;white-space:pre-wrap;margin-bottom:12px;",
                            "{description}"
                        }
                    })}

                    {props.price_line.as_ref().filter(|s| !s.is_empty()).map(|p| rsx! {
                        div { style: "font-size:22px;font-weight:800;color:#ffe600;text-shadow:2px 2px 0 #000;margin-bottom:12px;", "{p}" }
                    })}

                    {can_add.then(|| {
                        // Quantity stepper: − [n] +, clamped to 1..=99. Buttons
                        // are *disabled* (not hidden) at the bounds — Baymard's
                        // "disable, don't hide" guidance.
                        let cur = qty();
                        let at_min = cur <= 1;
                        let at_max = cur >= 99;
                        let min_op = if at_min { "0.35" } else { "1" };
                        let min_cur = if at_min { "not-allowed" } else { "pointer" };
                        let max_op = if at_max { "0.35" } else { "1" };
                        let max_cur = if at_max { "not-allowed" } else { "pointer" };
                        rsx! {
                        div { style: "display:flex;align-items:center;justify-content:center;gap:12px;margin-bottom:10px;",
                            button {
                                style: "width:44px;height:44px;font-size:22px;font-weight:800;background:#2a2a4a;color:#e8e8e8;border:4px solid #1a1a2e;box-shadow:2px 2px 0 #000;line-height:1;opacity:{min_op};cursor:{min_cur};",
                                "aria-label": "{t(current_lang(), T_MODAL_DECREASE_QTY)}",
                                disabled: at_min,
                                onclick: move |e: Event<MouseData>| { e.stop_propagation(); qty.set(qty().saturating_sub(1).max(1)); },
                                "−"
                            }
                            span { style: "font-size:20px;font-weight:800;color:#fff;min-width:40px;text-align:center;", "{cur}" }
                            button {
                                style: "width:44px;height:44px;font-size:22px;font-weight:800;background:#2a2a4a;color:#e8e8e8;border:4px solid #1a1a2e;box-shadow:2px 2px 0 #000;line-height:1;opacity:{max_op};cursor:{max_cur};",
                                "aria-label": "{t(current_lang(), T_MODAL_INCREASE_QTY)}",
                                disabled: at_max,
                                onclick: move |e: Event<MouseData>| { e.stop_propagation(); qty.set((qty() + 1).min(99)); },
                                "+"
                            }
                        }

                        if !fulfillment_options.is_empty() {
                            div { style: "margin-bottom:10px;",
                                div { style: "font-size:12px;color:#8b8b9e;margin-bottom:6px;", "{t(current_lang(), T_FULFILLMENT_LABEL)}" }
                                div { style: "display:flex;gap:8px;",
                                    for opt in fulfillment_options.iter() {
                                    {
                                        let opt = opt.clone();
                                        let selected = fulfillment() == opt;
                                        let border = if selected { "#39ff14" } else { "#2a2a4a" };
                                        let color = if selected { "#fff" } else { "#8b8b9e" };
                                        let bg = if selected { "#0f2a0f" } else { "#0f0f1a" };
                                        let label = match opt.as_str() {
                                            "dine_in" => t(current_lang(), T_FULFILLMENT_DINE_IN).to_string(),
                                            _ => t(current_lang(), T_FULFILLMENT_TAKEAWAY).to_string(),
                                        };
                                        let opt_for_handler = opt.clone();
                                        let handler = on_fulfillment_change;
                                        rsx! {
                                            button {
                                                style: "flex:1;padding:10px;border:3px solid {border};background:{bg};color:{color};font-size:13px;font-weight:700;cursor:pointer;",
                                                onclick: move |e: Event<MouseData>| {
                                                    e.stop_propagation();
                                                    fulfillment.set(opt.clone());
                                                    if let Some(ref h) = handler {
                                                        h.call(opt_for_handler.clone());
                                                    }
                                                },
                                                "{label}"
                                            }
                                        }
                                    }
                                }
                                }
                            }
                        }

                        button {
                            style: "font-size:14px;font-weight:700;width:100%;padding:12px 20px;margin-bottom:8px;background:#39ff14;color:#000;border:4px solid #2d9e0f;box-shadow:3px 3px 0 #000;cursor:pointer;",
                            onclick: move |e: Event<MouseData>| {
                                e.stop_propagation();
                                TelegramApp::init().haptic_notification(crate::ui::telegram::HapticNotification::Success);
                                on_add_to_cart.call(qty());
                                on_close.call(());
                            },
                            "{add_label}"
                        }
                        }
                    })}

                    {on_share.map(|share| {
                        rsx! {
                            button {
                                style: "font-size:14px;font-weight:700;width:100%;padding:12px 20px;margin-bottom:8px;background:#00e5ff;color:#000;border:4px solid #008ba3;box-shadow:3px 3px 0 #000;cursor:pointer;",
                                onclick: move |e: Event<MouseData>| { e.stop_propagation(); share.call(()); },
                                "🔗 {share_label}"
                            }
                        }
                    })}

                    button {
                        style: "font-size:14px;font-weight:700;width:100%;padding:12px 20px;background:#2a2a4a;color:#e8e8e8;border:4px solid #1a1a2e;box-shadow:3px 3px 0 #000;cursor:pointer;",
                        onclick: move |e: Event<MouseData>| { e.stop_propagation(); on_close.call(()); },
                        "{t(current_lang(), T_MODAL_CLOSE)}"
                    }
                }
            }
        }
        {lightbox_open().then(|| rsx! {
            ImageLightbox {
                src: lightbox_src.clone(),
                alt: lightbox_alt.clone(),
                on_close: move |_| lightbox_open.set(false),
            }
        })}
    }
}
