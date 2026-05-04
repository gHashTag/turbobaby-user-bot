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
                // Local dev: use production API (local backend can't compile on macOS due to mio crate)
                "https://woody-weed-bot-production.up.railway.app".to_string()
            } else {
                origin
            }
        })
        .unwrap_or_else(|| "https://woody-weed-bot-production.up.railway.app".to_string())
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
    let mut active_sort = use_signal(|| "default".to_string());

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

            // Sort pills
            div { style: "display:flex;gap:6px;padding:0 16px 12px;overflow-x:auto;",
                button {
                    style: filter_tab_style(active_sort() == "default"),
                    onclick: move |_| active_sort.set("default".to_string()),
                    "✨ Top"
                }
                button {
                    style: filter_tab_style(active_sort() == "price-asc"),
                    onclick: move |_| active_sort.set("price-asc".to_string()),
                    "💰 ↑"
                }
                button {
                    style: filter_tab_style(active_sort() == "price-desc"),
                    onclick: move |_| active_sort.set("price-desc".to_string()),
                    "💰 ↓"
                }
                button {
                    style: filter_tab_style(active_sort() == "name"),
                    onclick: move |_| active_sort.set("name".to_string()),
                    "A–Z"
                }
                button {
                    style: filter_tab_style(active_sort() == "thc"),
                    onclick: move |_| active_sort.set("thc".to_string()),
                    "🔥 THC"
                }
            }

            // Content area
            {
                match &*strains_resource.read() {
                    Some(Ok(all_strains)) => {
                        let filter_val = active_filter();
                        let sort_val = active_sort();
                        let mut filtered: Vec<ApiStrain> = if filter_val == "All" {
                            all_strains.clone()
                        } else {
                            let needle = filter_val.to_lowercase();
                            all_strains.iter()
                                .filter(|s| s.category.as_deref().map(|c| c.eq_ignore_ascii_case(&needle)).unwrap_or(false))
                                .cloned()
                                .collect()
                        };
                        // Sort
                        match sort_val.as_str() {
                            "price-asc" => filtered.sort_by(|a,b| a.price_per_gram.partial_cmp(&b.price_per_gram).unwrap_or(std::cmp::Ordering::Equal)),
                            "price-desc" => filtered.sort_by(|a,b| b.price_per_gram.partial_cmp(&a.price_per_gram).unwrap_or(std::cmp::Ordering::Equal)),
                            "name" => filtered.sort_by(|a,b| a.name.to_lowercase().cmp(&b.name.to_lowercase())),
                            "thc" => filtered.sort_by(|a,b| b.thc_percent.unwrap_or(0.0).partial_cmp(&a.thc_percent.unwrap_or(0.0)).unwrap_or(std::cmp::Ordering::Equal)),
                            _ => {
                                // SOTD first, then bestsellers, then default
                                filtered.sort_by(|a,b| {
                                    let a_sod = if a.is_strain_of_day { 0 } else { 1 };
                                    let b_sod = if b.is_strain_of_day { 0 } else { 1 };
                                    a_sod.cmp(&b_sod)
                                });
                            }
                        }

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
        .map(|t| format!("THC {:.0}%", t))
        .unwrap_or_default();
    let cbd_str = strain.cbd_percent
        .map(|c| format!("CBD {:.1}%", c))
        .unwrap_or_default();
    let effect_str = strain.effect.clone().unwrap_or_default();
    let flavor_str = strain.flavor_profile.clone().unwrap_or_default();
    let has_real_price = strain.price_per_gram > 0.0;

    let badge_style = category_badge_style(cat);
    let badge_label = format!("{} {}", emoji, cat);

    let img_url = strain.image_url.clone().unwrap_or_default();
    let has_image = !img_url.is_empty();
    let alt_name = strain.name.clone();
    // Cache-bust image URLs to avoid stale HTML cached by trunk dev server
    let img_url_bust = if img_url.is_empty() {
        String::new()
    } else {
        format!("{}?v=2", img_url)
    };

    rsx! {
        div { key: strain.id.clone(), style: card_style,
            // Image area — фото в полный рост карточки (aspect 2:3, реальная пропорция webp 600x901)
            div { style: "width:100%;aspect-ratio:2/3;background:linear-gradient(135deg,#1a1a2e,#16213e);display:flex;align-items:center;justify-content:center;position:relative;overflow:hidden;",
                {if has_image {
                    rsx! {
                        img {
                            src: "{img_url_bust}",
                            alt: "{alt_name}",
                            loading: "lazy",
                            style: "width:100%;height:100%;object-fit:cover;display:block;"
                        }
                    }
                } else {
                    rsx! {
                        span { style: "font-size:48px;", "{emoji}" }
                    }
                }}
                {is_sotd.then(|| rsx! {
                    span { style: "
                        position:absolute;top:4px;right:4px;
                        font-size:5px;background:#ffe600;color:#000;
                        padding:2px 4px;border-radius:3px;
                        z-index:2;
                    ", "⭐ SOTD" }
                })}
            }
            // Content — имя, badge, THC/CBD, effect, flavor, цена/г
            div { style: "padding:10px;font-family:'Inter',system-ui,sans-serif;",
                div { style: "font-size:14px;font-weight:700;margin-bottom:6px;color:#ffffff;line-height:1.2;letter-spacing:0.2px;",
                    "{strain.name}"
                }
                div { style: "display:flex;gap:6px;align-items:center;margin-bottom:6px;flex-wrap:wrap;",
                    span { style: badge_style, "{badge_label}" }
                    {(!thc_str.is_empty()).then(|| rsx! {
                        span { style: "font-size:11px;color:#39ff14;font-weight:700;", "{thc_str}" }
                    })}
                    {(!cbd_str.is_empty()).then(|| rsx! {
                        span { style: "font-size:11px;color:#00e5ff;font-weight:600;", "{cbd_str}" }
                    })}
                }
                {(!effect_str.is_empty()).then(|| rsx! {
                    div { style: "font-size:11px;color:#c9c9d4;margin-bottom:4px;line-height:1.35;",
                        "{effect_str}"
                    }
                })}
                {(!flavor_str.is_empty()).then(|| rsx! {
                    div { style: "font-size:10px;color:#8b8b9e;margin-bottom:8px;line-height:1.35;",
                        "🍃 {flavor_str}"
                    }
                })}
                div { style: "display:flex;gap:6px;align-items:baseline;margin-bottom:6px;",
                    {if has_real_price {
                        rsx! {
                            span { style: "font-size:18px;color:#39ff14;font-weight:700;", "{display_price}" }
                            span { style: "font-size:11px;color:#8b8b9e;", "/г" }
                            {has_discount.then(|| rsx! {
                                span { style: "font-size:11px;color:#8b8b9e;text-decoration:line-through;margin-left:4px;",
                                    "{original_price}"
                                }
                            })}
                        }
                    } else {
                        rsx! {
                            span { style: "font-size:11px;color:#8b8b9e;font-style:italic;", "Цена по запросу" }
                        }
                    }}
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
                                font-family:'Inter',system-ui,sans-serif;
                                font-size:12px;font-weight:700;letter-spacing:0.3px;
                                width:100%;padding:9px;
                                background:#39ff14;color:#0f0f1a;
                                border:none;border-radius:6px;cursor:pointer;
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
                            font-family:'Inter',system-ui,sans-serif;
                            font-size:12px;font-weight:600;letter-spacing:0.3px;
                            width:100%;padding:9px;
                            background:transparent;color:#8b8b9e;
                            border:2px solid #2a2a4a;border-radius:6px;cursor:not-allowed;
                        ", "Sold Out" }
                    }
                }}
            }
        }
    }
}
