use dioxus::prelude::*;
use serde::Deserialize;
use crate::ui::state::{Cart, CartItem};
use crate::ui::components::bottom_nav::BottomNav;
use crate::ui::api::context::api_base_url;
use crate::trios::core::Lang;
use crate::trios::i18n::{t, T_ACC_TITLE, T_ACC_DESC, T_FILTER_ALL, T_ADD_TO_CART};

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
struct ApiAccessory {
    id: String,
    name: String,
    category: Option<String>,
    description: Option<String>,
    price: f64,
    stock: Option<i32>,
    image_url: Option<String>,
    video_url: Option<String>,
    is_available: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct AccessoriesResponse {
    accessories: Vec<ApiAccessory>,
}

fn category_emoji(cat: &str) -> &'static str {
    match cat.to_lowercase().as_str() {
        "grinder" => "⚙️",
        "papers" | "rolling" => "📜",
        "pipe" => "🪈",
        "bong" => "💎",
        "storage" => "📦",
        "lighter" => "🔥",
        "clothing" => "👕",
        "souvenir" => "🎭",
        "vaporizer" => "💨",
        _ => "🛠️",
    }
}

fn category_badge_style(cat: &str) -> String {
    let (color, bg) = match cat.to_lowercase().as_str() {
        "grinder" => ("#00e5ff", "rgba(0,229,255,0.15)"),
        "papers" | "rolling" => ("#ffe600", "rgba(255,230,0,0.15)"),
        "pipe" => ("#b388ff", "rgba(179,136,255,0.15)"),
        _ => ("#888", "rgba(136,136,136,0.1)"),
    };
    format!(
        "font-size:13px;color:{};border:2px solid {};background:{};padding:2px 8px;",
        color, color, bg
    )
}

const CATEGORIES: &[&str] = &[
    "All", "Grinder", "Papers", "Pipe", "Bong", "Storage", "Lighter", "Clothing", "Souvenir", "Other",
];

#[component]
pub fn AccessoriesScreen() -> Element {
    let cart = use_context::<Signal<Cart>>();
    let mut active_category = use_signal(|| "All".to_string());

    let acc_title = t(Lang::Russian, T_ACC_TITLE);
    let acc_desc = t(Lang::Russian, T_ACC_DESC);
    let filter_all = t(Lang::Russian, T_FILTER_ALL);
    let add_to_cart = t(Lang::Russian, T_ADD_TO_CART);

    let accessories_resource = use_resource(|| async move {
        let base = api_base_url();
        let url = format!("{}/api/accessories", base);
        reqwest::Client::new()
            .get(&url)
            .send()
            .await
            .map_err(|e| e.to_string())?
            .json::<AccessoriesResponse>()
            .await
            .map(|r| r.accessories)
            .map_err(|e| e.to_string())
    });

    let filtered = match &*accessories_resource.read() {
        Some(Ok(accessories)) => {
            let cat = active_category();
            if cat == "All" {
                accessories.clone()
            } else {
                accessories.iter()
                    .filter(|a| a.category.as_deref().unwrap_or("").eq_ignore_ascii_case(&cat))
                    .cloned()
                    .collect()
            }
        }
        _ => Vec::new(),
    };

    rsx! {
        div { style: "min-height:100vh;background:#0f0f1a;color:#e8e8e8;padding-bottom:80px;",
            div { style: "padding:20px 16px 16px;text-align:center;",
                h1 { style: "font-size:24px;font-weight:800;color:#00e5ff;text-shadow:3px 3px 0 #000,0 0 10px rgba(0,229,255,0.5);letter-spacing:2px;", "{acc_title}" }
                p { style: "font-size:13px;color:#888;margin-top:4px;", "{acc_desc}" }
            }

            div { style: "display:flex;gap:6px;padding:0 16px 12px;overflow-x:auto;",
                for cat in CATEGORIES.iter() {
                    {
                        let is_active = active_category() == *cat;
                        let bg = if is_active { "#00e5ff" } else { "transparent" };
                        let color = if is_active { "#000" } else { "#888" };
                        let border = if is_active { "#00e5ff" } else { "#2a2a4a" };
                        let label = if *cat == "All" { filter_all.to_string() } else { cat.to_string() };
                        let cat_val = cat.to_string();
                        rsx! {
                            button {
                                style: "
                                    font-size:12px;font-weight:600;padding:8px 16px;
                                    background:{bg};color:{color};
                                    border:3px solid {border};border-radius:20px;
                                    cursor:pointer;white-space:nowrap;
                                ",
                                onclick: move |_| active_category.set(cat_val.clone()),
                                "{label}"
                            }
                        }
                    }
                }
            }

            {
                match &*accessories_resource.read() {
                    Some(Ok(accessories)) if accessories.is_empty() => rsx! {
                        div { style: "text-align:center;padding:48px 16px;",
                            div { style: "font-size:70px;margin-bottom:12px;", "🛠️" }
                            p { style: "font-size:15px;color:#888;", "No accessories available yet" }
                        }
                    },
                    Some(Ok(_)) => rsx! {
                        div { style: "display:grid;grid-template-columns:1fr 1fr;gap:12px;padding:0 16px;",
                            for accessory in filtered.iter() {
                                {render_accessory_card(accessory.clone(), cart, add_to_cart)}
                            }
                        }
                    },
                    Some(Err(e)) => rsx! {
                        div { style: "text-align:center;padding:48px 16px;",
                            div { style: "font-size:70px;margin-bottom:12px;", "⚠️" }
                            p { style: "font-size:15px;color:#ff4757;", "Error: {e}" }
                        }
                    },
                    None => rsx! {
                        div { style: "text-align:center;padding:48px 16px;",
                            p { style: "font-size:24px;", "🛠️" }
                            p { style: "font-size:15px;color:#888;margin-top:12px;", "Loading..." }
                        }
                    },
                }
            }

            BottomNav {}
        }
    }
}

