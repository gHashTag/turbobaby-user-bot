use crate::trios::core::Lang;
use crate::trios::i18n::{
    t, T_ADD_TO_CART, T_HOME_SUBTITLE, T_NAV_ACCESSORIES, T_NAV_GARDEN, T_NAV_MENU, T_NAV_SETS,
    T_NAV_TEA,
};
use crate::ui::api::context::api_base_url;
use crate::ui::assets;
use crate::ui::components::bottom_nav::BottomNav;
use crate::ui::routes::Route;
use crate::ui::state::{Cart, CartItem, CartItemType};
use dioxus::prelude::*;
use serde::Deserialize;

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

fn category_emoji(cat: &str) -> &'static str {
    match cat {
        "Sativa" => "☀️",
        "Indica" => "🌙",
        "Hybrid" => "⚖️",
        _ => "🌿",
    }
}

fn format_price(price: f64) -> String {
    let v = if price.is_finite() {
        price.max(0.0)
    } else {
        0.0
    };
    format!("฿{}", v as i32)
}

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
        crate::ui::api::local_client::LocalClient::new()
            .get(&url)
            .send()
            .await
            .map_err(|e| e.to_string())?
            .json::<SotdResponse>()
            .await
            .map(|r| r.strains.into_iter().next())
            .map_err(|e| e.to_string())
    });

    rsx! {
        div { style: "min-height:100vh;background:#0f0f1a;color:#e8e8e8;padding-bottom:80px;",

            div { style: "text-align:center;padding:20px 16px 16px;",
                img {
                    src: "{assets::logo::MAIN}",
                    alt: "Woody Weed Bot",
                    style: "height:120px;width:auto;display:block;margin:0 auto;box-shadow:0 0 20px rgba(57,255,20,0.3);cursor:pointer;user-select:none;transition:transform 0.1s;",
                }
                h1 { style: "font-size:24px;font-weight:800;color:#39ff14;text-shadow:3px 3px 0 #000,0 0 10px rgba(57,255,20,0.5);letter-spacing:2px;margin-top:12px;",
                    "WOODY WEEDPECKER"
                }
                p { style: "font-size:13px;color:#888;margin-top:6px;", "{home_subtitle}" }
            }

            {
                match &*sotd_resource.read() {
                    Some(Ok(Some(strain))) => {
                        let s = strain.clone();
                        let discount = if s.strain_of_day_discount.is_finite() { s.strain_of_day_discount.max(0.0) } else { 0.0 };
                        let price = if s.price_per_gram.is_finite() { s.price_per_gram.max(0.0) } else { 0.0 };
                        let cat = s.category.as_deref().unwrap_or("Hybrid");
                        let emoji = category_emoji(cat);
                        let has_discount = discount > 0.0;
                        let discount_label = if has_discount {
                            format!("🔥 {}% OFF", discount as i32)
                        } else {
                            "⭐ SOTD".to_string()
                        };
                        let display_price = if has_discount {
                            format_price((price * (1.0 - discount / 100.0)).max(0.0))
                        } else {
                            format_price(price)
                        };
                        let thc_str = s.thc_percent
                            .map(|t| format!("THC {}%", t as i32))
                            .unwrap_or_default();
                        let badge_label = format!("{} {}", emoji, cat);
                        let s_name = s.name.clone();
                        let s_id = s.id.clone();
                        let unit_price = if has_discount {
                            (price * (1.0 - discount / 100.0)).max(0.0)
                        } else {
                            price
                        };

                        rsx! {
                            div { style: "
                                margin:0 16px 16px;
                                background:linear-gradient(135deg,#ff6b35,#f7931e);
                                border:4px solid #ff6b35;
                                box-shadow:0 4px 15px rgba(255,107,53,0.4),4px 4px 0 #000;
                                padding:16px;position:relative;overflow:hidden;cursor:pointer;
                            ",
                                div { style: "display:flex;justify-content:space-between;align-items:center;margin-bottom:10px;position:relative;",
                                    h2 { style: "font-size:13px;font-weight:700;color:#fff;text-transform:uppercase;letter-spacing:1px;text-shadow:2px 2px 0 #000;",
                                        "⭐ Strain of the Day"
                                    }
                                    span { style: "
                                        font-size:13px;font-weight:700;
                                        background:#ffe600;color:#000;
                                        padding:4px 8px;
                                        box-shadow:2px 2px 0 #000;
                                    ", "{discount_label}" }
                                }
                                div { style: "font-size:17px;font-weight:700;margin-bottom:4px;position:relative;text-shadow:2px 2px 0 #000;", "{s_name}" }
                                div { style: "font-size:13px;color:#fff;margin-bottom:8px;position:relative;",
                                    span { style: "color:#fff;border:2px solid #fff;padding:2px 8px;font-size:13px;margin-right:8px;", "{badge_label}" }
                                    span { "{thc_str}" }
                                }
                                div { style: "display:flex;justify-content:space-between;align-items:center;position:relative;",
                                    span { style: "font-size:22px;font-weight:800;color:#fff;text-shadow:2px 2px 0 #000;", "{display_price}" }
                                    button {
                                        style: "
                                            font-size:14px;font-weight:700;
                                            background:#39ff14;color:#000;
                                            border:4px solid #2d9e0f;
                                            padding:12px 20px;
                                            box-shadow:3px 3px 0 #000;
                                            cursor:pointer;
                                        ",
                                        onclick: move |_| {
                                            let mut c = cart.write();
                                            c.add_item(CartItem {
                                                id: s_id.clone(),
                                                name: s_name.clone(),
                                                price: unit_price,
                                                quantity: 1,
                                                image_url: None,
                                                item_type: CartItemType::Strain,
                                            });
                                            crate::ui::telegram::TelegramApp::init().haptic_notification(crate::ui::telegram::HapticNotification::Success);
                                        },
                                        "{add_to_cart_label} 🛒"
                                    }
                                }
                            }
                        }
                    },
                    Some(Ok(None)) => {
                        rsx! {
                            div { style: "
                                margin:0 16px 16px;
                                background:#16213e;border:4px solid #2a2a4a;
                                box-shadow:4px 4px 0 #000;
                                padding:20px;text-align:center;
                            ",
                                p { style: "font-size:20px;margin-bottom:8px;", "🌟" }
                                p { style: "font-size:15px;color:#888;", "No strain of the day yet" }
                            }
                        }
                    },
                    Some(Err(_)) | None => {
                        rsx! {
                            div { style: "
                                margin:0 16px 16px;
                                background:#16213e;border:4px solid #2a2a4a;
                                box-shadow:4px 4px 0 #000;
                                padding:16px;min-height:100px;
                            ",
                                div { style: "font-size:13px;font-weight:700;color:#ffe600;text-shadow:2px 2px 0 #000;margin-bottom:10px;", "⭐ Strain of the Day" }
                                div { style: "font-size:15px;color:#888;", "Loading..." }
                            }
                        }
                    },
                }
            }

            div { style: "padding:0 16px 16px;",
                h2 { style: "font-size:13px;font-weight:700;color:#00e5ff;text-transform:uppercase;letter-spacing:1px;margin-bottom:12px;text-shadow:2px 2px 0 #000;",
                    "Categories"
                }
                div { style: "display:grid;grid-template-columns:repeat(3,1fr);gap:12px;",
                    Link { to: Route::Menu {},
                        div { style: "
                            background:#16213e;border:4px solid #2a2a4a;
                            box-shadow:4px 4px 0 #000;
                            padding:14px 8px;text-align:center;cursor:pointer;
                        ",
                            div { style: "font-size:28px;margin-bottom:6px;", "🌿" }
                            div { style: "font-size:13px;font-weight:700;color:#e8e8e8;", "{nav_menu}" }
                        }
                    }
                    Link { to: Route::Sets {},
                        div { style: "
                            background:#16213e;border:4px solid #2a2a4a;
                            box-shadow:4px 4px 0 #000;
                            padding:14px 8px;text-align:center;cursor:pointer;
                        ",
                            div { style: "font-size:28px;margin-bottom:6px;", "📦" }
                            div { style: "font-size:13px;font-weight:700;color:#e8e8e8;", "{nav_sets}" }
                        }
                    }
                    Link { to: Route::Sommelier {},
                        div { style: "
                            background:#16213e;border:4px solid #2a2a4a;
                            box-shadow:4px 4px 0 #000;
                            padding:14px 8px;text-align:center;cursor:pointer;
                        ",
                            div { style: "font-size:28px;margin-bottom:6px;", "🍷" }
                            div { style: "font-size:13px;font-weight:700;color:#e8e8e8;", "Sommelier" }
                        }
                    }
                    Link { to: Route::Accessories {},
                        div { style: "
                            background:#16213e;border:4px solid #2a2a4a;
                            box-shadow:4px 4px 0 #000;
                            padding:14px 8px;text-align:center;cursor:pointer;
                        ",
                            div { style: "font-size:28px;margin-bottom:6px;", "💨" }
                            div { style: "font-size:13px;font-weight:700;color:#e8e8e8;", "{nav_accessories}" }
                        }
                    }
                    Link { to: Route::Tea {},
                        div { style: "
                            background:#16213e;border:4px solid #2a2a4a;
                            box-shadow:4px 4px 0 #000;
                            padding:14px 8px;text-align:center;cursor:pointer;
                        ",
                            div { style: "font-size:28px;margin-bottom:6px;", "🍵" }
                            div { style: "font-size:13px;font-weight:700;color:#e8e8e8;", "{nav_tea}" }
                        }
                    }
                    Link { to: Route::Garden {},
                        div { style: "
                            background:#16213e;border:4px solid #2a2a4a;
                            box-shadow:4px 4px 0 #000;
                            padding:14px 8px;text-align:center;cursor:pointer;
                        ",
                            div { style: "font-size:28px;margin-bottom:6px;", "🌱" }
                            div { style: "font-size:13px;font-weight:700;color:#e8e8e8;", "{nav_garden}" }
                        }
                    }
                }
            }

            div { style: "padding:0 16px 16px;",
                h2 { style: "font-size:13px;font-weight:700;color:#b388ff;text-transform:uppercase;letter-spacing:1px;margin-bottom:12px;text-shadow:2px 2px 0 #000;",
                    "🎯 Adventures"
                }
                div { style: "display:grid;grid-template-columns:1fr 1fr;gap:12px;",
                    Link { to: Route::Quest { id: "daily".to_string() },
                        div { style: "
                            background:linear-gradient(135deg,rgba(0,229,255,0.1),rgba(57,255,20,0.1));
                            border:4px solid #00e5ff;
                            box-shadow:4px 4px 0 #000;
                            padding:12px;text-align:center;cursor:pointer;
                        ",
                            div { style: "font-size:28px;margin-bottom:4px;", "🎯" }
                            div { style: "font-size:13px;font-weight:700;color:#00e5ff;", "Daily Quest" }
                        }
                    }
                    Link { to: Route::TreasureHunt {},
                        div { style: "
                            background:linear-gradient(135deg,rgba(255,230,0,0.1),rgba(255,150,0,0.1));
                            border:4px solid #ffe600;
                            box-shadow:4px 4px 0 #000;
                            padding:12px;text-align:center;cursor:pointer;
                        ",
                            div { style: "font-size:28px;margin-bottom:4px;", "🏴‍☠️" }
                            div { style: "font-size:13px;font-weight:700;color:#ffe600;", "Treasure Hunt" }
                        }
                    }
                    Link { to: Route::ArHunt {},
                        div { style: "
                            background:linear-gradient(135deg,rgba(255,107,157,0.1),rgba(200,80,192,0.1));
                            border:4px solid #ff6b9d;
                            box-shadow:4px 4px 0 #000;
                            padding:12px;text-align:center;cursor:pointer;
                        ",
                            div { style: "font-size:28px;margin-bottom:4px;", "🔮" }
                            div { style: "font-size:13px;font-weight:700;color:#ff6b9d;", "AR Hunt" }
                        }
                    }
                    Link { to: Route::LocationQuest {},
                        div { style: "
                            background:linear-gradient(135deg,rgba(0,229,255,0.1),rgba(78,205,196,0.1));
                            border:4px solid #4ecdc4;
                            box-shadow:4px 4px 0 #000;
                            padding:12px;text-align:center;cursor:pointer;
                        ",
                            div { style: "font-size:28px;margin-bottom:4px;", "📍" }
                            div { style: "font-size:13px;font-weight:700;color:#4ecdc4;", "Location Quest" }
                        }
                    }
                }
            }

            // Tech Tree временно скрыт (по просьбе владельца). Код сохранён в routes/screens.
            // div { style: "padding:0 16px 16px;",
            //     h2 { style: "...", "🔧 More" }
            //     div { style: "display:grid;grid-template-columns:1fr;gap:12px;",
            //         Link { to: Route::TechTree {}, ... }
            //     }
            // }

            BottomNav { cart_count }
        }
    }
}
