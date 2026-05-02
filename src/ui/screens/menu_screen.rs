// Menu Screen — Connected to real API data
use dioxus::prelude::*;
use serde::Deserialize;
use crate::ui::routes::Route;
use crate::ui::state::{Cart, CartItem};

// ── API response types (matching backend JSON exactly) ─────────

#[derive(Debug, Clone, Deserialize, PartialEq)]
struct ApiStrain {
    id: String,
    name: String,
    category: Option<String>,
    thc_percent: Option<f64>,
    cbd_percent: Option<f64>,
    effect: Option<String>,
    flavor_profile: Option<String>,
    description: Option<String>,
    price_per_gram: f64,
    available_grams: Option<f64>,
    image_url: Option<String>,
    is_available: bool,
    is_strain_of_day: bool,
    strain_of_day_discount: f64,
}

#[derive(Debug, Deserialize)]
struct StrainsResponse {
    strains: Vec<ApiStrain>,
}

// ── Helpers ────────────────────────────────────────────────────

fn api_base_url() -> String {
    web_sys::window()
        .and_then(|w| w.location().origin().ok())
        .map(|origin| {
            if origin.contains(":8080") || origin.contains(":3001") {
                "http://localhost:3000".to_string()
            } else {
                origin
            }
        })
        .unwrap_or_else(|| "http://localhost:3000".to_string())
}

fn category_emoji(cat: &str) -> &'static str {
    match cat {
        "Sativa" => "☀️",
        "Indica" => "🌙",
        "Hybrid" => "⚖️",
        _ => "🌿",
    }
}

fn category_badge_style(cat: &str) -> String {
    let (color, bg) = match cat {
        "Sativa" => ("#39ff14", "rgba(57,255,20,0.1)"),
        "Indica" => ("#00e5ff", "rgba(0,229,255,0.1)"),
        "Hybrid" => ("#ffe600", "rgba(255,230,0,0.1)"),
        _ => ("#8b8b9e", "rgba(139,139,158,0.1)"),
    };
    format!(
        "font-size:5px;color:{};border:1px solid {};background:{};padding:1px 4px;border-radius:3px;",
        color, color, bg
    )
}

fn format_price(price: f64) -> String {
    format!("฿{}", price as i32)
}

fn filter_tab_style(is_active: bool) -> String {
    if is_active {
        "font-family:'Press Start 2P',monospace;font-size:6px;padding:6px 10px;background:#39ff14;color:#0f0f1a;border:2px solid #39ff14;border-radius:4px;cursor:pointer;white-space:nowrap;".to_string()
    } else {
        "font-family:'Press Start 2P',monospace;font-size:6px;padding:6px 10px;background:transparent;color:#8b8b9e;border:2px solid #2a2a4a;border-radius:4px;cursor:pointer;white-space:nowrap;".to_string()
    }
}

// ── Menu Screen Component ──────────────────────────────────────