fn render_accessory_card(a: ApiAccessory, mut cart: Signal<Cart>, add_to_cart: &'static str) -> Element {
    let cat = a.category.as_deref().unwrap_or("other");
    let emoji = category_emoji(cat);
    let is_available = a.is_available.unwrap_or(true);
    let stock = a.stock.unwrap_or(999);
    let show_low_stock = stock > 0 && stock <= 5;
    let show_out_of_stock = stock == 0 || !is_available;
    let price_str = format!("฿{}", a.price as i32);
    let a_name = a.name.clone();
    let a_id = a.id.clone();
    let a_price = a.price;
    let opacity = if show_out_of_stock { "opacity:0.6;" } else { "" };
    let desc = a.description.as_deref().unwrap_or("");
    let img_url = a.image_url.clone().unwrap_or_default();
    let has_image = !img_url.is_empty();
    let badge_style = category_badge_style(cat);
    let badge_label = format!("{} {}", emoji, cat);

    let card_style = format!(
        "background:#16213e;border:4px solid #2a2a4a;box-shadow:4px 4px 0 #000;overflow:hidden;position:relative;{}",
        opacity
    );

    rsx! {
        div { key: a.id.clone(), class: "comet-card", style: card_style,
            div { style: "width:100%;aspect-ratio:2/3;background:linear-gradient(135deg,#1a1a2e,#16213e);display:flex;align-items:center;justify-content:center;position:relative;overflow:hidden;",
                {if has_image {
                    rsx! {
                        img {
                            src: "{img_url}",
                            alt: "{a_name}",
                            loading: "lazy",
                            style: "width:100%;height:100%;object-fit:cover;display:block;"
                        }
                    }
                } else {
                    rsx! {
                        span { style: "font-size:48px;", "{emoji}" }
                    }
                }}
                {show_out_of_stock.then(|| rsx! {
                    span { style: "
                        position:absolute;top:8px;left:8px;
                        font-size:13px;font-weight:700;background:#ff4757;color:#fff;
                        padding:4px 8px;box-shadow:2px 2px 0 #000;z-index:2;
                    ", "SOLD OUT" }
                })}
            }
            div { style: "padding:12px;",
                div { style: "font-size:17px;font-weight:700;margin-bottom:6px;color:#fff;line-height:1.2;text-shadow:2px 2px 0 #000;",
                    "{a_name}"
                }
                div { style: "display:flex;gap:6px;align-items:center;margin-bottom:6px;flex-wrap:wrap;",
                    span { style: badge_style, "{badge_label}" }
                }
                if !desc.is_empty() {
                    div { style: "font-size:13px;color:#888;margin-bottom:4px;line-height:1.35;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;",
                        "{desc}"
                    }
                }
                if show_low_stock && !show_out_of_stock {
                    div { style: "font-size:13px;color:#ffe600;margin-bottom:4px;", "⚠ Only {stock} left" }
                }
                div { style: "display:flex;gap:6px;align-items:baseline;margin-bottom:6px;",
                    span { style: "font-size:20px;font-weight:800;color:#ffe600;text-shadow:2px 2px 0 #000;", "{price_str}" }
                }
            }
            div { style: "padding:0 12px 12px;",
                {if !show_out_of_stock {
                    rsx! {
                        button {
                            style: "
                                font-size:14px;font-weight:700;
                                width:100%;padding:12px 20px;
                                background:#39ff14;color:#000;
                                border:4px solid #2d9e0f;
                                box-shadow:3px 3px 0 #000;
                                cursor:pointer;
                            ",
                            onclick: move |_| {
                                cart.write().add_item(CartItem {
                                    id: a_id.clone(),
                                    name: a_name.clone(),
                                    price: a_price,
                                    quantity: 1,
                                    image_url: None,
                                    item_type: CartItemType::Accessory,
                                });
                                crate::ui::telegram::TelegramApp::init().haptic_notification(crate::ui::telegram::HapticNotification::Success);
                            },
                            "{add_to_cart} 🛒"
                        }
                    }
                } else {
                    rsx! {
                        button { style: "
                            font-size:14px;font-weight:600;
                            width:100%;padding:12px 20px;
                            background:transparent;color:#888;
                            border:4px solid #2a2a4a;
                            box-shadow:3px 3px 0 #000;
                            cursor:not-allowed;
                        ", "Sold Out" }
                    }
                }}
            }
        }
    }
}
