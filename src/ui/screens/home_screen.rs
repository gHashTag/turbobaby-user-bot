// Home Screen — Connected to real Strain of the Day API
use dioxus::prelude::*;
use serde::Deserialize;
use crate::ui::routes::Route;
use crate::ui::state::{Cart, CartItem};
use crate::ui::assets;
use crate::ui::api::context::api_base_url;
use crate::trios::core::Lang;
use crate::ui::components::bottom_nav::BottomNav;
use crate::trios::i18n::{t, T_HOME_SUBTITLE, T_NAV_MENU, T_NAV_SETS, T_NAV_ACCESSORIES, T_NAV_TEA, T_NAV_GARDEN, T_ADD_TO_CART};

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

    let home_subtitle = t(Lang::Russian, T_HOME_SUBTITLE).to_string();
    let nav_menu = t(Lang::Russian, T_NAV_MENU).to_string();
    let nav_sets = t(Lang::Russian, T_NAV_SETS).to_string();
    let nav_accessories = t(Lang::Russian, T_NAV_ACCESSORIES).to_string();
    let nav_tea = t(Lang::Russian, T_NAV_TEA).to_string();
    let nav_garden = t(Lang::Russian, T_NAV_GARDEN).to_string();
    let add_to_cart_label = t(Lang::Russian, T_ADD_TO_CART).to_string();

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
                    style: "height: 100px; width: auto; display: block; margin: 0 auto;",
                }
                p { style: "
                    font-size: 11px;
                    color: #8b8b9e;
                    margin-top: 6px;
                ", "{home_subtitle}" }
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
                                background: linear-gradient(135deg, #1a1a2e, #1a1a2e);
                                border: 2px solid #ffe600;
                                border-radius: 8px;
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
                                    h2 { style: "font-size: 14px; color: #ffe600; text-transform: uppercase; letter-spacing: 1px;", "⭐ Strain of the Day" }
                                    span { style: "
                                        font-size: 10px;
                                        background: #ffe600;
                                        color: #000;
                                        padding: 2px 6px;
                                        border-radius: 8px;
                                    ", "{discount_label}" }
                                }
                                div { style: "font-size: 10px; font-weight: bold; margin-bottom: 4px; position: relative;", "{s_name}" }
                                div { style: "font-size: 12px; color: #8b8b9e; margin-bottom: 8px; position: relative;",
                                    span { style: "color: #ffe600; border: 2px solid #ffe600; padding: 1px 4px; border-radius: 3px; font-size: 10px; margin-right: 6px;", "{badge_label}" }
                                    span { "{thc_str}" }
                                }
                                div { style: "display: flex; justify-content: space-between; align-items: center; position: relative;",
                                    span { style: "font-size: 9px; color: #39ff14;", "{display_price}" }
                                    button {
                                        style: "
                                            font-family: 'Press Start 2P', monospace;
                                            font-size: 10px;
                                            background: #39ff14;
                                            color: #0f0f1a;
                                            border: none;
                                            padding: 8px 14px;
                                            border-radius: 6px;
                                            cursor: pointer;
                                            box-shadow: 4px 4px 0 #000;
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
                                        "{add_to_cart_label} 🛒"
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
                                background: linear-gradient(135deg, #1a1a2e, #1a1a2e);
                                border: 2px solid #2a2a4a;
                                border-radius: 8px;
                                padding: 20px;
                                text-align: center;
                                box-shadow: 4px 4px 0 #000;
                            ",
                                p { style: "font-size: 10px; margin-bottom: 8px;", "🌟" }
                                p { style: "font-size: 12px; color: #8b8b9e;", "No strain of the day yet" }
                            }
                        }
                    },
                    Some(Err(_)) | None => {
                        // Loading or error — show skeleton
                        rsx! {
                            div { style: "
                                margin: 0 16px 16px;
                                background: linear-gradient(135deg, #1a1a2e, #1a1a2e);
                                border: 2px solid #2a2a4a;
                                border-radius: 8px;
                                padding: 16px;
                                box-shadow: 4px 4px 0 #000;
                                min-height: 100px;
                            ",
                                div { style: "font-size: 14px; color: #ffe600; margin-bottom: 10px;", "⭐ Strain of the Day" }
                                div { style: "font-size: 12px; color: #8b8b9e;", "Loading..." }
                            }
                        }
                    },
                }
            }

            // Categories
            div { style: "padding: 0 16px 16px;",
                h2 { style: "font-size: 14px; color: #00e5ff; margin-bottom: 12px; text-transform: uppercase; letter-spacing: 1px;", "Categories" }
                div { style: "display: grid; grid-template-columns: repeat(3, 1fr); gap: 10px;",
                    // Strains
                    Link { to: Route::Menu {},
                        div { style: "
                            background: #1a1a2e;
                            border: 2px solid #2a2a4a;
                            border-radius: 8px;
                            padding: 14px 8px;
                            text-align: center;
                            box-shadow: 4px 4px 0 #000;
                            cursor: pointer;
                            transition: border-color 0.2s;
                        ",
                            div { style: "font-size: 28px; margin-bottom: 6px;", "🌿" }
                            div { style: "font-size: 18px; color: #e8e8e8;", "{nav_menu}" }
                        }
                    }
                    // Sets
                    Link { to: Route::Sets {},
                        div { style: "
                            background: #1a1a2e;
                            border: 2px solid #2a2a4a;
                            border-radius: 8px;
                            padding: 14px 8px;
                            text-align: center;
                            box-shadow: 4px 4px 0 #000;
                            cursor: pointer;
                        ",
                            div { style: "font-size: 28px; margin-bottom: 6px;", "📦" }
                            div { style: "font-size: 18px; color: #e8e8e8;", "{nav_sets}" }
                        }
                    }
                    // Sommelier
                    Link { to: Route::Sommelier {},
                        div { style: "
                            background: #1a1a2e;
                            border: 2px solid #2a2a4a;
                            border-radius: 8px;
                            padding: 14px 8px;
                            text-align: center;
                            box-shadow: 4px 4px 0 #000;
                            cursor: pointer;
                        ",
                            div { style: "font-size: 28px; margin-bottom: 6px;", "🍷" }
                            div { style: "font-size: 18px; color: #e8e8e8;", "Sommelier" }
                        }
                    }
                    // Accessories
                    Link { to: Route::Accessories {},
                        div { style: "
                            background: #1a1a2e;
                            border: 2px solid #2a2a4a;
                            border-radius: 8px;
                            padding: 14px 8px;
                            text-align: center;
                            box-shadow: 4px 4px 0 #000;
                            cursor: pointer;
                        ",
                            div { style: "font-size: 28px; margin-bottom: 6px;", "💨" }
                            div { style: "font-size: 18px; color: #e8e8e8;", "{nav_accessories}" }
                        }
                    }
                    // Tea
                    Link { to: Route::Tea {},
                        div { style: "
                            background: #1a1a2e;
                            border: 2px solid #2a2a4a;
                            border-radius: 8px;
                            padding: 14px 8px;
                            text-align: center;
                            box-shadow: 4px 4px 0 #000;
                            cursor: pointer;
                        ",
                            div { style: "font-size: 28px; margin-bottom: 6px;", "🍵" }
                            div { style: "font-size: 18px; color: #e8e8e8;", "{nav_tea}" }
                        }
                    }
                    // Garden
                    Link { to: Route::Garden {},
                        div { style: "
                            background: #1a1a2e;
                            border: 2px solid #2a2a4a;
                            border-radius: 8px;
                            padding: 14px 8px;
                            text-align: center;
                            box-shadow: 4px 4px 0 #000;
                            cursor: pointer;
                        ",
                            div { style: "font-size: 28px; margin-bottom: 6px;", "🌱" }
                            div { style: "font-size: 18px; color: #e8e8e8;", "{nav_garden}" }
                        }
                    }
                }
            }

            // Quest & Adventures Section
            div { style: "padding: 0 16px 16px;",
                h2 { style: "font-size: 14px; color: #c850c0; margin-bottom: 12px; text-transform: uppercase; letter-spacing: 1px;", "🎯 Adventures" }
                div { style: "display: grid; grid-template-columns: 1fr 1fr; gap: 10px;",
                    // Daily Quest
                    Link { to: Route::Quest { id: "daily".to_string() },
                        div { style: "
                            background: linear-gradient(135deg, rgba(0,229,255,0.1), rgba(57,255,20,0.1));
                            border: 2px solid #00e5ff; border-radius: 8px;
                            padding: 12px; text-align: center;
                            box-shadow: 4px 4px 0 #000; cursor: pointer;
                        ",
                            div { style: "font-size: 28px; margin-bottom: 4px;", "🎯" }
                            div { style: "font-size: 18px; color: #00e5ff;", "Daily Quest" }
                        }
                    }
                    // Treasure Hunt
                    Link { to: Route::TreasureHunt {},
                        div { style: "
                            background: linear-gradient(135deg, rgba(255,230,0,0.1), rgba(255,150,0,0.1));
                            border: 2px solid #ffe600; border-radius: 8px;
                            padding: 12px; text-align: center;
                            box-shadow: 4px 4px 0 #000; cursor: pointer;
                        ",
                            div { style: "font-size: 28px; margin-bottom: 4px;", "🏴‍☠️" }
                            div { style: "font-size: 18px; color: #ffe600;", "Treasure Hunt" }
                        }
                    }
                    // AR Hunt
                    Link { to: Route::ArHunt {},
                        div { style: "
                            background: linear-gradient(135deg, rgba(255,107,157,0.1), rgba(200,80,192,0.1));
                            border: 2px solid #ff6b9d; border-radius: 8px;
                            padding: 12px; text-align: center;
                            box-shadow: 4px 4px 0 #000; cursor: pointer;
                        ",
                            div { style: "font-size: 28px; margin-bottom: 4px;", "🔮" }
                            div { style: "font-size: 18px; color: #ff6b9d;", "AR Hunt" }
                        }
                    }
                    // Location Quest
                    Link { to: Route::LocationQuest {},
                        div { style: "
                            background: linear-gradient(135deg, rgba(0,229,255,0.1), rgba(78,205,196,0.1));
                            border: 2px solid #4ecdc4; border-radius: 8px;
                            padding: 12px; text-align: center;
                            box-shadow: 4px 4px 0 #000; cursor: pointer;
                        ",
                            div { style: "font-size: 28px; margin-bottom: 4px;", "📍" }
                            div { style: "font-size: 18px; color: #4ecdc4;", "Location Quest" }
                        }
                    }
                }
            }

            // Tech Tree & Admin Section
            div { style: "padding: 0 16px 16px;",
                h2 { style: "font-size: 14px; color: #8b8b9e; margin-bottom: 12px; text-transform: uppercase; letter-spacing: 1px;", "🔧 More" }
                div { style: "display: grid; grid-template-columns: 1fr 1fr; gap: 10px;",
                    // Tech Tree
                    Link { to: Route::TechTree {},
                        div { style: "
                            background: #1a1a2e; border: 2px solid #2a2a4a;
                            border-radius: 8px; padding: 12px; text-align: center;
                            box-shadow: 4px 4px 0 #000; cursor: pointer;
                        ",
                            div { style: "font-size: 28px; margin-bottom: 4px;", "🌳" }
                            div { style: "font-size: 18px; color: #c850c0;", "Tech Tree" }
                        }
                    }
                    // Admin
                    Link { to: Route::Admin {},
                        div { style: "
                            background: #1a1a2e; border: 2px solid #2a2a4a;
                            border-radius: 8px; padding: 12px; text-align: center;
                            box-shadow: 4px 4px 0 #000; cursor: pointer;
                        ",
                            div { style: "font-size: 28px; margin-bottom: 4px;", "⚙️" }
                            div { style: "font-size: 18px; color: #ff4757;", "Admin" }
                        }
                    }
                }
            }

            BottomNav { cart_count }
        }
    }
}
