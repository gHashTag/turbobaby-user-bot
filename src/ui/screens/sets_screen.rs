use crate::trios::i18n::{t, T_ADD_TO_CART, T_SETS_DESC, T_SETS_TITLE};
use crate::ui::api::context::api_base_url;
use crate::ui::components::bottom_nav::BottomNav;
use crate::ui::components::product_detail_modal::ProductDetailModal;
use crate::ui::components::video_modal::VideoModal;
use crate::ui::state::{Cart, CartItem, CartItemType};
use dioxus::prelude::*;
use serde::Deserialize;

// Fixed accent for the sets theme. Previously each card derived its colour from a
// per-set "mood", but the API never carried mood data (the `target_mood`/
// `time_of_day`/`strains` fields and the whole mood filter were dead: 4 of 5
// filter chips matched nothing and every card was mislabelled "party"). The dead
// filter was removed; one accent keeps the look consistent.
const ACCENT: &str = "#b388ff";

#[derive(Debug, Clone, Deserialize)]
struct ApiSet {
    id: String,
    name: String,
    #[serde(default)]
    name_en: Option<String>,
    description: Option<String>,
    #[serde(default)]
    description_en: Option<String>,
    icon: Option<String>,
    total_price: f64,
    discount_percent: f64,
    image_url: Option<String>,
    #[serde(default)]
    video_url: Option<String>,
    is_available: Option<bool>,
    // Packs Phase 1 (migration 040). Defaulted so non-`sets` sources
    // (accessory_sets/tea_sets, which omit them) still deserialize.
    #[serde(default)]
    strain_count: usize,
    #[serde(default)]
    total_weight_grams: f64,
    #[serde(default)]
    badge: String,
    // Packs Phase 2 (sorting).
    #[serde(default)]
    created_at_epoch: f64,
    #[serde(default)]
    popularity: i64,
}

#[derive(Debug, Deserialize)]
struct SetsResponse {
    sets: Vec<ApiSet>,
}

/// Sort options for the "Все наборы" list. `(key, ru, en)`.
const SORTS: &[(&str, &str, &str)] = &[
    ("default", "По умолчанию", "Default"),
    ("popularity", "Популярность", "Popular"),
    ("price", "Цена", "Price"),
    ("new", "Новинки", "New"),
    ("discount", "Скидки", "Discount"),
];

/// Effective (discounted) price, clamped finite & non-negative.
fn effective_price(s: &ApiSet) -> f64 {
    let d = if s.discount_percent.is_finite() {
        s.discount_percent.max(0.0)
    } else {
        0.0
    };
    let t = if s.total_price.is_finite() {
        s.total_price.max(0.0)
    } else {
        0.0
    };
    (t * (1.0 - d / 100.0)).max(0.0)
}

/// Stable in-place sort of packs by the chosen key (no-op for "default").
fn sort_packs(v: &mut [ApiSet], key: &str) {
    use std::cmp::Ordering;
    match key {
        // Cheapest first.
        "price" => v.sort_by(|a, b| {
            effective_price(a)
                .partial_cmp(&effective_price(b))
                .unwrap_or(Ordering::Equal)
        }),
        // Newest first.
        "new" => v.sort_by(|a, b| {
            b.created_at_epoch
                .partial_cmp(&a.created_at_epoch)
                .unwrap_or(Ordering::Equal)
        }),
        // Biggest discount first.
        "discount" => v.sort_by(|a, b| {
            b.discount_percent
                .partial_cmp(&a.discount_percent)
                .unwrap_or(Ordering::Equal)
        }),
        // Most-ordered first.
        "popularity" => v.sort_by(|a, b| b.popularity.cmp(&a.popularity)),
        _ => {}
    }
}

