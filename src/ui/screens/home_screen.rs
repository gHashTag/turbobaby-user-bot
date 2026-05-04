// Home Screen — Connected to real Strain of the Day API
use dioxus::prelude::*;
use serde::Deserialize;
use crate::ui::routes::Route;
use crate::ui::state::{Cart, CartItem};
use crate::ui::assets;

// ── API response types ──────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, PartialEq)]
struct SotdStrain {
    id: String,
    name: String,
    category: Option<String>,
    thc_percent: Option<f64>,
    price_per_gram: f64,
    image_url: Option<String>,
    strain_of_day_discount: f64,
}

#[derive(Debug, Deserialize)]
struct SotdResponse {
    strains: Vec<SotdStrain>,
}

// ── Helpers ─────────────────────────────────────────────────────

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

fn format_price(price: f64) -> String {
    format!("฿{}", price as i32)
}

// ── Home Screen Component ───────────────────────────────────────

#[component]
pub fn HomeScreen() -> Element {
    let mut cart = use_context::<Signal<Cart>>();
    let cart_count: u32 = cart.read().items.iter().map(|i| i.quantity).sum();

    let sotd_resource = use_resource(|| async move {
        let base = api_base_url();
        let url = format!("{}/api/strains/strain-of-day", base);
        reqwest::Client::new()
            .get(&url)
            .send()
            .await
            .map_err(|e| e.to_string())?
            .json::<SotdResponse>()
            .await
            .map(|r| r.strains.into_iter().next()) // Take first strain
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
            // Header
            div { style: "
                text-align: center;
                padding: 20px 16px 12px;
            ",
                img {
                    src: "{assets::logo::MAIN}",
                    alt: "Woody Weed Bot",
                    style: "height: 80px; width: auto; display: block; margin: 0 auto;",
                }
                p { style: "
                    font-size: 7px;
                    color: #8b8b9e;
                    margin-top: 6px;
                ", "Premium Cannabis Delivery in Bangkok" }
            }

            // Hero — Strain of the Day (from API)
            {
                match &*sotd_resource.read() {
                    Some(Ok(Some(strain))) => {
                        let s = strain.clone();
                        let cat = s.category.as_deref().unwrap_or("Hybrid");
                        let emoji = category_emoji(cat);
                        let has_discount = s.strain_of_day_discount > 0.0;
                        let discount_label = if has_discount {
                            format!("{}% OFF", s.strain_of_day_discount as i32)
                        } else {
                            "⭐ SOTD".to_string()
                        };
                        let display_price = if has_discount {
                            format_price(s.price_per_gram * (1.0 - s.strain_of_day_discount / 100.0))
                        } else {
                            format_price(s.price_per_gram)
                        };
                        let thc_str = s.thc_percent
                            .map(|t| format!("{}% THC", t as i32))
                            .unwrap_or_default();
                        let badge_label = format!("{} {}", emoji, cat);
                        let s_name = s.name.clone();
                        let s_id = s.id.clone();
                        let unit_price = if has_discount {
                            s.price_per_gram * (1.0 - s.strain_of_day_discount / 100.0)
                        } else {
                            s.price_per_gram
                        };

                        rsx! {
                            div { style: "
                                margin: 0 16px 16px;
                                background: linear-gradient(135deg, #1a1a2e, #16213e);
                                border: 2px solid #ffe600;
                                border-radius: 12px;
                                padding: 16px;
                                box-shadow: 0 0 20px rgba(255,230,0,0.15), 4px 4px 0 #000;
                                position: relative;
                                overflow: hidden;
                            ",
                                div { style: "
                                    position: absolute; top: 0; left: 0; right: 0; bottom: 0;
                                    opacity: 0.03;
                                    background: repeating-linear-gradient(0deg, transparent, transparent 4px, rgba(255,255,255,0.1) 4px, rgba(255,255,255,0.1) 8px);
                                " }
                                div { style: "display: flex; justify-content: space-between; align-items: center; margin-bottom: 10px; position: relative;",
                                    h2 { style: "font-size: 9px; color: #ffe600; text-transform: uppercase; letter-spacing: 1px;", "⭐ Strain of the Day" }
                                    span { style: "
                                        font-size: 6px;
                                        background: #ffe600;
                                        color: #000;
                                        padding: 2px 6px;
                                        border-radius: 4px;
                                    ", "{discount_label}" }
                                }
                                div { style: "font-size: 12px; font-weight: bold; margin-bottom: 4px; position: relative;", "{s_name}" }
                                div { style: "font-size: 8px; color: #8b8b9e; margin-bottom: 8px; position: relative;",
                                    span { style: "color: #ffe600; border: 1px solid #ffe600; padding: 1px 4px; border-radius: 3px; font-size: 6px; margin-right: 6px;", "{badge_label}" }
                                    span { "{thc_str}" }
                                }
                                div { style: "display: flex; justify-content: space-between; align-items: center; position: relative;",
                                    span { style: "font-size: 11px; color: #39ff14;", "{display_price}" }
                                    button {
                                        style: "
                                            font-family: 'Press Start 2P', monospace;
                                            font-size: 7px;
                                            background: #39ff14;
                                            color: #0f0f1a;
                                            border: none;
                                            padding: 8px 14px;
                                            border-radius: 6px;
                                            cursor: pointer;
                                            box-shadow: 2px 2px 0 #000;
                                        ",
                                        onclick: move |_| {
                                            let mut c = cart.write();
                                            c.add_item(CartItem {
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
                            }
                        }
                    },
                    Some(Ok(None)) => {
                        // No strain of the day — show placeholder
                        rsx! {
                            div { style: "
                                margin: 0 16px 16px;
                                background: linear-gradient(135deg, #1a1a2e, #16213e);
                                border: 2px solid #2a2a4a;
                                border-radius: 12px;
                                padding: 20px;
                                text-align: center;
                                box-shadow: 4px 4px 0 #000;
                            ",
                                p { style: "font-size: 20px; margin-bottom: 8px;", "🌟" }
                                p { style: "font-size: 8px; color: #8b8b9e;", "No strain of the day yet" }
                            }
                        }
                    },
                    Some(Err(_)) | None => {
                        // Loading or error — show skeleton
                        rsx! {
                            div { style: "
                                margin: 0 16px 16px;
                                background: linear-gradient(135deg, #1a1a2e, #16213e);
                                border: 2px solid #2a2a4a;
                                border-radius: 12px;
                                padding: 16px;
                                box-shadow: 4px 4px 0 #000;
                                min-height: 100px;
                            ",
                                div { style: "font-size: 9px; color: #ffe600; margin-bottom: 10px;", "⭐ Strain of the Day" }
                                div { style: "font-size: 8px; color: #8b8b9e;", "Loading..." }
                            }
                        }
                    },
                }
            }

            // Categories
            div { style: "padding: 0 16px 16px;",
                h2 { style: "font-size: 10px; color: #00e5ff; margin-bottom: 12px; text-transform: uppercase; letter-spacing: 1px;", "Categories" }
                div { style: "display: grid; grid-template-columns: repeat(3, 1fr); gap: 10px;",
                    // Strains
                    Link { to: Route::Menu {},
                        div { style: "
                            background: #16213e;
                            border: 2px solid #2a2a4a;
                            border-radius: 8px;
                            padding: 14px 8px;
                            text-align: center;
                            box-shadow: 3px 3px 0 #000;
                            cursor: pointer;
                            transition: border-color 0.2s;
                        ",
                            div { style: "font-size: 28px; margin-bottom: 6px;", "🌿" }
                            div { style: "font-size: 7px; color: #e8e8e8;", "Strains" }
                        }
                    }
                    // Sets
                    Link { to: Route::Sets {},
                        div { style: "
                            background: #16213e;
                            border: 2px solid #2a2a4a;
                            border-radius: 8px;
                            padding: 14px 8px;
                            text-align: center;
                            box-shadow: 3px 3px 0 #000;
                            cursor: pointer;
                        ",
                            div { style: "font-size: 28px; margin-bottom: 6px;", "📦" }
                            div { style: "font-size: 7px; color: #e8e8e8;", "Sets" }
                        }
                    }
                    // Sommelier
                    Link { to: Route::Sommelier {},
                        div { style: "
                            background: #16213e;
                            border: 2px solid #2a2a4a;
                            border-radius: 8px;
                            padding: 14px 8px;
                            text-align: center;
                            box-shadow: 3px 3px 0 #000;
                            cursor: pointer;
                        ",
                            div { style: "font-size: 28px; margin-bottom: 6px;", "🍷" }
                            div { style: "font-size: 7px; color: #e8e8e8;", "Sommelier" }
                        }
                    }
                    // Accessories
                    Link { to: Route::Accessories {},
                        div { style: "
                            background: #16213e;
                            border: 2px solid #2a2a4a;
                            border-radius: 8px;
                            padding: 14px 8px;
                            text-align: center;
                            box-shadow: 3px 3px 0 #000;
                            cursor: pointer;
                        ",
                            div { style: "font-size: 28px; margin-bottom: 6px;", "💨" }
                            div { style: "font-size: 7px; color: #e8e8e8;", "Accessories" }
                        }
                    }
                    // Tea
                    Link { to: Route::Tea {},
                        div { style: "
                            background: #16213e;
                            border: 2px solid #2a2a4a;
                            border-radius: 8px;
                            padding: 14px 8px;
                            text-align: center;
                            box-shadow: 3px 3px 0 #000;
                            cursor: pointer;
                        ",
                            div { style: "font-size: 28px; margin-bottom: 6px;", "🍵" }
                            div { style: "font-size: 7px; color: #e8e8e8;", "Tea" }
                        }
                    }
                    // Garden
                    Link { to: Route::Garden {},
                        div { style: "
                            background: #16213e;
                            border: 2px solid #2a2a4a;
                            border-radius: 8px;
                            padding: 14px 8px;
                            text-align: center;
                            box-shadow: 3px 3px 0 #000;
                            cursor: pointer;
                        ",
                            div { style: "font-size: 28px; margin-bottom: 6px;", "🌱" }
                            div { style: "font-size: 7px; color: #e8e8e8;", "Garden" }
                        }
                    }
                }
            }

            // Quest Banner
            Link { to: Route::Quest { id: "daily".to_string() },
                div { style: "
                    margin: 0 16px 16px;
                    background: linear-gradient(135deg, rgba(0,229,255,0.1), rgba(57,255,20,0.1));
                    border: 2px solid #00e5ff;
                    border-radius: 12px;
                    padding: 14px;
                    display: flex;
                    align-items: center;
                    gap: 12px;
                    box-shadow: 0 0 12px rgba(0,229,255,0.15), 3px 3px 0 #000;
                    cursor: pointer;
                ",
                    div { style: "font-size: 32px;", "🎯" }
                    div { style: "flex: 1;",
                        div { style: "font-size: 9px; color: #00e5ff; margin-bottom: 4px;", "Daily Quest" }
                        div { style: "font-size: 7px; color: #8b8b9e;", "Scan QR codes to earn rewards!" }
                    }
                    div { style: "font-size: 14px; color: #00e5ff;", "→" }
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
                        div { style: "font-size: 6px; color: #39ff14; margin-top: 2px;", "Home" }
                    }
                }
                Link { to: Route::Menu {},
                    div { style: "text-align: center; cursor: pointer;",
                        div { style: "font-size: 20px;", "🌿" }
                        div { style: "font-size: 6px; color: #8b8b9e; margin-top: 2px;", "Menu" }
                    }
                }
                Link { to: Route::Cart {},
                    div { style: "text-align: center; cursor: pointer; position: relative;",
                        div { style: "font-size: 20px;", "🛒" }
                        if cart_count > 0 {
                            div { style: "
                                position: absolute; top: -4px; right: -8px;
                                background: #ff4757; color: white;
                                font-size: 6px; padding: 1px 4px;
                                border-radius: 8px; min-width: 12px; text-align: center;
                            ", "{cart_count}" }
                        }
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
