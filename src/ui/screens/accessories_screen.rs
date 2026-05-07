use dioxus::prelude::*;
use serde::Deserialize;
use crate::ui::state::{Cart, CartItem};
use crate::ui::components::bottom_nav::BottomNav;
use crate::ui::api::context::api_base_url;
use crate::trios::core::Lang;
use crate::trios::i18n::{t, T_ACC_TITLE, T_ACC_DESC, T_FILTER_ALL, T_ADD_TO_CART};

#[derive(Debug, Clone, Deserialize)]
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

const CATEGORIES: &[&str] = &[
    "All", "Grinder", "Papers", "Pipe", "Bong", "Storage", "Lighter", "Clothing", "Souvenir", "Other",
];

#[component]
pub fn AccessoriesScreen() -> Element {
    let mut cart = use_context::<Signal<Cart>>();
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
        div { style: "
            min-height: 100vh;
            background: #0f0f1a;
            color: #e8e8e8;
            font-family: 'Press Start 2P', monospace;
            padding-bottom: 80px;
        ",
            div { style: "padding: 20px 16px 12px; text-align: center;",
                h1 { style: "font-size: 18px; color: #00e5ff; text-shadow: 0 0 8px rgba(0,229,255,0.5);", "{acc_title}" }
                p { style: "font-size: 18px; color: #8b8b9e; margin-top: 4px;", "{acc_desc}" }
            }

            // Category filter tabs
            div { style: "display: flex; gap: 6px; padding: 0 16px 12px; overflow-x: auto;",
                for cat in CATEGORIES.iter() {
                    {
                        let is_active = active_category() == *cat;
                        let bg = if is_active { "#00e5ff" } else { "transparent" };
                        let color = if is_active { "#0f0f1a" } else { "#8b8b9e" };
                        let border = if is_active { "#00e5ff" } else { "#2a2a4a" };
                        let label = if *cat == "All" { filter_all.to_string() } else { cat.to_string() };
                        let cat_val = cat.to_string();
                        rsx! {
                            button {
                                style: "
                                    font-family: 'Press Start 2P', monospace;
                                    font-size: 10px; padding: 6px 10px;
                                    background: {bg}; color: {color};
                                    border: 2px solid {border}; border-radius: 8px;
                                    cursor: pointer; white-space: nowrap;
                                ",
                                onclick: move |_| active_category.set(cat_val.clone()),
                                "{label}"
                            }
                        }
                    }
                }
            }

            // Products grid
            {
                match &*accessories_resource.read() {
                    Some(Ok(accessories)) if accessories.is_empty() => rsx! {
                        div { style: "text-align: center; padding: 40px 16px;",
                            div { style: "font-size: 36px; margin-bottom: 12px;", "🛠️" }
                            p { style: "font-size: 12px; color: #8b8b9e;", "No accessories available yet" }
                        }
                    },
                    Some(Ok(_)) => rsx! {
                        div { style: "display: grid; grid-template-columns: 1fr 1fr; gap: 10px; padding: 0 16px;",
                            for accessory in filtered.iter() {
                                {
                                    let a = accessory.clone();
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
                                    let opacity = if show_out_of_stock { "0.6" } else { "1" };
                                    let desc = a.description.as_deref().unwrap_or("");

                                    rsx! {
                                        div { style: "
                                            background: #1a1a2e; border: 2px solid #2a2a4a;
                                            border-radius: 8px; overflow: hidden; box-shadow: 4px 4px 0 #000;
                                            opacity: {opacity};
                                        ",
                                            div { style: "
                                                height: 100px;
                                                background: linear-gradient(135deg, #1a1a2e, #1a1a2e);
                                                display: flex; align-items: center; justify-content: center;
                                                font-size: 36px; position: relative;
                                            ",
                                                if let Some(ref vid) = a.video_url {
                                                    video { src: "{vid}", style: "width: 100%; height: 100%; object-fit: cover;", autoplay: "true", muted: "true", r#loop: "true", playsinline: "true" }
                                                } else if let Some(ref img) = a.image_url {
                                                    if !img.is_empty() {
                                                        img { src: "{img}", alt: "{a_name}", style: "width: 100%; height: 100%; object-fit: cover;" }
                                                    } else {
                                                        "{emoji}"
                                                    }
                                                } else {
                                                    "{emoji}"
                                                }
                                                if show_out_of_stock {
                                                    span { style: "
                                                        position: absolute; bottom: 4px; left: 50%;
                                                        transform: translateX(-50%);
                                                        font-size: 9px; background: #ff4757; color: white;
                                                        padding: 1px 6px; border-radius: 3px;
                                                    ", "SOLD OUT" }
                                                }
                                            }
                                            div { style: "padding: 8px;",
                                                div { style: "font-size: 18px; color: #00e5ff; margin-bottom: 2px; text-transform: uppercase;", "{cat}" }
                                                div { style: "font-size: 14px; font-weight: bold; margin-bottom: 2px;", "{a_name}" }
                                                if !desc.is_empty() {
                                                    div { style: "font-size: 18px; color: #8b8b9e; margin-bottom: 4px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;", "{desc}" }
                                                }
                                                if show_low_stock && !show_out_of_stock {
                                                    div { style: "font-size: 9px; color: #ffe600; margin-bottom: 4px;", "⚠ Only {stock} left" }
                                                }
                                                div { style: "font-size: 16px; color: #39ff14; margin-bottom: 6px;", "{price_str}" }
                                            }
                                            div { style: "padding: 0 8px 8px;",
                                                if show_out_of_stock {
                                                    button { style: "
                                                        font-family: 'Press Start 2P', monospace;
                                                        font-size: 10px; width: 100%; padding: 6px;
                                                        background: transparent; color: #8b8b9e;
                                                        border: 2px solid #2a2a4a; border-radius: 8px; cursor: not-allowed;
                                                    ", "Sold Out" }
                                                } else {
                                                    button {
                                                        style: "
                                                            font-family: 'Press Start 2P', monospace;
                                                            font-size: 10px; width: 100%; padding: 6px;
                                                            background: #39ff14; color: #0f0f1a;
                                                            border: none; border-radius: 8px; cursor: pointer;
                                                        ",
                                                        onclick: move |_| {
                                                            let mut c = cart.write();
                                                            c.add_item(CartItem {
                                                                id: a_id.clone(),
                                                                name: a_name.clone(),
                                                                price: a_price,
                                                                quantity: 1,
                                                                image_url: None,
                                                            });
                                                        },
                                                        "{add_to_cart}"
                                                    }
                                                }
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
                        div { style: "display: grid; grid-template-columns: 1fr 1fr; gap: 10px; padding: 0 16px;",
                            for _ in 0..4 {
                                div { style: "
                                    background: #1a1a2e; border: 2px solid #2a2a4a;
                                    border-radius: 8px; padding: 20px; text-align: center;
                                    box-shadow: 4px 4px 0 #000; min-height: 140px;
                                ",
                                    div { style: "font-size: 12px; color: #8b8b9e;", "Loading..." }
                                }
                            }
                        }
                    },
                }
            }

            BottomNav {}
        }
    }
}