#[component]
pub fn MenuScreen() -> Element {
    let mut active_filter = use_signal(|| "All".to_string());

    let cart = use_context::<Signal<Cart>>();
    let cart_count: u32 = cart.read().items.iter().map(|i| i.quantity).sum();

    let strains_resource = use_resource(|| async move {
        let base = api_base_url();
        let url = format!("{}/api/strains", base);
        reqwest::Client::new()
            .get(&url)
            .send()
            .await
            .map_err(|e| e.to_string())?
            .json::<StrainsResponse>()
            .await
            .map(|r| r.strains)
            .map_err(|e| e.to_string())
    });

    rsx! {
        div { style: "
            min-height: 100vh;
            background: #0f0f1a;
            color: #e8e8e8;
            font-family: 'Press Start 2P', monospace;
            padding-bottom: 80px;
        ",
            // Header with cart counter
            div { style: "padding: 20px 16px 12px; text-align: center; position: relative;",
                h1 { style: "font-size: 12px; color: #39ff14; text-shadow: 0 0 8px rgba(57,255,20,0.5);", "🌿 Menu" }
                p { style: "font-size: 7px; color: #8b8b9e; margin-top: 4px;", "Browse our premium selection" }
                if cart_count > 0 {
                    Link { to: Route::Cart {},
                        div { style: "
                            position: absolute; top: 20px; right: 16px;
                            background: #39ff14; color: #0f0f1a;
                            font-size: 7px; padding: 4px 8px;
                            border-radius: 10px; cursor: pointer;
                            font-family: 'Press Start 2P', monospace;
                        ",
                            "🛒 {cart_count}"
                        }
                    }
                }
            }

            // Filter tabs
            div { style: "display: flex; gap: 6px; padding: 0 16px 12px; overflow-x: auto;",
                button {
                    style: filter_tab_style(active_filter() == "All"),
                    onclick: move |_| active_filter.set("All".to_string()),
                    "All"
                }
                button {
                    style: filter_tab_style(active_filter() == "Sativa"),
                    onclick: move |_| active_filter.set("Sativa".to_string()),
                    "☀️ Sativa"
                }
                button {
                    style: filter_tab_style(active_filter() == "Indica"),
                    onclick: move |_| active_filter.set("Indica".to_string()),
                    "🌙 Indica"
                }
                button {
                    style: filter_tab_style(active_filter() == "Hybrid"),
                    onclick: move |_| active_filter.set("Hybrid".to_string()),
                    "⚖️ Hybrid"
                }
            }

            // Content area
            {
                match &*strains_resource.read() {
                    Some(Ok(all_strains)) => {
                        let filter_val = active_filter();
                        let filtered: Vec<ApiStrain> = if filter_val == "All" {
                            all_strains.clone()
                        } else {
                            all_strains.iter()
                                .filter(|s| s.category.as_deref() == Some(filter_val.as_str()))
                                .cloned()
                                .collect()
                        };

                        if filtered.is_empty() {
                            let f = filter_val.clone();
                            rsx! {
                                div { style: "text-align: center; padding: 40px 16px;",
                                    p { style: "font-size: 20px; margin-bottom: 12px;", "🔍" }
                                    p { style: "font-size: 8px; color: #8b8b9e;", "No {f} strains found" }
                                }
                            }
                        } else {
                            rsx! {
                                div { style: "display: grid; grid-template-columns: 1fr 1fr; gap: 10px; padding: 0 16px;",
                                    {filtered.into_iter().map(|strain| render_strain_card(strain, cart))}
                                }
                            }
                        }
                    },
                    Some(Err(e)) => {
                        let err_msg = e.clone();
                        rsx! {
                            div { style: "text-align: center; padding: 40px 16px;",
                                p { style: "font-size: 20px; margin-bottom: 12px;", "⚠️" }
                                p { style: "font-size: 8px; color: #ff4757;", "Error loading strains" }
                                p { style: "font-size: 6px; color: #8b8b9e; margin-top: 8px; word-break: break-all;", "{err_msg}" }
                            }
                        }
                    },
                    None => {
                        rsx! {
                            div { style: "text-align: center; padding: 40px 16px;",
                                p { style: "font-size: 24px;", "🌿" }
                                p { style: "font-size: 8px; color: #8b8b9e; margin-top: 12px;", "Loading strains..." }
                            }
                        }
                    },
                }
            }

            // Bottom Navigation
            nav { style: "
                position: fixed;
                bottom: 0;
                left: 0;
                right: 0;
                background: #1a1a2e;
                border-top: 2px solid #2a2a4a;
                display: flex;
                justify-content: space-around;
                padding: 10px 0;
                z-index: 100;
            ",
                Link { to: Route::Home {},
                    div { style: "text-align: center; cursor: pointer;",
                        div { style: "font-size: 20px;", "🏠" }
                        div { style: "font-size: 6px; color: #8b8b9e; margin-top: 2px;", "Home" }
                    }
                }
                Link { to: Route::Menu {},
                    div { style: "text-align: center; cursor: pointer;",
                        div { style: "font-size: 20px;", "🌿" }
                        div { style: "font-size: 6px; color: #39ff14; margin-top: 2px;", "Menu" }
                    }
                }
                Link { to: Route::Cart {},
                    div { style: "text-align: center; cursor: pointer;",
                        div { style: "font-size: 20px;", "🛒" }
                        div { style: "font-size: 6px; color: #8b8b9e; margin-top: 2px;", "Cart" }
                    }
                }
                Link { to: Route::Orders {},
                    div { style: "text-align: center; cursor: pointer;",
                        div { style: "font-size: 20px;", "📋" }
                        div { style: "font-size: 6px; color: #8b8b9e; margin-top: 2px;", "Orders" }
                    }
                }
                Link { to: Route::Profile {},
                    div { style: "text-align: center; cursor: pointer;",
                        div { style: "font-size: 20px;", "👤" }
                        div { style: "font-size: 6px; color: #8b8b9e; margin-top: 2px;", "Profile" }
                    }
                }
            }
        }
    }
}

