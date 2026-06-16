//! Full-screen product detail popup, opened by tapping a catalog card.
//!
//! Catalog cards (menu/accessories/tea/sets) only had room for a truncated
//! one-line description (or, for strains, none at all) — tapping a card did
//! nothing. This modal surfaces the **full** description plus the key meta
//! fields when a card is tapped.
//!
//! Structurally it mirrors the per-card video popup that already ships in
//! `menu_screen`/`accessories_screen` (fixed `inset:0` overlay at z-index
//! 1000, tap-the-backdrop-to-close, inner container that stops propagation,
//! explicit close button) — that pattern is proven inside the Telegram
//! WebView, so we reuse its shape here instead of inventing a new one. All
//! meta fields are optional because accessories/tea/sets have no THC etc.

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
    /// Preformatted category badge text, e.g. "🌿 Hybrid".
    #[props(default)]
    pub category_badge: Option<String>,
    /// Preformatted "THC 24%".
    #[props(default)]
    pub thc: Option<String>,
    /// Preformatted "CBD 1.0%".
    #[props(default)]
    pub cbd: Option<String>,
    /// Effect line (already localized / extracted by the caller).
    #[props(default)]
    pub effect: Option<String>,
    /// Flavor profile (caller does not prefix the leaf emoji).
    #[props(default)]
    pub flavor: Option<String>,
    /// Preformatted price line, e.g. "฿350/g" or "฿1200".
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
    /// Called when the user taps the add-to-cart button (caller pushes the
    /// item into the cart). The modal closes itself afterwards.
    pub on_add_to_cart: EventHandler<()>,
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
        .unwrap_or_else(|| "В корзину 🛒".to_string());
    let img = props.image_url.clone().unwrap_or_default();
    let has_image = is_usable_src(&img);
    let name = props.name.clone();
    let alt = props.name.clone();
    let description = props.description.clone();

    rsx! {
        div {
            style: "position:fixed;inset:0;background:rgba(0,0,0,0.85);display:flex;align-items:center;justify-content:center;z-index:1000;padding:16px;",
            onclick: move |_| on_close.call(()),
            div {
                style: "background:#16213e;border:4px solid #2a2a4a;box-shadow:4px 4px 0 #000;max-width:480px;width:100%;max-height:85vh;overflow:auto;position:relative;",
                onclick: move |e: Event<MouseData>| e.stop_propagation(),

                // Close (✕) — top-right, always reachable while scrolling.
                button {
                    style: "position:absolute;top:8px;right:8px;width:32px;height:32px;border-radius:50%;background:rgba(0,0,0,0.6);border:1px solid #fff;color:#fff;font-size:16px;line-height:1;display:flex;align-items:center;justify-content:center;cursor:pointer;z-index:2;",
                    onclick: move |e: Event<MouseData>| { e.stop_propagation(); on_close.call(()); },
                    "✕"
                }

                {has_image.then(|| rsx! {
                    div { style: "width:100%;aspect-ratio:1/1;background:linear-gradient(135deg,#1a1a2e,#16213e);overflow:hidden;",
                        img {
                            src: "{img}",
                            alt: "{alt}",
                            loading: "lazy",
                            style: "width:100%;height:100%;object-fit:cover;display:block;"
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
                        {props.thc.as_ref().filter(|s| !s.is_empty()).map(|s| rsx! {
                            span { style: "font-size:13px;color:#39ff14;font-weight:700;", "{s}" }
                        })}
                        {props.cbd.as_ref().filter(|s| !s.is_empty()).map(|s| rsx! {
                            span { style: "font-size:13px;color:#00e5ff;font-weight:600;", "{s}" }
                        })}
                    }

                    {props.effect.as_ref().filter(|s| !s.is_empty()).map(|s| rsx! {
                        div { style: "font-size:14px;color:#aaa;margin-bottom:6px;line-height:1.4;", "{s}" }
                    })}
                    {props.flavor.as_ref().filter(|s| !s.is_empty()).map(|s| rsx! {
                        div { style: "font-size:14px;color:#888;margin-bottom:10px;line-height:1.4;", "🍃 {s}" }
                    })}

                    {(!description.is_empty()).then(|| rsx! {
                        div { style: "font-size:14px;color:#ddd;line-height:1.55;white-space:pre-wrap;margin-bottom:12px;",
                            "{description}"
                        }
                    })}

                    {props.price_line.as_ref().filter(|s| !s.is_empty()).map(|p| rsx! {
                        div { style: "font-size:22px;font-weight:800;color:#ffe600;text-shadow:2px 2px 0 #000;margin-bottom:12px;", "{p}" }
                    })}

                    {can_add.then(|| rsx! {
                        button {
                            style: "font-size:14px;font-weight:700;width:100%;padding:12px 20px;margin-bottom:8px;background:#39ff14;color:#000;border:4px solid #2d9e0f;box-shadow:3px 3px 0 #000;cursor:pointer;",
                            onclick: move |e: Event<MouseData>| { e.stop_propagation(); on_add_to_cart.call(()); on_close.call(()); },
                            "{add_label}"
                        }
                    })}

                    button {
                        style: "font-size:14px;font-weight:700;width:100%;padding:12px 20px;background:#2a2a4a;color:#e8e8e8;border:4px solid #1a1a2e;box-shadow:3px 3px 0 #000;cursor:pointer;",
                        onclick: move |e: Event<MouseData>| { e.stop_propagation(); on_close.call(()); },
                        "Закрыть"
                    }
                }
            }
        }
    }
}