#[component]
pub fn SetsScreen() -> Element {
    let cart = use_context::<Signal<Cart>>();
    let mut sort_by = use_signal(|| "default".to_string());

    let sets_title = t(crate::ui::lang::current_lang(), T_SETS_TITLE);
    let sets_desc = t(crate::ui::lang::current_lang(), T_SETS_DESC);
    let _add_to_cart = t(crate::ui::lang::current_lang(), T_ADD_TO_CART);

    let sets_resource = use_resource(|| async move {
        let base = api_base_url();
        let url = format!("{}/api/sets", base);
        crate::ui::api::local_client::LocalClient::new()
            .get(&url)
            .send()
            .await
            .map_err(|e| e.to_string())?
            .json::<SetsResponse>()
            .await
            .map(|r| r.sets)
            .map_err(|e| e.to_string())
    });

    let all_sets = match &*sets_resource.read() {
        Some(Ok(sets)) => sets.clone(),
        _ => Vec::new(),
    };

    rsx! {
        div { style: "min-height:100vh;background:#0f0f1a;color:#e8e8e8;padding-bottom:80px;",

            div { style: "padding:20px 16px 16px;text-align:center;",
                h1 { style: "font-size:24px;font-weight:800;color:#b388ff;text-shadow:3px 3px 0 #000,0 0 10px rgba(179,136,255,0.5);letter-spacing:2px;",
                    "{sets_title}"
                }
                p { style: "font-size:13px;color:#888;margin-top:4px;", "{sets_desc}" }
            }

            {
                match &*sets_resource.read() {
                    Some(Ok(sets)) if sets.is_empty() => rsx! {
                        div { style: "text-align:center;padding:48px 16px;",
                            div { style: "font-size:36px;margin-bottom:12px;", "📦" }
                            p { style: "font-size:15px;color:#888;", "No sets available yet" }
                        }
                    },
                    Some(Ok(_)) => {
                        // Promo packs (a badge OR a discount) lead in a swipeable
                        // carousel; everything else lists below. Stable partition
                        // keeps the admin order within each group.
                        let (promo, mut rest): (Vec<ApiSet>, Vec<ApiSet>) = all_sets
                            .iter()
                            .cloned()
                            .partition(|s| crate::trios::packs::is_promo(&s.badge, s.discount_percent));
                        // Phase 2: sort the "Все наборы" list by the chosen key.
                        let active_sort = sort_by();
                        sort_packs(&mut rest, &active_sort);
                        let promo_count = promo.len();
                        let rest_n = rest.len();
                        let promo_hdr = crate::ui::lang::localized("🔥 Акции и спецпредложения", Some("🔥 Sale & Special"));
                        let rest_hdr = crate::ui::lang::localized("Все наборы", Some("All packs"));
                        rsx! {
                            if promo_count > 0 {
                                div { style: "padding:0 16px 8px;",
                                    h2 { style: "font-size:13px;font-weight:700;color:#b388ff;text-transform:uppercase;letter-spacing:1px;margin-bottom:12px;text-shadow:2px 2px 0 #000;",
                                        "{promo_hdr}"
                                    }
                                }
                                div { style: "display:flex;overflow-x:auto;scroll-snap-type:x mandatory;-webkit-overflow-scrolling:touch;gap:12px;padding:0 16px 8px;",
                                    for set in promo.iter() {
                                        div { style: "flex:0 0 85%;scroll-snap-align:center;box-sizing:border-box;",
                                            { render_set_card(set.clone(), cart) }
                                        }
                                    }
                                }
                                if promo_count > 1 {
                                    div { style: "display:flex;justify-content:center;gap:6px;margin:0 0 14px;",
                                        for _i in 0..promo_count {
                                            span { style: "width:8px;height:8px;border-radius:50%;background:#b388ff;opacity:0.55;box-shadow:1px 1px 0 #000;" }
                                        }
                                    }
                                }
                            }
                            if rest_n > 0 {
                                div { style: "padding:0 16px 8px;",
                                    h2 { style: "font-size:13px;font-weight:700;color:#b388ff;text-transform:uppercase;letter-spacing:1px;margin-bottom:12px;text-shadow:2px 2px 0 #000;",
                                        "{rest_hdr} ({rest_n})"
                                    }
                                }
                                // Sort chips (Phase 2).
                                div { style: "display:flex;gap:6px;padding:0 16px 12px;overflow-x:auto;",
                                    for (key, ru, en) in SORTS.iter() {
                                        {
                                            let is_active = active_sort == *key;
                                            let bg = if is_active { "#b388ff" } else { "transparent" };
                                            let color = if is_active { "#000" } else { "#8b8b9e" };
                                            let border = if is_active { "#b388ff" } else { "#2a2a4a" };
                                            let label = crate::ui::lang::localized(ru, Some(en));
                                            let k = key.to_string();
                                            rsx! {
                                                button {
                                                    style: "font-size:13px;padding:6px 10px;background:{bg};color:{color};border:4px solid {border};border-radius:20px;cursor:pointer;white-space:nowrap;",
                                                    onclick: move |_| sort_by.set(k.clone()),
                                                    "{label}"
                                                }
                                            }
                                        }
                                    }
                                }
                                div { style: "display:flex;flex-direction:column;gap:12px;padding:0 16px;",
                                    for set in rest.iter() {
                                        { render_set_card(set.clone(), cart) }
                                    }
                                }
                            }
                        }
                    },
                    Some(Err(e)) => rsx! {
                        div { style: "text-align:center;padding:48px 16px;",
                            div { style: "font-size:36px;margin-bottom:12px;", "⚠️" }
                            p { style: "font-size:15px;color:#ff4757;", "Error: {e}" }
                        }
                    },
                    None => rsx! {
                        div { style: "display:flex;flex-direction:column;gap:12px;padding:0 16px;",
                            div { style: "
                                background:#16213e;border:4px solid #2a2a4a;
                                box-shadow:4px 4px 0 #000;
                                padding:20px;text-align:center;min-height:120px;
                            ",
                                div { style: "font-size:15px;color:#b388ff;margin-bottom:8px;", "Loading sets..." }
                            }
                        }
                    },
                }
            }

            BottomNav {}
        }
    }
}

