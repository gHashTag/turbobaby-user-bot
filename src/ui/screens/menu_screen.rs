use crate::trios::core::Lang;
use crate::trios::i18n::{t, T_ADD_TO_CART, T_LOADING, T_MENU_DESC, T_MENU_TITLE};
use crate::ui::api::context::api_base_url;
use crate::ui::components::bottom_nav::BottomNav;
use crate::ui::routes::Route;
use crate::ui::state::{Cart, CartItem, CartItemType};
use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct ApiStrain {
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
    video_url: Option<String>,
    is_available: bool,
    #[serde(default)]
    is_strain_of_day: bool,
    #[serde(default)]
    strain_of_day_discount: f64,
    // Marketing flags (migration 028, TZ #2). All serde-default so older API
    // responses that don't include them keep working.
    #[serde(default)]
    discount_percent: f64,
    #[serde(default)]
    sale_price: Option<f64>,
    #[serde(default)]
    sale_active: bool,
    #[serde(default)]
    sale_until: Option<String>,
    #[serde(default)]
    is_best_seller: bool,
    #[serde(default)]
    is_new_arrival: bool,
    #[serde(default)]
    new_until: Option<String>,
    #[serde(default)]
    display_order: i32,
}

/// Local convenience wrapper around the shared `trios::pricing` helper.
/// Cycle #55 extracted the math into `trios/pricing.rs` so a future
/// server-side price-authority check uses the same code path — drift between
/// client and server pricing would otherwise flag every order as fraud.
fn is_active_until(until: Option<&str>) -> bool {
    crate::trios::pricing::is_active_until(until, chrono::Utc::now())
}

#[derive(Debug, Deserialize)]
struct StrainsResponse {
    strains: Vec<ApiStrain>,
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
        "Sativa" => ("#ffe600", "rgba(255,230,0,0.15)"),
        "Indica" => ("#b388ff", "rgba(179,136,255,0.15)"),
        "Hybrid" => ("#39ff14", "rgba(57,255,20,0.15)"),
        _ => ("#888", "rgba(136,136,136,0.1)"),
    };
    format!(
        "font-size:13px;color:{};border:2px solid {};background:{};padding:2px 8px;",
        color, color, bg
    )
}

fn format_price(price: f64) -> String {
    let v = if price.is_finite() {
        price.max(0.0)
    } else {
        0.0
    };
    format!("฿{}", v as i32)
}

/// Safe f64 comparison that places NaN at the end.
fn cmp_f64(a: f64, b: f64) -> std::cmp::Ordering {
    a.partial_cmp(&b).unwrap_or_else(|| {
        if a.is_nan() && b.is_nan() {
            std::cmp::Ordering::Equal
        } else if a.is_nan() {
            std::cmp::Ordering::Greater
        } else {
            std::cmp::Ordering::Less
        }
    })
}

fn filter_tab_style(is_active: bool) -> String {
    if is_active {
        "font-size:12px;font-weight:600;padding:8px 16px;background:rgba(57,255,20,0.15);color:#39ff14;border:3px solid #39ff14;border-radius:20px;cursor:pointer;white-space:nowrap;".to_string()
    } else {
        "font-size:12px;font-weight:600;padding:8px 16px;background:transparent;color:#888;border:3px solid #2a2a4a;border-radius:20px;cursor:pointer;white-space:nowrap;".to_string()
    }
}

