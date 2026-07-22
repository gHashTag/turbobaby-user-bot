//! Dedicated set card with a uniform footprint.
//!
//! Unlike the free-form `render_set_card` in `sets_screen.rs` (which shows the
//! whole image at natural proportions and therefore has variable height), this
//! component locks the card to a fixed aspect ratio and fills the media area
//! with `width:100%;height:100%;object-fit:cover`.  Every card in the grid has
//! the same height because the height is derived from the width via
//! `aspect-ratio`.

use crate::trios::i18n::{t, T_ADD_TO_CART};
use crate::ui::components::product_detail_modal::ProductDetailModal;
use crate::ui::components::video_modal::VideoModal;
use crate::ui::lang;
use crate::ui::share::{share_product, ProductKind};
use crate::ui::state::{Cart, CartItem, CartItemType};
use dioxus::prelude::*;

/// Public, serializable data needed to render a set card.
#[derive(PartialEq, Clone)]
pub struct SetCardData {
    pub id: String,
    pub name: String,
    pub name_en: Option<String>,
    pub description: Option<String>,
    pub description_en: Option<String>,
    pub icon: Option<String>,
    pub total_price: f64,
    pub discount_percent: f64,
    pub image_url: Option<String>,
    pub video_url: Option<String>,
    pub is_available: Option<bool>,
    pub strain_count: usize,
    pub total_weight_grams: f64,
    pub badge: String,
}

impl SetCardData {
    pub fn effective_price(&self) -> f64 {
        let d = if self.discount_percent.is_finite() {
            self.discount_percent.max(0.0)
        } else {
            0.0
        };
        let t = if self.total_price.is_finite() {
            self.total_price.max(0.0)
        } else {
            0.0
        };
        (t * (1.0 - d / 100.0)).max(0.0)
    }
}

