use crate::trios::i18n::{t, T_ADD_TO_CART, T_SETS_DESC, T_SETS_TITLE};
use crate::ui::api::context::api_base_url;
use crate::ui::components::bottom_nav::BottomNav;
use crate::ui::components::card_media::CardMedia;
use crate::ui::components::product_detail_modal::ProductDetailModal;
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

    // Single selected set for both card taps and deep-link shares.
    // Held at screen level because the cards render inline in a `for` loop,
    // where a per-card `use_signal` would violate hook ordering when the list
    // is re-sorted.
    let mut selected_set = use_signal(|| None::<ApiSet>);
    let mut selected_set_video = use_signal(|| None::<String>);

    // Deep-link target: open the shared set modal once the catalog loads.
    let mut pending = use_context::<Signal<Option<SharedProduct>>>();
    use_effect(move || {
        let target = pending.read().clone();
        if let Some(target) = target {
            if target.kind == ProductKind::Set {
                match &*sets_resource.read() {
                    Some(Ok(sets)) => {
                        if let Some(s) = sets.iter().find(|s| s.id == target.id).cloned() {
                            selected_set.set(Some(s));
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
                            div { style: "display:grid;grid-template-columns:1fr 1fr;gap:12px;padding:0 16px;",
                                for set in ordered.iter() {
                                    {
                                        let s = set.clone();
                                        let s_select = s.clone();
                                        render_set_card(s, cart, move || selected_set.set(Some(s_select.clone())), move |url| selected_set_video.set(Some(url)))
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

fn render_set_card<S, V>(
    set: ApiSet,
    mut cart: Signal<Cart>,
    mut on_select: S,
    mut on_video: V,
) -> Element
where
    S: FnMut() + 'static,
    V: FnMut(String) + 'static,
{
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

    // Use the EXACT same block-root pattern as strain/accessory cards.
    // The only difference is that set metadata (weight, description) can be
    // missing, so we keep the same number of visual lines by rendering a
    // non-breaking space when a slot is empty. Each text slot is clamped to a
    // fixed line count so every card occupies the same vertical space.
    let weight_text = if weight_line.is_empty() {
        "\u{00A0}".to_string()
    } else {
        format!("⚖️ {}", weight_line)
    };
    let desc_text = if desc.is_empty() {
        "\u{00A0}".to_string()
    } else {
        desc.clone()
    };

    let card_style = format!(
        "background:#16213e;border:4px solid {};box-shadow:4px 4px 0 #000;overflow:hidden;position:relative;cursor:pointer;{}",
        ACCENT, opacity
    );

    rsx! {
        div { key: set.id.clone(), class: "comet-card", style: card_style,
            onclick: move |_| on_select(),
            CardMedia {
                image_url: set.image_url.clone(),
                video_url: set.video_url.clone(),
                emoji: icon.to_string(),
                alt: set_name.clone(),
                on_video_click: move |_| on_video(set.video_url.clone().unwrap_or_default()),
                // Badge stack exactly like strain/accessory cards: flex column,
                // top-left, with the SET tag on top and promo/discount below it.
                div { style: "position:absolute;top:8px;left:8px;display:flex;flex-direction:column;gap:4px;z-index:2;align-items:flex-start;",
                    span { style: "font-size:13px;font-weight:700;background:{ACCENT};color:#000;padding:4px 8px;box-shadow:2px 2px 0 #000;", "📦 SET" }
                    if let Some(bl) = badge_label.clone() {
                        span { style: "font-size:11px;font-weight:700;background:{badge_color};color:#fff;padding:3px 7px;box-shadow:2px 2px 0 #000;",
                            "{bl}"
                        }
                    }
                    if has_discount {
                        span { style: "font-size:13px;font-weight:700;background:{ACCENT};color:#000;padding:4px 8px;box-shadow:2px 2px 0 #000;",
                            "{discount_badge}" }
                    }
                }
            }
            div { style: "padding:12px;",
                div { style: "font-size:17px;font-weight:700;margin-bottom:6px;color:#fff;line-height:1.2;text-shadow:2px 2px 0 #000;display:-webkit-box;-webkit-line-clamp:2;-webkit-box-orient:vertical;overflow:hidden;min-height:2.4em;max-height:2.4em;",
                    "{set_name}"
                }
                div { style: "font-size:13px;color:#b388ff;font-weight:600;margin-bottom:4px;line-height:1.35;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;min-height:1.35em;max-height:1.35em;",
                    "{weight_text}"
                }
                div { style: "font-size:13px;color:#888;margin-bottom:8px;line-height:1.35;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;min-height:1.35em;max-height:1.35em;",
                    "{desc_text}"
                }
                div { style: "display:flex;gap:6px;align-items:baseline;margin-bottom:6px;min-height:1.2em;max-height:1.2em;",
                    span { style: "font-size:20px;font-weight:800;color:#ffe600;text-shadow:2px 2px 0 #000;", "{price_str}" }
                    {has_discount.then(|| rsx! {
                        span { style: "font-size:13px;color:#888;text-decoration:line-through;margin-left:4px;", "{original_price_str}" }
                    })}
                }
            }
            div { style: "padding:0 12px 12px;",
                {if is_available {
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
                                    fulfillment: None,
                                });
                                crate::ui::telegram::TelegramApp::init().haptic_notification(crate::ui::telegram::HapticNotification::Success);
                            },
                            "{add_to_cart} 🛒"
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
