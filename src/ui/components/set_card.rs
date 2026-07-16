//! Dedicated uniform-height set card for the Sets grid.
//!
//! This component replicates the original set card design: the image/video area
//! fills the top part of the card with `width:100%;height:100%;object-fit:cover`
//! (the same style used in the very first set card video preview in 5a2d97a),
//! badge stack, name/weight/price/button layout, and locks the whole card to a
//! fixed aspect ratio so every cell in the 2-column grid is exactly the same size.

use crate::trios::i18n::{t, T_ADD_TO_CART};
use crate::ui::lang;
use crate::ui::state::{Cart, CartItem, CartItemType};
use dioxus::prelude::*;

/// Public data needed to render a uniform set card.
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

/// Render a uniform-height set card.
///
/// `on_select` opens the product detail modal; `on_video` opens the video
/// modal.  Both are hoisted to the screen level so the card itself does not
/// hold per-item signals (which would break hook ordering when the grid is
/// re-sorted).
pub fn render_uniform_set_card(
    set: SetCardData,
    mut cart: Signal<Cart>,
    accent: &'static str,
    mut on_select: impl FnMut() + 'static,
    mut on_video: impl FnMut(String) + 'static,
) -> Element {
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
    let badge_label = badge
        .label()
        .map(|(ru, en)| lang::localized(ru, Some(en)));
    let badge_color = badge.color();
    let strain_word = lang::localized("сортов", Some("strains"));
    let weight_line = crate::trios::packs::weight_line(
        set.total_weight_grams,
        set.strain_count,
        &strain_word,
    );

    // Keep the same number of visual text lines on every card so the content
    // area is identical in size.  Use a non-breaking space when a slot has no
    // real text.
    let weight_text = if weight_line.is_empty() {
        "\u{00A0}".to_string()
    } else {
        format!("⚖️ {}", weight_line)
    };
    let desc_text = if desc.is_empty() {
        "\u{00A0}".to_string()
    } else {
        desc.clone()
    };

    let img_url = set.image_url.clone().unwrap_or_default();
    let has_image = is_media_url(&img_url);
    let vid_url = set.video_url.clone().unwrap_or_default();
    let has_video = is_media_url(&vid_url);

    // Lock the card to a fixed aspect ratio so every card in the 2-column grid
    // is the same height.  The media area takes 50% of the card.
    let card_style = format!(
        "background:#16213e;border:4px solid {};box-shadow:4px 4px 0 #000;overflow:hidden;position:relative;cursor:pointer;display:flex;flex-direction:column;height:100%;aspect-ratio:2/3;{}",
        accent, opacity
    );

    rsx! {
        div { class: "comet-card", style: card_style,
            onclick: move |_| on_select(),
            // Media area: exactly 50% of the card height, image/video fill it.
            div { style: "flex:0 0 50%;position:relative;overflow:hidden;background:linear-gradient(135deg,#1a1a2e,#16213e);display:flex;align-items:center;justify-content:center;",
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
                        onclick: move |e: Event<MouseData>| { e.stop_propagation(); on_video(vid_url.clone()); }
                    }
                } else if has_image {
                    img {
                        src: "{img_url}",
                        alt: "{set_name}",
                        loading: "lazy",
                        style: "width:100%;height:100%;object-fit:cover;display:block;"
                    }
                } else {
                    span { style: "font-size:48px;", "{icon}" }
                }
                // Badge stack (top-left), matching the original set card.
                div { style: "position:absolute;top:8px;left:8px;display:flex;flex-direction:column;gap:4px;z-index:4;align-items:flex-start;",
                    span { style: "font-size:13px;font-weight:700;background:{accent};color:#000;padding:4px 8px;box-shadow:2px 2px 0 #000;", "📦 SET" }
                    if let Some(bl) = badge_label.clone() {
                        span { style: "font-size:11px;font-weight:700;background:{badge_color};color:#fff;padding:3px 7px;box-shadow:2px 2px 0 #000;", "{bl}" }
                    }
                    if has_discount {
                        span { style: "font-size:13px;font-weight:700;background:{accent};color:#000;padding:4px 8px;box-shadow:2px 2px 0 #000;", "{discount_badge}" }
                    }
                }
            }
            // Content area: fills the remaining 50% of the card, text clamped.
            div { style: "flex:1 1 auto;padding:12px;display:flex;flex-direction:column;overflow:hidden;",
                div { style: "font-size:17px;font-weight:700;margin-bottom:6px;color:#fff;line-height:1.2;text-shadow:2px 2px 0 #000;display:-webkit-box;-webkit-line-clamp:2;-webkit-box-orient:vertical;overflow:hidden;",
                    "{set_name}"
                }
                div { style: "font-size:13px;color:#b388ff;font-weight:600;margin-bottom:4px;line-height:1.35;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;",
                    "{weight_text}"
                }
                div { style: "font-size:13px;color:#888;margin-bottom:8px;line-height:1.35;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;",
                    "{desc_text}"
                }
                div { style: "display:flex;gap:6px;align-items:baseline;margin-top:auto;",
                    span { style: "font-size:20px;font-weight:800;color:#ffe600;text-shadow:2px 2px 0 #000;", "{price_str}" }
                    if has_discount {
                        span { style: "font-size:13px;color:#888;text-decoration:line-through;margin-left:4px;", "{original_price_str}" }
                    }
                }
            }
            div { style: "padding:0 12px 12px;",
                if is_available {
                    button {
                        style: "
                            font-size:14px;font-weight:700;
                            width:100%;padding:12px 20px;
                            background:#39ff14;color:#000;
                            border:4px solid #2d9e0f;
                            box-shadow:3px 3px 0 #000;
                            cursor:pointer;
                        ",
                        onclick: move |e: Event<MouseData>| {
                            e.stop_propagation();
                            let s = set.clone();
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
                        "{add_to_cart_label} 🛒"
                    }
                } else {
                    button { style: "
                        font-size:14px;font-weight:600;
                        width:100%;padding:12px 20px;
                        background:transparent;color:#888;
                        border:4px solid #2a2a4a;
                        box-shadow:3px 3px 0 #000;
                        cursor:not-allowed;
                    ", "Sold Out" }
                }
            }
        }
    }
}

fn is_media_url(u: &str) -> bool {
    u.starts_with("http://")
        || u.starts_with("https://")
        || (u.starts_with('/') && !u.starts_with("//"))
}