fn render_set_card(set: ApiSet, mut cart: Signal<Cart>) -> Element {
    let add_to_cart = t(crate::ui::lang::current_lang(), T_ADD_TO_CART).to_string();
    let discount = if set.discount_percent.is_finite() {
        set.discount_percent.max(0.0)
    } else {
        0.0
    };
    let total_price = if set.total_price.is_finite() {
        set.total_price.max(0.0)
    } else {
        0.0
    };
    let has_discount = discount > 0.0;
    let discounted_price = if has_discount {
        (total_price * (1.0 - discount / 100.0)).max(0.0)
    } else {
        total_price
    };
    let original_price_str = crate::trios::pricing::format_baht(total_price);
    let price_str = crate::trios::pricing::format_baht(discounted_price);
    let discount_badge = if has_discount {
        format!("{}% OFF", discount as i32)
    } else {
        String::new()
    };
    let set_name = crate::ui::lang::localized(&set.name, set.name_en.as_deref());
    let set_id = set.id.clone();
    let icon = set.icon.as_deref().unwrap_or("🎁");
    let desc = crate::ui::lang::localized(
        set.description.as_deref().unwrap_or(""),
        set.description_en.as_deref(),
    );
    let img_url = set.image_url.clone().unwrap_or_default();
    let has_image = !img_url.is_empty()
        && (img_url.starts_with("http://")
            || img_url.starts_with("https://")
            || (img_url.starts_with("/") && !img_url.starts_with("//")));
    let video_url = set.video_url.clone().unwrap_or_default();
    let has_video = !video_url.is_empty()
        && (video_url.starts_with("http://")
            || video_url.starts_with("https://")
            || (video_url.starts_with("/") && !video_url.starts_with("//")));
    let mut show_video = use_signal(|| false);
    let mut detail_open = use_signal(|| false);
    let desc_full = desc.to_string();
    let is_available = set.is_available.unwrap_or(true);
    let opacity = if is_available { "" } else { "opacity:0.6;" };
    // Packs Phase 1: promo badge chip + weight/strain-count meta line.
    let badge = crate::trios::packs::PackBadge::from_str(&set.badge);
    let badge_label = badge
        .label()
        .map(|(ru, en)| crate::ui::lang::localized(ru, Some(en)));
    let badge_color = badge.color();
    let strain_word = crate::ui::lang::localized("сортов", Some("strains"));
    let weight_line =
        crate::trios::packs::weight_line(set.total_weight_grams, set.strain_count, &strain_word);

    rsx! {
        div { style: "
            background:#16213e;
            border:4px solid {ACCENT};
            box-shadow:4px 4px 0 #000;
            overflow:hidden;
            position:relative;
            cursor:pointer;
            {opacity}
        ",
            onclick: move |_| detail_open.set(true),
            div { style: "
                min-height:100px;
                background:linear-gradient(135deg,#1a1a2e,#16213e);
                display:flex;align-items:center;justify-content:center;
                font-size:40px;position:relative;
            ",
                if has_image {
                    img { src: "{img_url}", alt: "{set_name}", style: "width:100%;height:auto;object-fit:contain;display:block;" }
                } else {
                    "{icon}"
                }
                // On-image badge (top-left), mirroring strain/accessory cards.
                span { style: "position:absolute;top:8px;left:8px;z-index:2;font-size:13px;font-weight:700;background:{ACCENT};color:#000;padding:4px 8px;box-shadow:2px 2px 0 #000;",
                    "📦 SET"
                }
                // Promo badge (SALE / SPECIAL OFFER / LIMITED EDITION), under the SET tag.
                if let Some(bl) = badge_label.clone() {
                    span { style: "position:absolute;top:38px;left:8px;z-index:2;font-size:11px;font-weight:700;background:{badge_color};color:#fff;padding:3px 7px;box-shadow:2px 2px 0 #000;",
                        "{bl}"
                    }
                }
                if has_discount {
                    span { style: "
                        position:absolute;top:8px;right:8px;
                        font-size:13px;font-weight:700;background:{ACCENT};color:#000;
                        padding:4px 8px;box-shadow:2px 2px 0 #000;z-index:2;
                    ", "{discount_badge}" }
                }
                if has_video {
                    button { style: "position:absolute;bottom:8px;right:8px;width:44px;height:44px;border-radius:50%;background:rgba(0,0,0,0.6);border:1px solid #fff;color:#fff;font-size:16px;display:flex;align-items:center;justify-content:center;cursor:pointer;z-index:2;",
                        "aria-label": "Смотреть видео",
                        onclick: move |e: Event<MouseData>| { e.stop_propagation(); show_video.set(true); }, "▶️" }
                }
                if show_video() {
                    VideoModal { url: video_url.clone(), on_close: move |_| show_video.set(false) }
                }
            }
            div { style: "padding:14px;",
                div { style: "display:flex;justify-content:space-between;align-items:center;margin-bottom:4px;",
                    span { style: "font-size:16px;font-weight:700;text-shadow:2px 2px 0 #000;", "{set_name}" }
                }
                if !weight_line.is_empty() {
                    div { style: "font-size:12px;color:#b388ff;font-weight:600;margin-bottom:6px;", "⚖️ {weight_line}" }
                }
                if !desc.is_empty() {
                    div { style: "font-size:13px;color:#888;margin-bottom:6px;", "{desc}" }
                }
                div { style: "display:flex;justify-content:space-between;align-items:center;",
                    div {
                        if has_discount {
                            span { style: "font-size:13px;color:#888;text-decoration:line-through;margin-right:6px;", "{original_price_str}" }
                        }
                        span { style: "font-size:22px;font-weight:800;color:#ffe600;text-shadow:2px 2px 0 #000;", "{price_str}" }
                    }
                }
            }
            div { style: "padding:0 14px 14px;",
                {if is_available {
                    rsx! {
                        button {
                            style: "
                                font-size:14px;font-weight:700;width:100%;padding:12px 20px;
                                background:#39ff14;color:#000;
                                border:4px solid #2d9e0f;
                                box-shadow:3px 3px 0 #000;
                                cursor:pointer;
                            ",
                            onclick: move |e: Event<MouseData>| {
                                e.stop_propagation();
                                let mut c = cart.write();
                                c.add_item(CartItem {
                                    id: set_id.clone(),
                                    name: set_name.clone(),
                                    price: discounted_price,
                                    quantity: 1,
                                    image_url: None,
                                    item_type: CartItemType::Set,
                                });
                                crate::ui::telegram::TelegramApp::init().haptic_notification(crate::ui::telegram::HapticNotification::Success);
                            },
                            "{add_to_cart}"
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
            {detail_open().then(|| {
                let add_id = set.id.clone();
                let add_name = crate::ui::lang::localized(&set.name, set.name_en.as_deref());
                let add_price = discounted_price;
                let avail = is_available;
                rsx! {
                    ProductDetailModal {
                        name: crate::ui::lang::localized(&set.name, set.name_en.as_deref()),
                        image_url: set.image_url.clone(),
                        description: desc_full.clone(),
                        price_line: Some(price_str.clone()),
                        can_add: avail,
                        add_to_cart_label: Some(add_to_cart.clone()),
                        on_add_to_cart: move |q: u32| {
                            cart.write().add_item(CartItem {
                                id: add_id.clone(),
                                name: add_name.clone(),
                                price: add_price,
                                quantity: q,
                                image_url: None,
                                item_type: CartItemType::Set,
                            });
                            crate::ui::telegram::TelegramApp::init().haptic_notification(crate::ui::telegram::HapticNotification::Success);
                        },
                        on_close: move |_| detail_open.set(false),
                    }
                }
            })}
        }
    }
}
