use crate::trios::i18n::{
    t, T_ADD_TO_CART, T_HOME_SUBTITLE, T_NAV_ACCESSORIES, T_NAV_GARDEN, T_NAV_MENU, T_NAV_SETS,
    T_NAV_TEA,
};
use crate::ui::api::context::api_base_url;
use crate::ui::assets;
use crate::ui::components::bottom_nav::BottomNav;
use crate::ui::components::video_modal::VideoModal;
use crate::ui::routes::Route;
use crate::ui::share::SharedProduct;
use crate::ui::state::{Cart, CartItem, CartItemType};
use dioxus::prelude::*;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, PartialEq)]
struct SotdStrain {
    id: String,
    name: String,
    #[serde(default)]
    name_en: Option<String>,
    category: Option<String>,
    thc_percent: Option<f64>,
    price_per_gram: f64,
    image_url: Option<String>,
    #[serde(default)]
    video_url: Option<String>,
    strain_of_day_discount: f64,
}

#[derive(Debug, Deserialize)]
struct SotdResponse {
    strains: Vec<SotdStrain>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
struct HomePack {
    id: String,
    name: String,
    #[serde(default)]
    name_en: Option<String>,
    #[serde(default)]
    icon: Option<String>,
    #[serde(default)]
    image_url: Option<String>,
    #[serde(default)]
    total_price: f64,
    #[serde(default)]
    discount_percent: f64,
    #[serde(default)]
    badge: String,
    #[serde(default)]
    total_weight_grams: f64,
    #[serde(default)]
    strain_count: usize,
    #[serde(default)]
    is_available: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct HomePacksResponse {
    sets: Vec<HomePack>,
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
    // Single source of truth (was identical in menu/cart, narrowing to i32).
    crate::trios::pricing::format_baht(price)
}

/// Compact pack card for the home carousel. Tapping navigates to the Sets
/// section (the full card with add-to-cart / details lives there).
fn render_home_pack_card(p: HomePack) -> Element {
    let name = crate::ui::lang::localized(&p.name, p.name_en.as_deref());
    let discount = if p.discount_percent.is_finite() {
        p.discount_percent.max(0.0)
    } else {
        0.0
    };
    let total = if p.total_price.is_finite() {
        p.total_price.max(0.0)
    } else {
        0.0
    };
    let has_discount = discount > 0.0;
    let price = if has_discount {
        (total * (1.0 - discount / 100.0)).max(0.0)
    } else {
        total
    };
    let price_str = crate::trios::pricing::format_baht(price);
    let icon = p
        .icon
        .clone()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "🎁".to_string());
    let img = p.image_url.clone().unwrap_or_default();
    let has_image = img.starts_with("http://")
        || img.starts_with("https://")
        || (img.starts_with('/') && !img.starts_with("//"));
    let badge = crate::trios::packs::PackBadge::from_str(&p.badge);
    let badge_label = badge
        .label()
        .map(|(ru, en)| crate::ui::lang::localized(ru, Some(en)));
    let badge_color = badge.color();
    let strain_word = crate::ui::lang::localized("сортов", Some("strains"));
    let weight_line =
        crate::trios::packs::weight_line(p.total_weight_grams, p.strain_count, &strain_word);