/// Render a uniform-height set card for the Sets grid.
///
/// Kept as a plain function (not a `#[component]`) because it needs a
/// `Signal<Cart>` handle; passing signals through `#[component]` props is
/// fragile in Dioxus 0.6 and the inline function call works reliably.
pub fn render_uniform_set_card(
    set: SetCardData,
    cart: Signal<Cart>,
    accent: &'static str,
) -> Element {
    let mut cart = cart;
    let add_to_cart_label = t(lang::current_lang(), T_ADD_TO_CART).to_string();
    let price = set.effective_price();
    let has_discount = set.discount_percent > 0.0 && set.discount_percent.is_finite();
    let original_price_str = crate::trios::pricing::format_baht(set.total_price);
    let price_str = crate::trios::pricing::format_baht(price);
    let discount_badge = if has_discount {
        format!("{}% OFF", set.discount_percent as i32)
    } else {
        String::new()
    };

    let set_name = lang::localized(&set.name, set.name_en.as_deref());
    let desc = lang::localized(
        set.description.as_deref().unwrap_or(""),
        set.description_en.as_deref(),
    );
    let icon = set.icon.as_deref().unwrap_or("🎁");

    let is_available = set.is_available.unwrap_or(true);
    let opacity = if is_available { "" } else { "opacity:0.6;" };

    let badge = crate::trios::packs::PackBadge::from_str(&set.badge);
    let badge_label = badge.label().map(|(ru, en)| lang::localized(ru, Some(en)));
    let badge_color = badge.color();
    let strain_word = lang::localized("сортов", Some("strains"));
    let weight_line = crate::trios::packs::weight_line(
        set.total_weight_grams,
        set.strain_count,
        &strain_word,
    );

    let mut detail_open = use_signal(|| false);
    let mut show_video = use_signal(|| false);

    let s0 = set.clone();
    let s4 = set.clone();
    let s5 = set.clone();
    let img_url = s0.image_url.clone().unwrap_or_default();
    let has_image = is_media_url(&img_url);
    let vid_url = s0.video_url.clone().unwrap_or_default();
    let has_video = is_media_url(&vid_url);

    // Portrait product card: height is 1.5× the width, so every card in the
    // 2-column grid is identical in size. The media area fills the top ~58% of
    // the card and the image/video is forced to cover that entire area.
    let card_style = format!(
        "background:#16213e;border:4px solid {};box-shadow:4px 4px 0 #000;overflow:hidden;position:relative;cursor:pointer;display:flex;flex-direction:column;height:100%;aspect-ratio:2/3;{}",
        accent, opacity
    );

    rsx! {
        div { style: card_style,
            onclick: move |_| detail_open.set(true),
            // Media area: fills top 58% of the card, image/video cover it fully.
            div { style: "flex:0 0 58%;position:relative;overflow:hidden;background:linear-gradient(135deg,#1a1a2e,#16213e);display:flex;align-items:center;justify-content:center;",
                if has_video {
                    video {
                        style: "width:100%;height:100%;object-fit:cover;display:block;",
                        src: "{vid_url}",
                        "type": "video/mp4",
                        autoplay: true,
                        muted: true,
                        "loop": true,
                        "playsinline": "true",
                        "webkit-playsinline": "true",
                        preload: "auto",
                        poster: if has_image { "{img_url}" } else { "" },
                        crossorigin: "anonymous",
                    }
                    div {
                        style: "position:absolute;inset:0;z-index:3;cursor:pointer;",
                        onclick: move |e: Event<MouseData>| { e.stop_propagation(); show_video.set(true); }
                    }
                } else if has_image {
                    img {
                        src: "{img_url}",
                        alt: "{set_name}",
                        loading: "lazy",
                        style: "width:100%;height:100%;object-fit:cover;display:block;"
                    }
                } else {
                    span { style: "font-size:40px;", "{icon}" }
                }
                // Badge stack top-left, matching strain/accessory cards.
                div { style: "position:absolute;top:8px;left:8px;display:flex;flex-direction:column;gap:4px;z-index:4;align-items:flex-start;",
                    span { style: "font-size:12px;font-weight:700;background:{accent};color:#000;padding:3px 7px;box-shadow:2px 2px 0 #000;", "📦 SET" }
                    if let Some(bl) = badge_label.clone() {
                        span { style: "font-size:10px;font-weight:700;background:{badge_color};color:#fff;padding:2px 6px;box-shadow:2px 2px 0 #000;", "{bl}" }
                    }
                    if has_discount {
                        span { style: "font-size:12px;font-weight:700;background:{accent};color:#000;padding:3px 7px;box-shadow:2px 2px 0 #000;", "{discount_badge}" }
                    }
                }
            }
            // Content area: bottom 42% of the card, tightly clamped.
            div { style: "flex:1 1 auto;padding:10px;display:flex;flex-direction:column;overflow:hidden;",
                div { style: "font-size:15px;font-weight:700;color:#fff;line-height:1.2;text-shadow:2px 2px 0 #000;display:-webkit-box;-webkit-line-clamp:2;-webkit-box-orient:vertical;overflow:hidden;margin-bottom:4px;",
                    "{set_name}"
                }
                if !weight_line.is_empty() {
                    div { style: "font-size:11px;color:#b388ff;font-weight:600;margin-bottom:3px;", "⚖️ {weight_line}" }
                }
                if !desc.is_empty() {
                    div { style: "font-size:12px;color:#888;margin-bottom:4px;line-height:1.3;display:-webkit-box;-webkit-line-clamp:1;-webkit-box-orient:vertical;overflow:hidden;",
                        "{desc}"
                    }
                }
                div { style: "display:flex;gap:4px;align-items:baseline;margin-top:auto;",
                    span { style: "font-size:18px;font-weight:800;color:#ffe600;text-shadow:2px 2px 0 #000;", "{price_str}" }
                    if has_discount {
                        span { style: "font-size:12px;color:#888;text-decoration:line-through;", "{original_price_str}" }
                    }
                }
            }
            div { style: "padding:0 10px 10px;margin-top:auto;",
                if is_available {
                    button {
                        style: "
                            font-size:13px;font-weight:700;width:100%;padding:10px 16px;
                            background:#39ff14;color:#000;
                            border:3px solid #2d9e0f;
                            box-shadow:2px 2px 0 #000;
                            cursor:pointer;
                        ",
                        onclick: move |e: Event<MouseData>| {
                            e.stop_propagation();
                            let s = s4.clone();
                            let mut c = cart.write();
                            c.add_item(CartItem {
                                id: s.id.clone(),
                                name: lang::localized(&s.name, s.name_en.as_deref()),
                                price,
                                quantity: 1,
                                image_url: None,
                                item_type: CartItemType::Set,
                                fulfillment: None,
                            });
                            crate::ui::telegram::TelegramApp::init().haptic_notification(crate::ui::telegram::HapticNotification::Success);
                        },
                        "{add_to_cart_label}"
                    }
                } else {
                    button { style: "
                        font-size:13px;font-weight:600;width:100%;padding:10px 16px;
                        background:transparent;color:#888;
                        border:3px solid #2a2a4a;
                        box-shadow:2px 2px 0 #000;
                        cursor:not-allowed;
                    ", "Sold Out" }
                }
            }
            {detail_open().then(|| {
                let s = s5.clone();
                let add_price = price;
                let avail = is_available;
                let share_id = s.id.clone();
                let share_name = lang::localized(&s.name, s.name_en.as_deref());
                rsx! {
                    ProductDetailModal {
                        name: lang::localized(
                            &s.name, s.name_en.as_deref()),
                        image_url: s.image_url.clone(),
                        description: lang::localized(
                            s.description.as_deref().unwrap_or(""),
                            s.description_en.as_deref(),
                        ),
                        price_line: Some(price_str.clone()),
                        can_add: avail,
                        add_to_cart_label: Some(add_to_cart_label.clone()),
                        on_add_to_cart: move |q: u32| {
                            cart.write().add_item(CartItem {
                                id: s.id.clone(),
                                name: lang::localized(
                                    &s.name, s.name_en.as_deref()),
                                price: add_price,
                                quantity: q,
                                image_url: None,
                                item_type: CartItemType::Set,
                                fulfillment: None,
                            });
                            crate::ui::telegram::TelegramApp::init().haptic_notification(crate::ui::telegram::HapticNotification::Success);
                        },
                        on_share: Some(EventHandler::new(move |_| share_product(ProductKind::Set, &share_id, &share_name))),
                        on_close: move |_| detail_open.set(false),
                    }
                }
            })}
            {show_video().then(|| {
                let url = vid_url.clone();
                rsx! {
                    VideoModal { url, on_close: move |_| show_video.set(false) }
                }
            })}
        }
    }
}

fn is_media_url(u: &str) -> bool {
    u.starts_with("http://")
        || u.starts_with("https://")
        || (u.starts_with('/') && !u.starts_with("//"))
}