#[component]
pub fn MenuScreen() -> Element {
    let mut active_filter = use_signal(|| "All".to_string());
    let mut active_sort = use_signal(|| "default".to_string());

    let cart = use_context::<Signal<Cart>>();
    let cart_count: u32 = cart.read().items.iter().map(|i| i.quantity).sum();

    let menu_title = t(Lang::Russian, T_MENU_TITLE).to_string();
    let menu_desc = t(Lang::Russian, T_MENU_DESC).to_string();
    let loading_label = t(Lang::Russian, T_LOADING).to_string();

    let strains_resource: Resource<Result<Vec<ApiStrain>, String>> = use_resource(move || {
        async move {
            // Fetch from API
            let base = api_base_url();
            let url = format!("{}/api/strains", base);
            let response = crate::ui::api::local_client::LocalClient::new()
                .get(&url)
                .send()
                .await
                .map_err(|e| format!("Network error: {}", e))?;

            let text = response
                .text()
                .await
                .map_err(|e| format!("Read text error: {}", e))?;
            let strains_resp: StrainsResponse =
                serde_json::from_str(&text).map_err(|e| format!("Parse error: {}", e))?;

            Ok(strains_resp.strains)
        }
    });

    rsx! {
        div { style: "min-height:100vh;background:#0f0f1a;color:#e8e8e8;padding-bottom:80px;",

            div { style: "padding:20px 16px 16px;text-align:center;position:relative;",
                h1 { style: "font-size:24px;font-weight:800;color:#39ff14;text-shadow:3px 3px 0 #000,0 0 10px rgba(57,255,20,0.5);letter-spacing:2px;",
                    "{menu_title}"
                }
                p { style: "font-size:13px;color:#888;margin-top:4px;", "{menu_desc}" }
                if cart_count > 0 {
                    Link { to: Route::Cart {},
                        div { style: "
                            position:absolute;top:20px;right:16px;
                            background:#39ff14;color:#000;
                            font-size:13px;font-weight:700;padding:4px 8px;
                            box-shadow:2px 2px 0 #000;cursor:pointer;
                        ",
                            "🛒 {cart_count}"
                        }
                    }
                }
            }

            div { style: "display:flex;gap:6px;padding:0 16px 12px;overflow-x:auto;",
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
                        match sort_val.as_str() {
                            "price-asc" => filtered.sort_by(|a,b| cmp_f64(a.price_per_gram, b.price_per_gram)),
                            "price-desc" => filtered.sort_by(|a,b| cmp_f64(b.price_per_gram, a.price_per_gram)),
                            "name" => filtered.sort_by(|a,b| a.name.to_lowercase().cmp(&b.name.to_lowercase())),
                            "thc" => filtered.sort_by(|a,b| cmp_f64(b.thc_percent.unwrap_or(0.0), a.thc_percent.unwrap_or(0.0))),
                            _ => {
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
                                div { style: "text-align:center;padding:48px 16px;",
                                    p { style: "font-size:20px;margin-bottom:12px;", "🔍" }
                                    p { style: "font-size:15px;color:#888;", "No {f} strains found" }
                                }
                            }
                        } else {
                            // TZ #2: pull SOTD + New Arrivals into their own hero
                            // blocks. The remaining strains keep the regular grid
                            // below. Render only when filter is "All" so the
                            // category tabs (Sativa/Indica/Hybrid) still drill in.
                            let show_hero = filter_val == "All" && sort_val == "default";
                            let sotd: Option<ApiStrain> = if show_hero {
                                filtered.iter().find(|s| s.is_strain_of_day).cloned()
                            } else { None };
                            let new_arrivals: Vec<ApiStrain> = if show_hero {
                                filtered.iter()
                                    .filter(|s| !s.is_strain_of_day
                                        && s.is_new_arrival
                                        && is_active_until(s.new_until.as_deref()))
                                    .cloned()
                                    .collect()
                            } else { Vec::new() };
                            let hero_ids: std::collections::HashSet<String> =
                                sotd.iter().map(|s| s.id.clone())
                                    .chain(new_arrivals.iter().map(|s| s.id.clone()))
                                    .collect();
                            let rest: Vec<ApiStrain> = filtered.iter()
                                .filter(|s| !hero_ids.contains(&s.id))
                                .cloned()
                                .collect();
                            rsx! {
                                // Hero #1 — Strain of the Day
                                {sotd.map(|s| {
                                    let sid = s.id.clone();
                                    rsx! {
                                        div { key: "hero-sotd",
                                            style: "padding:8px 16px 4px;",
                                            div { style: "font-size:13px;font-weight:800;color:#ffe600;text-shadow:2px 2px 0 #000;letter-spacing:1px;margin-bottom:8px;",
                                                "🔥 STRAIN OF THE DAY"
                                            }
                                            div { key: "{sid}",
                                                style: "max-width:380px;margin:0 auto;",
                                                {render_strain_card(s, cart)}
                                            }
                                        }
                                    }
                                })}
                                // Hero #2 — New Arrivals
                                {(!new_arrivals.is_empty()).then(|| rsx! {
                                    div { key: "hero-new",
                                        style: "padding:12px 16px 4px;",
                                        div { style: "font-size:13px;font-weight:800;color:#00e5ff;text-shadow:2px 2px 0 #000;letter-spacing:1px;margin-bottom:8px;",
                                            "🆕 NEW ARRIVALS"
                                        }
                                        div { style: "display:grid;grid-template-columns:1fr 1fr;gap:12px;",
                                            {new_arrivals.into_iter().map(|s| render_strain_card(s, cart))}
                                        }
                                    }
                                })}
                                // Rest of the catalog
                                div { style: "display:grid;grid-template-columns:1fr 1fr;gap:12px;padding:8px 16px 0;",
                                    {rest.into_iter().map(|s| render_strain_card(s, cart))}
                                }
                            }
                        }
                    },
                    Some(Err(e)) => {
                        let err_msg = e.clone();
                        rsx! {
                            div { style: "text-align:center;padding:48px 16px;",
                                p { style: "font-size:20px;margin-bottom:12px;", "⚠️" }
                                p { style: "font-size:15px;color:#ff4757;", "Error loading strains" }
                                p { style: "font-size:13px;color:#888;margin-top:8px;word-break:break-all;", "{err_msg}" }
                            }
                        }
                    },
                    None => {
                        rsx! {
                            div { style: "text-align:center;padding:48px 16px;",
                                p { style: "font-size:24px;", "🌿" }
                                p { style: "font-size:15px;color:#888;margin-top:12px;", "{loading_label}" }
                            }
                        }
                    },
                }
            }

            BottomNav { cart_count }
        }
    }
}

