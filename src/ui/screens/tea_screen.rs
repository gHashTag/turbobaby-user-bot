use crate::trios::i18n::{t, T_ADD_TO_CART, T_FILTER_ALL, T_TEA_DESC, T_TEA_TITLE};
use crate::ui::api::context::api_base_url;
use crate::ui::components::bottom_nav::BottomNav;
use crate::ui::components::product_detail_modal::ProductDetailModal;
use crate::ui::state::{Cart, CartItem, CartItemType};
use dioxus::prelude::*;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
struct ApiTea {
    id: String,
    name: String,
    subcategory: Option<String>,
    description: Option<String>,
    price: f64,
    stock: Option<i32>,
    image_url: Option<String>,
    video_url: Option<String>,
    is_available: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct TeaResponse {
    products: Vec<ApiTea>,
}

fn subcategory_emoji(sub: &str) -> &'static str {
    match sub.to_lowercase().as_str() {
        "tea" | "cbd tea" => "🍵",
        "dessert" | "fruit" => "🥭",
        "teaware" => "🫖",
        "herbal" => "🍃",
        "flower" => "💜",
        _ => "🍵",
    }
}

const SUBCATEGORIES: &[&str] = &["All", "Tea", "Dessert", "Teaware"];

#[component]
pub fn TeaScreen() -> Element {
    let mut cart = use_context::<Signal<Cart>>();
    let mut active_sub = use_signal(|| "All".to_string());
    // Tapping a tea card opens a detail popup with the full description.
    // Held at screen level (one modal) because the cards render inline in a
    // `for` loop, where a per-card `use_signal` would violate hook ordering.
    let mut selected_tea = use_signal(|| None::<ApiTea>);

    let tea_title = t(crate::ui::lang::current_lang(), T_TEA_TITLE);
    let tea_desc = t(crate::ui::lang::current_lang(), T_TEA_DESC);
    let filter_all = t(crate::ui::lang::current_lang(), T_FILTER_ALL);
    let add_to_cart = t(crate::ui::lang::current_lang(), T_ADD_TO_CART);

    let tea_resource = use_resource(|| async move {
        let base = api_base_url();
        let url = format!("{}/api/tea-products", base);
        crate::ui::api::local_client::LocalClient::new()
            .get(&url)
            .send()
            .await
            .map_err(|e| e.to_string())?
            .json::<TeaResponse>()
            .await
            .map(|r| r.products)
            .map_err(|e| e.to_string())
    });

    let filtered = match &*tea_resource.read() {
        Some(Ok(teas)) => {
            let sub = active_sub();
            if sub == "All" {
                teas.clone()
            } else {
                teas.iter()
                    .filter(|t| {
                        t.subcategory
                            .as_deref()
                            .unwrap_or("")
                            .eq_ignore_ascii_case(&sub)
                    })
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
            padding-bottom: 80px;
        ",
            div { style: "padding: 20px 16px 16px; text-align: center;",
                h1 { style: "font-size: 24px; font-weight: 800; color: #39ff14; text-shadow: 3px 3px 0 #000, 0 0 10px rgba(57,255,20,0.5); letter-spacing: 2px;", "{tea_title}" }
                p { style: "font-size: 13px; color: #8b8b9e; margin-top: 4px;", "{tea_desc}" }
            }

            // Subcategory filter tabs (pill chips)
            div { style: "display: flex; gap: 6px; padding: 0 16px 12px; overflow-x: auto;",
                for sub in SUBCATEGORIES.iter() {
                    {
                        let is_active = active_sub() == *sub;
                        let bg = if is_active { "#39ff14" } else { "transparent" };
                        let color = if is_active { "#000" } else { "#8b8b9e" };
                        let border = if is_active { "#39ff14" } else { "#2a2a4a" };
                        let label = if *sub == "All" { filter_all.to_string() } else { sub.to_string() };
                        let sub_val = sub.to_string();
                        rsx! {
                            button {
                                style: "
                                    font-size: 13px; padding: 6px 10px;
                                    background: {bg}; color: {color};
                                    border: 4px solid {border}; border-radius: 20px;
                                    cursor: pointer; white-space: nowrap;
                                ",
                                onclick: move |_| active_sub.set(sub_val.clone()),
                                "{label}"
                            }
                        }
                    }
                }
            }

            // Tea products grid
            {
                match &*tea_resource.read() {
                    Some(Ok(teas)) if teas.is_empty() => rsx! {
                        div { style: "text-align: center; padding: 40px 16px;",
                            div { style: "font-size: 70px; margin-bottom: 12px;", "🍵" }
                            p { style: "font-size: 13px; color: #8b8b9e;", "No tea products available yet" }
                        }
                    },
                    Some(Ok(_)) => rsx! {
                        div { style: "display: grid; grid-template-columns: 1fr 1fr; gap: 10px; padding: 0 16px;",
                            for tea in filtered.iter() {
                                {
                                    let t = tea.clone();
                                    let sub = t.subcategory.as_deref().unwrap_or("tea");
                                    let emoji = subcategory_emoji(sub);
                                    let is_available = t.is_available.unwrap_or(true);
                                    let stock = t.stock.unwrap_or(999).max(0);
                                    let show_low_stock = stock > 0 && stock <= 5;
                                    let show_out_of_stock = stock == 0 || !is_available;
                                    let t_price = if t.price.is_finite() { t.price.max(0.0) } else { 0.0 };
                                    let price_str = format!("฿{}", t_price as i32);
                                    let t_name = t.name.clone();
                                    let t_id = t.id.clone();
                                    let opacity = if show_out_of_stock { "0.6" } else { "1" };
                                    let desc = t.description.as_deref().unwrap_or("");
                                    let t_click = t.clone();

                                    rsx! {
                                        div { style: "
                                            background: #16213e; border: 4px solid #2a2a4a;
                                            border-radius: 0; overflow: hidden; box-shadow: 4px 4px 0 #000;
                                            opacity: {opacity}; cursor: pointer;
                                        ",
                                            onclick: move |_| selected_tea.set(Some(t_click.clone())),
                                            div { style: "
                                                height: 100px;
                                                background: linear-gradient(135deg, #16213e, #16213e);
                                                display: flex; align-items: center; justify-content: center;
                                                font-size: 36px; position: relative;
                                            ",
                                                if let Some(ref img) = t.image_url {
                                                    if !img.is_empty() && (img.starts_with("http://") || img.starts_with("https://") || (img.starts_with("/") && !img.starts_with("//"))) {
                                                        img { src: "{img}", alt: "{t_name}", style: "width: 100%; height: 100%; object-fit: cover;" }
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
                                                        font-size: 13px; background: #ff4757; color: white;
                                                        padding: 1px 6px; border-radius: 0;
                                                    ", "SOLD OUT" }
                                                }
                                                {if let Some(ref vid) = t.video_url {
                                                    if !vid.is_empty() && (vid.starts_with("http://") || vid.starts_with("https://") || (vid.starts_with("/") && !vid.starts_with("//"))) {
                                                        let vid = vid.clone();
                                                        rsx! {
                                                            a { href: "{vid}", target: "_blank", style: "position:absolute;bottom:4px;right:4px;width:28px;height:28px;border-radius:50%;background:rgba(0,0,0,0.6);border:1px solid #fff;color:#fff;font-size:12px;display:flex;align-items:center;justify-content:center;cursor:pointer;z-index:2;text-decoration:none;",
                                                                onclick: move |e: Event<MouseData>| { e.stop_propagation(); }, "▶️" }
                                                        }
                                                    } else { rsx!{ "" } }
                                                } else { rsx!{ "" } }}
                                            }
                                            div { style: "padding: 8px;",
                                                div { style: "font-size: 13px; font-weight: 700; color: #b388ff; margin-bottom: 2px; text-transform: uppercase;", "{sub}" }
                                                div { style: "font-size: 17px; font-weight: 700; margin-bottom: 2px;", "{t_name}" }
                                                if !desc.is_empty() {
                                                    div { style: "font-size: 13px; color: #8b8b9e; margin-bottom: 4px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;", "{desc}" }
                                                }
                                                if show_low_stock && !show_out_of_stock {
                                                    div { style: "font-size: 13px; color: #ffe600; margin-bottom: 4px;", "⚠ Only {stock} left" }
                                                }
                                                div { style: "font-size: 20px; font-weight: 800; color: #ffe600; text-shadow: 2px 2px 0 #000; margin-bottom: 6px;", "{price_str}" }
                                            }
                                            div { style: "padding: 0 8px 8px;",
                                                if show_out_of_stock {
                                                    button { style: "
                                                        font-size: 13px; width: 100%; padding: 8px;
                                                        background: transparent; color: #8b8b9e;
                                                        border: 4px solid #2a2a4a; border-radius: 0; cursor: not-allowed;
                                                    ", "Sold Out" }
                                                } else {
                                                    button {
                                                        style: "
                                                            font-size: 14px; font-weight: 700; width: 100%; padding: 8px;
                                                            background: #39ff14; color: #000;
                                                            border: 4px solid #2d9e0f; border-radius: 0; cursor: pointer;
                                                            box-shadow: 3px 3px 0 #000;
                                                            transition: transform 0.1s, box-shadow 0.1s;
                                                        ",
                                                        onclick: move |e: Event<MouseData>| {
                                                            e.stop_propagation();
                                                            let mut c = cart.write();
                                                            c.add_item(CartItem {
                                                                id: t_id.clone(),
                                                                name: t_name.clone(),
                                                                price: t_price,
                                                                quantity: 1,
                                                                image_url: None,
                                                                item_type: CartItemType::Tea,
                                                            });
                                                            crate::ui::telegram::TelegramApp::init().haptic_notification(crate::ui::telegram::HapticNotification::Success);
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
                            div { style: "font-size: 70px; margin-bottom: 12px;", "⚠️" }
                            p { style: "font-size: 13px; color: #ff4757;", "Error: {e}" }
                        }
                    },
                    None => rsx! {
                        div { style: "display: grid; grid-template-columns: 1fr 1fr; gap: 10px; padding: 0 16px;",
                            for _ in 0..4 {
                                div { style: "
                                    background: #16213e; border: 4px solid #2a2a4a;
                                    border-radius: 0; padding: 20px; text-align: center;
                                    box-shadow: 4px 4px 0 #000; min-height: 140px;
                                ",
                                    div { style: "font-size: 13px; color: #8b8b9e;", "Loading..." }
                                }
                            }
                        }
                    },
                }
            }

            {selected_tea().map(|tea| {
                let sub = tea.subcategory.as_deref().unwrap_or("tea").to_uppercase();
                let price_str = format!("฿{}", (if tea.price.is_finite() { tea.price.max(0.0) } else { 0.0 }) as i32);
                rsx! {
                    ProductDetailModal {
                        name: tea.name.clone(),
                        image_url: tea.image_url.clone(),
                        description: tea.description.clone().unwrap_or_default(),
                        category_badge: Some(sub),
                        price_line: Some(price_str),
                        on_close: move |_| selected_tea.set(None),
                    }
                }
            })}

            BottomNav {}
        }
    }
}