// ── Strain Card Renderer ───────────────────────────────────────

fn render_strain_card(strain: ApiStrain, mut cart: Signal<Cart>) -> Element {
    let cat = strain.category.as_deref().unwrap_or("Hybrid");
    let emoji = category_emoji(cat);
    let is_sotd = strain.is_strain_of_day;
    let discount = strain.strain_of_day_discount;
    let has_discount = is_sotd && discount > 0.0;

    let border_color = if is_sotd { "#ffe600" } else { "#2a2a4a" };
    let card_style = format!(
        "background:#16213e;border:2px solid {};border-radius:8px;overflow:hidden;box-shadow:4px 4px 0 #000;position:relative;{}",
        border_color,
        if strain.is_available { "".to_string() } else { "opacity:0.6;".to_string() }
    );

    let display_price = if has_discount {
        let discounted = strain.price_per_gram * (1.0 - discount / 100.0);
        format_price(discounted)
    } else {
        format_price(strain.price_per_gram)
    };

    let original_price = format_price(strain.price_per_gram);

    let thc_str = strain.thc_percent
        .map(|t| format!("THC: {}%", t as i32))
        .unwrap_or_default();

    let desc: String = strain.description
        .as_deref()
        .unwrap_or("")
        .chars()
        .take(32)
        .collect();

    let badge_style = category_badge_style(cat);
    let badge_label = format!("{} {}", emoji, cat);

    rsx! {
        div { key: strain.id.clone(), style: card_style,
            // Image area
            div { style: "height:80px;background:linear-gradient(135deg,#1a1a2e,#16213e);display:flex;align-items:center;justify-content:center;font-size:36px;position:relative;",
                "{emoji}"
                {is_sotd.then(|| rsx! {
                    span { style: "
                        position:absolute;top:4px;right:4px;
                        font-size:5px;background:#ffe600;color:#000;
                        padding:2px 4px;border-radius:3px;
                    ", "⭐ SOTD" }
                })}
            }
            // Content
            div { style: "padding:8px;",
                div { style: "font-size:9px;font-weight:bold;margin-bottom:4px;",
                    "{strain.name}"
                }
                div { style: "display:flex;gap:4px;align-items:center;margin-bottom:4px;",
                    span { style: badge_style, "{badge_label}" }
                    {(!thc_str.is_empty()).then(|| rsx! {
                        span { style: "font-size:7px;color:#8b8b9e;", "{thc_str}" }
                    })}
                }
                {(!desc.is_empty()).then(|| rsx! {
                    div { style: "font-size:7px;color:#8b8b9e;margin-bottom:6px;",
                        "{desc}"
                    }
                })}
                div { style: "display:flex;gap:4px;align-items:center;margin-bottom:6px;",
                    span { style: "font-size:9px;color:#39ff14;", "{display_price}" }
                    {has_discount.then(|| rsx! {
                        span { style: "font-size:7px;color:#8b8b9e;text-decoration:line-through;",
                            "{original_price}"
                        }
                    })}
                }
            }
            // Action button
            div { style: "padding:0 8px 8px;",
                {if strain.is_available {
                    let unit_price = if has_discount {
                        strain.price_per_gram * (1.0 - discount / 100.0)
                    } else {
                        strain.price_per_gram
                    };
                    let s_id = strain.id.clone();
                    let s_name = strain.name.clone();
                    rsx! {
                        button {
                            style: "
                                font-family:'Press Start 2P',monospace;
                                font-size:6px;width:100%;padding:6px;
                                background:#39ff14;color:#0f0f1a;
                                border:none;border-radius:4px;cursor:pointer;
                            ",
                            onclick: move |_| {
                                cart.write().add_item(CartItem {
                                    id: s_id.clone(),
                                    name: s_name.clone(),
                                    price: unit_price,
                                    quantity: 1,
                                    image_url: None,
                                });
                            },
                            "Add to Cart 🛒"
                        }
                    }
                } else {
                    rsx! {
                        button { style: "
                            font-family:'Press Start 2P',monospace;
                            font-size:6px;width:100%;padding:6px;
                            background:transparent;color:#8b8b9e;
                            border:2px solid #2a2a4a;border-radius:4px;cursor:not-allowed;
                        ", "Sold Out" }
                    }
                }}
            }
        }
    }
}