fn render_strain_card(strain: ApiStrain, mut cart: Signal<Cart>) -> Element {
    let add_to_cart_label = t(Lang::Russian, T_ADD_TO_CART).to_string();
    let cat = strain.category.as_deref().unwrap_or("Hybrid");
    let emoji = category_emoji(cat);
    let is_sotd = strain.is_strain_of_day;
    let discount = if strain.strain_of_day_discount.is_finite() {
        strain.strain_of_day_discount.max(0.0)
    } else {
        0.0
    };
    let price = if strain.price_per_gram.is_finite() {
        strain.price_per_gram.max(0.0)
    } else {
        0.0
    };
    // Marketing flags (TZ #2) — gated by expiry window so admin-set timers
    // actually expire in the UI without a page reload.
    let sale_live = strain.sale_active && is_active_until(strain.sale_until.as_deref());
    let new_live = strain.is_new_arrival && is_active_until(strain.new_until.as_deref());
    let is_best = strain.is_best_seller;

    // Cycle #55: math extracted to `trios::pricing::effective_strain_price`
    // so a future server-side price-authority check reuses the same function
    // (otherwise drift between client and server math flags every order as
    // fraud). 15 table-driven unit tests in `trios::pricing` cover the
    // precedence rules.
    let priced = crate::trios::pricing::effective_strain_price(
        &crate::trios::pricing::MarketingFlags {
            price_per_gram: strain.price_per_gram,
            is_strain_of_day: is_sotd,
            strain_of_day_discount: discount,
            sale_active: strain.sale_active,
            sale_until: strain.sale_until.as_deref(),
            sale_price: strain.sale_price,
            discount_percent: strain.discount_percent,
            is_new_arrival: strain.is_new_arrival,
            new_until: strain.new_until.as_deref(),
        },
        chrono::Utc::now(),
    );
    let effective_price = priced.price;
    let has_discount = priced.has_discount;

    // Border tints by the highest-priority flag — same precedence as the
    // priority CASE that drives sort order.
    let border_color = if is_sotd {
        "#ffe600"
    } else if new_live {
        "#00e5ff"
    } else if is_best {
        "#ff9d00"
    } else if sale_live {
        "#ff4757"
    } else {
        "#2a2a4a"
    };
    let card_style = format!(
        "background:#16213e;border:4px solid {};box-shadow:4px 4px 0 #000;overflow:hidden;position:relative;{}",
        border_color,
        if strain.is_available { "".to_string() } else { "opacity:0.6;".to_string() }
    );

    let display_price = format_price(effective_price);
    let original_price = format_price(price);

    let thc_str = strain
        .thc_percent
        .map(|t| format!("THC {:.0}%", t))
        .unwrap_or_default();
    let cbd_str = strain
        .cbd_percent
        .map(|c| format!("CBD {:.1}%", c))
        .unwrap_or_default();
    let effect_str = strain.effect.clone().unwrap_or_default();
    let flavor_str = strain.flavor_profile.clone().unwrap_or_default();
    let has_real_price = price > 0.0;

    let badge_style = category_badge_style(cat);
    let badge_label = format!("{} {}", emoji, cat);

    let img_url = strain.image_url.clone().unwrap_or_default();
    let has_image = !img_url.is_empty()
        && (img_url.starts_with("http://")
            || img_url.starts_with("https://")
            || (img_url.starts_with("/") && !img_url.starts_with("//")));
    let alt_name = strain.name.clone();
    let img_url_bust = if !has_image {
        String::new()
    } else {
        format!("{}?v=2", img_url)
    };
    let video_url = strain.video_url.clone().unwrap_or_default();
    let has_video = !video_url.is_empty()
        && (video_url.starts_with("http://")
            || video_url.starts_with("https://")
            || (video_url.starts_with("/") && !video_url.starts_with("//")));
    let mut show_video = use_signal(|| false);

    rsx! {
        div { key: strain.id.clone(), class: "comet-card", style: card_style,
            div { style: "width:100%;aspect-ratio:2/3;background:linear-gradient(135deg,#1a1a2e,#16213e);display:flex;align-items:center;justify-content:center;position:relative;overflow:hidden;",
                {if has_video {
                    rsx! {
                        video {
                            style: "width:100%;height:100%;object-fit:cover;display:block;",
                            src: "{video_url}",
                            autoplay: true,
                            muted: true,
                            loop: true,
                            // playsinline is REQUIRED for muted autoplay inside
                            // mobile WebViews (Telegram/iOS Safari). Without it the
                            // browser blocks autoplay, the <video> stays paused on a
                            // blank frame and the card looks like it has no video.
                            "playsinline": "true",
                            "webkit-playsinline": "true",
                            preload: "auto",
                            // poster shows the product image until the first video
                            // frame is painted, so there is never an empty box.
                            poster: if has_image { "{img_url}" } else { "" },
                            onclick: move |e: Event<MouseData>| { e.stop_propagation(); show_video.set(true); }
                        }
                    }
                } else if has_image {
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
                // TZ #2 badge stack: stacked top-left, ordered by visual priority.
                div { style: "position:absolute;top:8px;left:8px;display:flex;flex-direction:column;gap:4px;z-index:2;align-items:flex-start;",
                    {is_sotd.then(|| rsx! {
                        span { style: "
                            font-size:13px;font-weight:700;background:#ffe600;color:#000;
                            padding:4px 8px;box-shadow:2px 2px 0 #000;
                        ", "⭐ SOTD" }
                    })}
                    {new_live.then(|| rsx! {
                        span { style: "
                            font-size:13px;font-weight:700;background:#00e5ff;color:#000;
                            padding:4px 8px;box-shadow:2px 2px 0 #000;
                        ", "🆕 NEW" }
                    })}
                    {is_best.then(|| rsx! {
                        span { style: "
                            font-size:13px;font-weight:700;background:#ff9d00;color:#000;
                            padding:4px 8px;box-shadow:2px 2px 0 #000;
                        ", "⭐ BEST" }
                    })}
                    {sale_live.then(|| rsx! {
                        span { style: "
                            font-size:13px;font-weight:700;background:#ff4757;color:#fff;
                            padding:4px 8px;box-shadow:2px 2px 0 #000;
                        ", "🔥 SALE" }
                    })}
                }
                {if show_video() {
                    rsx! {
                        div { style: "position:fixed;inset:0;background:rgba(0,0,0,0.85);display:flex;align-items:center;justify-content:center;z-index:1000;padding:16px;",
                            onclick: move |_| show_video.set(false),
                            div { style: "background:#1a1a2e;padding:16px;border-radius:8px;max-width:90vw;max-height:80vh;display:flex;flex-direction:column;align-items:center;gap:8px;",
                                onclick: move |e: Event<MouseData>| e.stop_propagation(),
                                video { style: "max-width:100%;max-height:60vh;border-radius:6px;", controls: true, src: "{video_url}" }
                                button { style: "padding:8px 16px;background:#2a2a4a;color:#e8e8e8;border:none;border-radius:4px;cursor:pointer;",
                                    onclick: move |_| show_video.set(false), "Закрыть" }
                            }
                        }
                    }
                } else {
                    rsx! {}
                }}
            }
            div { style: "padding:12px;",
                div { style: "font-size:17px;font-weight:700;margin-bottom:6px;color:#fff;line-height:1.2;text-shadow:2px 2px 0 #000;",
                    "{strain.name}"
                }
                div { style: "display:flex;gap:6px;align-items:center;margin-bottom:6px;flex-wrap:wrap;",
                    span { style: badge_style, "{badge_label}" }
                    {(!thc_str.is_empty()).then(|| rsx! {
                        span { style: "font-size:13px;color:#39ff14;font-weight:700;", "{thc_str}" }
                    })}
                    {(!cbd_str.is_empty()).then(|| rsx! {
                        span { style: "font-size:13px;color:#00e5ff;font-weight:600;", "{cbd_str}" }
                    })}
                }
                {(!effect_str.is_empty()).then(|| rsx! {
                    div { style: "font-size:13px;color:#aaa;margin-bottom:4px;line-height:1.35;",
                        "{effect_str}"
                    }
                })}
                {(!flavor_str.is_empty()).then(|| rsx! {
                    div { style: "font-size:13px;color:#888;margin-bottom:8px;line-height:1.35;",
                        "🍃 {flavor_str}"
                    }
                })}
                div { style: "display:flex;gap:6px;align-items:baseline;margin-bottom:6px;",
                    {if has_real_price {
                        rsx! {
                            span { style: "font-size:20px;font-weight:800;color:#ffe600;text-shadow:2px 2px 0 #000;", "{display_price}" }
                            span { style: "font-size:13px;color:#888;", "/g" }
                            {has_discount.then(|| rsx! {
                                span { style: "font-size:13px;color:#888;text-decoration:line-through;margin-left:4px;",
                                    "{original_price}"
                                }
                            })}
                        }
                    } else {
                        rsx! {
                            span { style: "font-size:13px;color:#888;font-style:italic;", "Price on request" }
                        }
                    }}
                }
            }
            div { style: "padding:0 12px 12px;",
                {if strain.is_available {
                    // Reuse the same `effective_price` computed above so the
                    // cart line price matches the badge / strikethrough math.
                    let unit_price = effective_price;
                    let _ = has_discount; // kept for the price-display branch above
                    let s_id = strain.id.clone();
                    let s_name = strain.name.clone();
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