    rsx! {
        Link { to: Route::Sets {},
            div { style: "flex:0 0 70%;scroll-snap-align:center;box-sizing:border-box;background:#16213e;border:4px solid #b388ff;box-shadow:4px 4px 0 #000;overflow:hidden;cursor:pointer;",
                div { style: "position:relative;min-height:90px;background:linear-gradient(135deg,#1a1a2e,#16213e);display:flex;align-items:center;justify-content:center;font-size:40px;",
                    if has_image {
                        img { src: "{img}", alt: "{name}", style: "width:100%;height:auto;object-fit:contain;display:block;" }
                    } else {
                        "{icon}"
                    }
                    span { style: "position:absolute;top:6px;left:6px;font-size:11px;font-weight:700;background:#b388ff;color:#000;padding:3px 6px;box-shadow:2px 2px 0 #000;", "📦 SET" }
                    if let Some(bl) = badge_label.clone() {
                        span { style: "position:absolute;top:32px;left:6px;font-size:10px;font-weight:700;background:{badge_color};color:#fff;padding:2px 6px;box-shadow:2px 2px 0 #000;", "{bl}" }
                    }
                    if has_discount {
                        span { style: "position:absolute;top:6px;right:6px;font-size:11px;font-weight:700;background:#b388ff;color:#000;padding:3px 6px;box-shadow:2px 2px 0 #000;", "{discount as i32}% OFF" }
                    }
                }
                div { style: "padding:10px 12px;",
                    div { style: "font-size:15px;font-weight:700;color:#e8e8e8;text-shadow:2px 2px 0 #000;margin-bottom:2px;white-space:nowrap;overflow:hidden;text-overflow:ellipsis;", "{name}" }
                    if !weight_line.is_empty() {
                        div { style: "font-size:11px;color:#b388ff;font-weight:600;margin-bottom:4px;", "⚖️ {weight_line}" }
                    }
                    span { style: "font-size:18px;font-weight:800;color:#ffe600;text-shadow:2px 2px 0 #000;", "{price_str}" }
                }
            }
        }
    }
}

#[component]
pub fn HomeScreen() -> Element {
    let cart = use_context::<Signal<Cart>>();
    let cart_count: u32 = cart.read().items.iter().map(|i| i.quantity).sum();

    // Deep-link landing: when the app opens with a shared product, navigate
    // from the home route to the catalog screen that owns the product.
    let pending = use_context::<Signal<Option<SharedProduct>>>();
    let nav = navigator();
    use_effect(move || {
        if let Some(target) = pending.read().clone() {
            // Navigate to the catalog screen that owns the shared product.
            // The target screen will open the product modal and clear the target.
            nav.push(target.kind.route());
        }
    });

    let home_subtitle = t(crate::ui::lang::current_lang(), T_HOME_SUBTITLE).to_string();
    let nav_menu = t(crate::ui::lang::current_lang(), T_NAV_MENU).to_string();
    let nav_sets = t(crate::ui::lang::current_lang(), T_NAV_SETS).to_string();
    let nav_accessories = t(crate::ui::lang::current_lang(), T_NAV_ACCESSORIES).to_string();
    let nav_tea = t(crate::ui::lang::current_lang(), T_NAV_TEA).to_string();
    let nav_garden = t(crate::ui::lang::current_lang(), T_NAV_GARDEN).to_string();
    let add_to_cart_label = t(crate::ui::lang::current_lang(), T_ADD_TO_CART).to_string();

    // Packs carousel (first block). Promo packs lead; tap → Sets section.
    let packs_resource = use_resource(|| async move {
        let base = api_base_url();
        let url = format!("{}/api/sets", base);
        crate::ui::api::local_client::LocalClient::new()
            .get(&url)
            .send()
            .await
            .map_err(|e| e.to_string())?
            .json::<HomePacksResponse>()
            .await
            .map(|r| r.sets)
            .map_err(|e| e.to_string())
    });

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
            .map(|r| r.strains)
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

            // Packs carousel — FIRST content block (flagship of the shop).
            {
                match &*packs_resource.read() {
                    Some(Ok(packs)) if packs.iter().any(|p| p.is_available.unwrap_or(true)) => {
                        let mut list: Vec<HomePack> = packs
                            .iter()
                            .filter(|p| p.is_available.unwrap_or(true))
                            .cloned()
                            .collect();
                        // Promo packs first; stable sort keeps admin order otherwise.
                        list.sort_by_key(|p| {
                            if crate::trios::packs::is_promo(&p.badge, p.discount_percent) { 0 } else { 1 }
                        });
                        let count = list.len();
                        let packs_hdr = crate::ui::lang::localized("📦 Наборы", Some("📦 Packs"));
                        rsx! {
                            div { style: "padding:0 16px 8px;",
                                h2 { style: "font-size:13px;font-weight:700;color:#b388ff;text-transform:uppercase;letter-spacing:1px;margin-bottom:12px;text-shadow:2px 2px 0 #000;",
                                    "{packs_hdr}"
                                }
                            }
                            div { style: "display:flex;overflow-x:auto;scroll-snap-type:x mandatory;-webkit-overflow-scrolling:touch;gap:12px;padding:0 16px 8px;",
                                for p in list.iter() {
                                    { render_home_pack_card(p.clone()) }
                                }
                            }
                            if count > 1 {
                                div { style: "display:flex;justify-content:center;gap:6px;margin:0 0 14px;",
                                    for _i in 0..count {
                                        span { style: "width:8px;height:8px;border-radius:50%;background:#b388ff;opacity:0.55;box-shadow:1px 1px 0 #000;" }
                                    }
                                }
                            }
                        }
                    },
                    // No packs / loading / error: render nothing (home stays clean).
                    _ => rsx! {},
                }
            }

            {
                match &*sotd_resource.read() {
                    Some(Ok(strains)) if !strains.is_empty() => {
                        // Carousel: up to 3 featured strains, native horizontal
                        // scroll-snap (one swipe = one slide), dot indicators.
                        let list = strains.clone();
                        let count = list.len();
                        rsx! {
                            div { style: "display:flex;overflow-x:auto;scroll-snap-type:x mandatory;-webkit-overflow-scrolling:touch;",
                                for s in list.iter() {
                                    div { style: "flex:0 0 100%;scroll-snap-align:center;box-sizing:border-box;",
                                        { render_sotd_card(s.clone(), cart, add_to_cart_label.clone()) }
                                    }
                                }
                            }
                            if count > 1 {
                                div { style: "display:flex;justify-content:center;gap:6px;margin:0 0 14px;",
                                    for _i in 0..count {
                                        span { style: "width:8px;height:8px;border-radius:50%;background:#ff6b35;opacity:0.55;box-shadow:1px 1px 0 #000;" }
                                    }
                                }
                            }
                        }
                    },
                    Some(Ok(_)) => {
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
                            div { style: "font-size:28px;margin-bottom:6px;", "🥤" }
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

/// One "Strain of the Day" carousel slide: the featured-strain card, with an
/// optional ▶ button that plays the strain's video (the owner's "card OR video").
/// Name is localized; add-to-cart mirrors the menu card.
fn render_sotd_card(s: SotdStrain, mut cart: Signal<Cart>, add_to_cart_label: String) -> Element {
    let discount = if s.strain_of_day_discount.is_finite() {
        s.strain_of_day_discount.max(0.0)
    } else {
        0.0
    };
    let price = if s.price_per_gram.is_finite() {
        s.price_per_gram.max(0.0)
    } else {
        0.0
    };
    let cat = s.category.as_deref().unwrap_or("Hybrid");
    let emoji = category_emoji(cat);
    let has_discount = discount > 0.0;
    let discount_label = if has_discount {
        format!("🔥 {}% OFF", discount as i32)
    } else {
        "⭐ SOTD".to_string()
    };
    let unit_price = if has_discount {
        (price * (1.0 - discount / 100.0)).max(0.0)
    } else {
        price
    };
    let display_price = format_price(unit_price);
    let thc_str = s
        .thc_percent
        .map(|t| format!("THC {}%", t as i32))
        .unwrap_or_default();
    let badge_label = format!("{} {}", emoji, cat);
    let s_name = crate::ui::lang::localized(&s.name, s.name_en.as_deref());
    let s_id = s.id.clone();
    let video_url = s.video_url.clone().unwrap_or_default();
    let has_video = !video_url.is_empty()
        && (video_url.starts_with("http://")
            || video_url.starts_with("https://")
            || (video_url.starts_with("/") && !video_url.starts_with("//")));
    let mut show_video = use_signal(|| false);

    rsx! {
        div { style: "
            margin:0 16px 16px;
            background:linear-gradient(135deg,#ff6b35,#f7931e);
            border:4px solid #ff6b35;
            box-shadow:0 4px 15px rgba(255,107,53,0.4),4px 4px 0 #000;
            padding:16px;position:relative;overflow:hidden;
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
                div { style: "display:flex;gap:8px;align-items:center;",
                    if has_video {
                        button {
                            style: "width:44px;height:44px;border-radius:50%;background:rgba(0,0,0,0.45);border:2px solid #fff;color:#fff;font-size:16px;display:flex;align-items:center;justify-content:center;cursor:pointer;",
                            "aria-label": "Смотреть видео",
                            onclick: move |_| show_video.set(true),
                            "▶️"
                        }
                    }
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
                                fulfillment: None,
                            });
                            crate::ui::telegram::TelegramApp::init().haptic_notification(crate::ui::telegram::HapticNotification::Success);
                        },
                        "{add_to_cart_label} 🛒"
                    }
                }
            }
            if show_video() {
                VideoModal { url: video_url.clone(), on_close: move |_| show_video.set(false) }
            }
        }
    }
}
