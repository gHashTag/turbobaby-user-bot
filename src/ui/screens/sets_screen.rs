use crate::trios::i18n::{t, T_ADD_TO_CART, T_SETS_DESC, T_SETS_TITLE};
use crate::ui::api::context::api_base_url;
use crate::ui::components::bottom_nav::BottomNav;
use crate::ui::components::product_detail_modal::ProductDetailModal;
use crate::ui::components::set_card::{render_uniform_set_card, SetCardData};
use crate::ui::components::video_modal::VideoModal;
use crate::ui::share::{share_product, ProductKind, SharedProduct};
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
    let mut cart = use_context::<Signal<Cart>>();
    let mut sort_by = use_signal(|| "default".to_string());
    let mut selected_set = use_signal(|| None::<SetCardData>);
    let mut selected_set_video = use_signal(|| None::<String>);

    let sets_title = t(crate::ui::lang::current_lang(), T_SETS_TITLE);
    let sets_desc = t(crate::ui::lang::current_lang(), T_SETS_DESC);
    let add_to_cart = t(crate::ui::lang::current_lang(), T_ADD_TO_CART);

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

    // Deep-link target: open the shared set modal once the catalog loads.
    let mut pending = use_context::<Signal<Option<SharedProduct>>>();
    use_effect(move || {
        let target = pending.read().clone();
        if let Some(target) = target {
            if target.kind == ProductKind::Set {
                match &*sets_resource.read() {
                    Some(Ok(sets)) => {
                        if let Some(s) = sets.iter().find(|s| s.id == target.id).cloned() {
                            selected_set.set(Some(SetCardData {
                                id: s.id.clone(),
                                name: s.name.clone(),
                                name_en: s.name_en.clone(),
                                description: s.description.clone(),
                                description_en: s.description_en.clone(),
                                icon: s.icon.clone(),
                                total_price: s.total_price,
                                discount_percent: s.discount_percent,
                                image_url: s.image_url.clone(),
                                video_url: s.video_url.clone(),
                                is_available: s.is_available,
                                strain_count: s.strain_count,
                                total_weight_grams: s.total_weight_grams,
                                badge: s.badge.clone(),
                            }));
                        }
                        pending.set(None);
                    }
                    Some(Err(_)) => pending.set(None),
                    None => {}
                }
            }
        }
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
                        // Like the strains menu: a 2-column grid of cards. Promo
                        // packs (badge or discount) lead; within each group the
                        // chosen sort applies.
                        let (mut promo, mut rest): (Vec<ApiSet>, Vec<ApiSet>) = all_sets
                            .iter()
                            .cloned()
                            .partition(|s| crate::trios::packs::is_promo(&s.badge, s.discount_percent));
                        let active_sort = sort_by();
                        sort_packs(&mut promo, &active_sort);
                        sort_packs(&mut rest, &active_sort);
                        let mut ordered = promo;
                        ordered.extend(rest);
                        rsx! {
                            // Sort chips (like the strain filter row).
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
                            div { style: "display:grid;grid-template-columns:1fr 1fr;gap:12px;padding:0 16px;align-items:stretch;",
                                for set in ordered.iter() {
                                    {
                                        let s = set.clone();
                                        let s_select = s.clone();
                                        let c = cart;
                                        render_uniform_set_card(
                                            SetCardData {
                                                id: s.id.clone(),
                                                name: s.name.clone(),
                                                name_en: s.name_en.clone(),
                                                description: s.description.clone(),
                                                description_en: s.description_en.clone(),
                                                icon: s.icon.clone(),
                                                total_price: s.total_price,
                                                discount_percent: s.discount_percent,
                                                image_url: s.image_url.clone(),
                                                video_url: s.video_url.clone(),
                                                is_available: s.is_available,
                                                strain_count: s.strain_count,
                                                total_weight_grams: s.total_weight_grams,
                                                badge: s.badge.clone(),
                                            },
                                            c,
                                            ACCENT,
                                            move || selected_set.set(Some(SetCardData {
                                                id: s_select.id.clone(),
                                                name: s_select.name.clone(),
                                                name_en: s_select.name_en.clone(),
                                                description: s_select.description.clone(),
                                                description_en: s_select.description_en.clone(),
                                                icon: s_select.icon.clone(),
                                                total_price: s_select.total_price,
                                                discount_percent: s_select.discount_percent,
                                                image_url: s_select.image_url.clone(),
                                                video_url: s_select.video_url.clone(),
                                                is_available: s_select.is_available,
                                                strain_count: s_select.strain_count,
                                                total_weight_grams: s_select.total_weight_grams,
                                                badge: s_select.badge.clone(),
                                            })),
                                            move |url: String| selected_set_video.set(Some(url)),
                                        )
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

            // Selected set modal (card tap or deep link).
            {selected_set().map(|set| {
                let add_id = set.id.clone();
                let add_name = crate::ui::lang::localized(&set.name, set.name_en.as_deref());
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
                let price_str = crate::trios::pricing::format_baht(discounted_price);
                let avail = set.is_available.unwrap_or(true);
                let desc = crate::ui::lang::localized(
                    set.description.as_deref().unwrap_or(""),
                    set.description_en.as_deref(),
                );
                let share_name = add_name.clone();
                let share_id = set.id.clone();
                rsx! {
                    ProductDetailModal {
                        name: add_name.clone(),
                        image_url: set.image_url.clone(),
                        description: desc,
                        price_line: Some(price_str),
                        can_add: avail,
                        add_to_cart_label: Some(add_to_cart.to_string()),
                        on_add_to_cart: move |q: u32| {
                            cart.write().add_item(CartItem {
                                id: add_id.clone(),
                                name: add_name.clone(),
                                price: discounted_price,
                                quantity: q,
                                image_url: None,
                                item_type: CartItemType::Set,
                                fulfillment: None,
                            });
                            crate::ui::telegram::TelegramApp::init().haptic_notification(crate::ui::telegram::HapticNotification::Success);
                        },
                        on_share: Some(EventHandler::new(move |_| share_product(ProductKind::Set, &share_id, &share_name))),
                        on_close: move |_| selected_set.set(None),
                    }
                }
            })}

            {selected_set_video().map(|url| rsx! {
                VideoModal { url, on_close: move |_| selected_set_video.set(None) }
            })}

            BottomNav {}
        }
    }
}
