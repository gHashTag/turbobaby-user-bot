use crate::trios::i18n::{t, T_ADD_TO_CART, T_FILTER_ALL, T_TEA_DESC, T_TEA_TITLE};
use crate::ui::api::context::api_base_url;
use crate::ui::components::bottom_nav::BottomNav;
use crate::ui::components::product_detail_modal::ProductDetailModal;
use crate::ui::share::{share_product, ProductKind, SharedProduct};
use crate::ui::state::{Cart, CartItem, CartItemType};
use dioxus::prelude::*;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
struct ApiTea {
    id: String,
    name: String,
    #[serde(default)]
    name_en: Option<String>,
    subcategory: Option<String>,
    #[serde(default)]
    subcategory_en: Option<String>,
    description: Option<String>,
    #[serde(default)]
    description_en: Option<String>,
    price: f64,
    stock: Option<i32>,
    image_url: Option<String>,
    video_url: Option<String>,
    is_available: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct TeaResponse {
    products: Vec<ApiTea>,
}

use crate::trios::drink_categories as dcat;

#[component]
pub fn TeaScreen() -> Element {
    let mut cart = use_context::<Signal<Cart>>();
    // Holds the active category's **canonical key** (or "All").
    let mut active_sub = use_signal(|| "All".to_string());
    // Tapping a tea card opens a detail popup with the full description.
    // Held at screen level (one modal) because the cards render inline in a
    // `for` loop, where a per-card `use_signal` would violate hook ordering.
    let mut selected_tea = use_signal(|| None::<ApiTea>);

    let tea_title = t(crate::ui::lang::current_lang(), T_TEA_TITLE);
    let tea_desc = t(crate::ui::lang::current_lang(), T_TEA_DESC);
    let filter_all = t(crate::ui::lang::current_lang(), T_FILTER_ALL);
    let add_to_cart = t(crate::ui::lang::current_lang(), T_ADD_TO_CART);

    let tea_resource = use_resource(|| async move {
        let base = api_base_url();
        let url = format!("{}/api/tea-products", base);
        crate::ui::api::local_client::LocalClient::new()
            .get(&url)
            .send()
            .await
            .map_err(|e| e.to_string())?
            .json::<TeaResponse>()
            .await
            .map(|r| r.products)
            .map_err(|e| e.to_string())
    });

    // Deep-link target: open the shared tea product modal once the catalog loads.
    let mut pending = use_context::<Signal<Option<SharedProduct>>>();
    use_effect(move || {
        let target = pending.read().clone();
        if let Some(target) = target {
            if target.kind == ProductKind::Tea {
                match &*tea_resource.read() {
                    Some(Ok(teas)) => {
                        if let Some(t) = teas.iter().find(|t| t.id == target.id).cloned() {
                            selected_tea.set(Some(t));
                        }
                        pending.set(None);
                    }
                    Some(Err(_)) => pending.set(None),
                    None => {}
                }
            }
        }
    });

    // Categories present in the live catalog, derived from the data (a category
    // exists iff a drink carries it). Drives the filter chips below.
    let categories = match &*tea_resource.read() {
        Some(Ok(teas)) => dcat::present_categories(teas.iter().map(|t| {
            (
                t.subcategory.as_deref().unwrap_or(""),
                t.subcategory_en.as_deref(),
            )
        })),
        _ => Vec::new(),
    };

    let filtered = match &*tea_resource.read() {
        Some(Ok(teas)) => {
            let key = active_sub();
            if key == "All" {
                teas.clone()
            } else {
                teas.iter()
                    .filter(|t| {
                        dcat::canonical_key(
                            t.subcategory.as_deref().unwrap_or(""),
                            t.subcategory_en.as_deref(),
                        ) == key
                    })
                    .cloned()
                    .collect()
            }
        }
        _ => Vec::new(),
    };

    rsx! {
        div { style: "
            min-height: 100vh;
            background: #0f0f1a;
            color: #e8e8e8;
            padding-bottom: 80px;
        ",
            div { style: "padding: 20px 16px 16px; text-align: center;",
                h1 { style: "font-size: 24px; font-weight: 800; color: #39ff14; text-shadow: 3px 3px 0 #000, 0 0 10px rgba(57,255,20,0.5); letter-spacing: 2px;", "{tea_title}" }
                p { style: "font-size: 13px; color: #8b8b9e; margin-top: 4px;", "{tea_desc}" }
            }

            // Category filter tabs (pill chips) — derived dynamically from the
            // catalog, so a newly-added category auto-appears here.
            div { style: "display: flex; gap: 6px; padding: 0 16px 12px; overflow-x: auto;",
                // "All" chip first.
                {
                    let is_active = active_sub() == "All";
                    let bg = if is_active { "#39ff14" } else { "transparent" };
                    let color = if is_active { "#000" } else { "#8b8b9e" };
                    let border = if is_active { "#39ff14" } else { "#2a2a4a" };
                    rsx! {
                        button {
                            style: "
                                font-size: 13px; padding: 6px 10px;
                                background: {bg}; color: {color};
                                border: 4px solid {border}; border-radius: 20px;
                                cursor: pointer; white-space: nowrap;
                            ",
                            onclick: move |_| active_sub.set("All".to_string()),
                            "{filter_all}"
                        }
                    }
                }
                for cat in categories.iter() {
                    {
                        let is_active = active_sub() == cat.key;
                        let bg = if is_active { "#39ff14" } else { "transparent" };
                        let color = if is_active { "#000" } else { "#8b8b9e" };
                        let border = if is_active { "#39ff14" } else { "#2a2a4a" };
                        let label = crate::ui::lang::localized(&cat.ru, Some(&cat.en));
                        let emoji = dcat::emoji(&cat.key);
                        let key_val = cat.key.clone();
                        rsx! {
                            button {
                                style: "
                                    font-size: 13px; padding: 6px 10px;
                                    background: {bg}; color: {color};
                                    border: 4px solid {border}; border-radius: 20px;
                                    cursor: pointer; white-space: nowrap;
                                ",
                                onclick: move |_| active_sub.set(key_val.clone()),
                                "{emoji} {label}"
                            }
                        }
                    }
                }
            }

            // Tea products grid
            {
                match &*tea_resource.read() {
                    Some(Ok(teas)) if teas.is_empty() => rsx! {
                        div { style: "text-align: center; padding: 40px 16px;",
                            div { style: "font-size: 70px; margin-bottom: 12px;", "🍵" }
                            p { style: "font-size: 13px; color: #8b8b9e;", "No tea products available yet" }
                        }
                    },
                    Some(Ok(_)) => rsx! {
                        div { style: "display: grid; grid-template-columns: 1fr 1fr; gap: 10px; padding: 0 16px;",
                            for tea in filtered.iter() {
                                {
                                    let t = tea.clone();
                                    let sub = t.subcategory.as_deref().unwrap_or("tea");
                                    let emoji = dcat::emoji(&dcat::canonical_key(sub, t.subcategory_en.as_deref()));
                                    let is_available = t.is_available.unwrap_or(true);
                                    let stock = t.stock.unwrap_or(999).max(0);
                                    let show_low_stock = stock > 0 && stock <= 5;
                                    let show_out_of_stock = stock == 0 || !is_available;
                                    let t_price = if t.price.is_finite() { t.price.max(0.0) } else { 0.0 };
                                    let price_str = crate::trios::pricing::format_baht(t_price);
                                    let t_name = crate::ui::lang::localized(&t.name, t.name_en.as_deref());
                                    let sub_disp = crate::ui::lang::localized(sub, t.subcategory_en.as_deref());
                                    let t_id = t.id.clone();
                                    let opacity = if show_out_of_stock { "0.6" } else { "1" };
                                    let desc = crate::ui::lang::localized(
                                        t.description.as_deref().unwrap_or(""),
                                        t.description_en.as_deref(),
                                    );
                                    let t_click = t.clone();

                                    rsx! {
                                        div { style: "
                                            background: #16213e; border: 4px solid #2a2a4a;
                                            border-radius: 0; overflow: hidden; box-shadow: 4px 4px 0 #000;
                                            opacity: {opacity}; cursor: pointer;
                                        ",
                                            onclick: move |_| selected_tea.set(Some(t_click.clone())),
                                            div { style: "
                                                min-height: 100px;
                                                background: linear-gradient(135deg, #16213e, #16213e);
                                                display: flex; align-items: center; justify-content: center;
                                                font-size: 36px; position: relative;
                                            ",
                                                if let Some(ref img) = t.image_url {
                                                    if !img.is_empty() && (img.starts_with("http://") || img.starts_with("https://") || (img.starts_with("/") && !img.starts_with("//"))) {
                                                        img { src: "{img}", alt: "{t_name}", style: "width: 100%; height: auto; object-fit: contain; display: block;" }
                                                    } else {
                                                        "{emoji}"
                                                    }
                                                } else {
                                                    "{emoji}"
                                                }
                                                if show_out_of_stock {
                                                    span { style: "
                                                        position: absolute; bottom: 4px; left: 50%;
                                                        transform: translateX(-50%);
                                                        font-size: 13px; background: #ff4757; color: white;
                                                        padding: 1px 6px; border-radius: 0;
                                                    ", "SOLD OUT" }
                                                }
                                                {if let Some(ref vid) = t.video_url {
                                                    if !vid.is_empty() && (vid.starts_with("http://") || vid.starts_with("https://") || (vid.starts_with("/") && !vid.starts_with("//"))) {
                                                        let vid = vid.clone();
                                                        rsx! {
                                                            a { href: "{vid}", target: "_blank", "aria-label": "Смотреть видео", style: "position:absolute;bottom:4px;right:4px;width:44px;height:44px;border-radius:50%;background:rgba(0,0,0,0.6);border:1px solid #fff;color:#fff;font-size:16px;display:flex;align-items:center;justify-content:center;cursor:pointer;z-index:2;text-decoration:none;",
                                                                onclick: move |e: Event<MouseData>| { e.stop_propagation(); }, "▶️" }
                                                        }
                                                    } else { rsx!{ "" } }
                                                } else { rsx!{ "" } }}
                                            }
                                            div { style: "padding: 8px;",
                                                div { style: "font-size: 13px; font-weight: 700; color: #b388ff; margin-bottom: 2px; text-transform: uppercase;", "{sub_disp}" }
                                                div { style: "font-size: 17px; font-weight: 700; margin-bottom: 2px;", "{t_name}" }
                                                if !desc.is_empty() {
                                                    div { style: "font-size: 13px; color: #8b8b9e; margin-bottom: 4px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;", "{desc}" }
                                                }
                                                if show_low_stock && !show_out_of_stock {
                                                    div { style: "font-size: 13px; color: #ffe600; margin-bottom: 4px;", "⚠ Only {stock} left" }
                                                }
                                                div { style: "font-size: 20px; font-weight: 800; color: #ffe600; text-shadow: 2px 2px 0 #000; margin-bottom: 6px;", "{price_str}" }
                                            }
                                            div { style: "padding: 0 8px 8px;",
                                                if show_out_of_stock {
                                                    button { style: "
                                                        font-size: 13px; width: 100%; padding: 8px;
                                                        background: transparent; color: #8b8b9e;
                                                        border: 4px solid #2a2a4a; border-radius: 0; cursor: not-allowed;
                                                    ", "Sold Out" }
                                                } else {
                                                    button {
                                                        style: "
                                                            font-size: 14px; font-weight: 700; width: 100%; padding: 8px;
                                                            background: #39ff14; color: #000;
                                                            border: 4px solid #2d9e0f; border-radius: 0; cursor: pointer;
                                                            box-shadow: 3px 3px 0 #000;
                                                            transition: transform 0.1s, box-shadow 0.1s;
                                                        ",
                                                        onclick: move |e: Event<MouseData>| {
                                                            e.stop_propagation();
                                                            let mut c = cart.write();
                                                            c.add_item(CartItem {
                                                                id: t_id.clone(),
                                                                name: t_name.clone(),
                                                                price: t_price,
                                                                quantity: 1,
                                                                image_url: None,
                                                                item_type: CartItemType::Tea,
                                                                fulfillment: None,
                                                            });
                                                            crate::ui::telegram::TelegramApp::init().haptic_notification(crate::ui::telegram::HapticNotification::Success);
                                                        },
                                                        "{add_to_cart}"
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    },
                    Some(Err(e)) => rsx! {
                        div { style: "text-align: center; padding: 40px 16px;",
                            div { style: "font-size: 70px; margin-bottom: 12px;", "⚠️" }
                            p { style: "font-size: 13px; color: #ff4757;", "Error: {e}" }
                        }
                    },
                    None => rsx! {
                        div { style: "display: grid; grid-template-columns: 1fr 1fr; gap: 10px; padding: 0 16px;",
                            for _ in 0..4 {
                                div { style: "
                                    background: #16213e; border: 4px solid #2a2a4a;
                                    border-radius: 0; padding: 20px; text-align: center;
                                    box-shadow: 4px 4px 0 #000; min-height: 140px;
                                ",
                                    div { style: "font-size: 13px; color: #8b8b9e;", "Loading..." }
                                }
                            }
                        }
                    },
                }
            }

            {selected_tea().map(|tea| {
                let sub = crate::ui::lang::localized(
                    tea.subcategory.as_deref().unwrap_or("tea"),
                    tea.subcategory_en.as_deref(),
                ).to_uppercase();
                let t_price = if tea.price.is_finite() { tea.price.max(0.0) } else { 0.0 };
                let price_str = crate::trios::pricing::format_baht(t_price);
                let add_id = tea.id.clone();
                let add_name = crate::ui::lang::localized(&tea.name, tea.name_en.as_deref());
                let avail = tea.is_available.unwrap_or(true) && tea.stock.unwrap_or(999) > 0;
                let share_id = tea.id.clone();
                let share_name = add_name.clone();
                rsx! {
                    ProductDetailModal {
                        name: crate::ui::lang::localized(&tea.name, tea.name_en.as_deref()),
                        image_url: tea.image_url.clone(),
                        description: crate::ui::lang::localized(
                            tea.description.as_deref().unwrap_or(""),
                            tea.description_en.as_deref(),
                        ),
                        category_badge: Some(sub),
                        price_line: Some(price_str),
                        can_add: avail,
                        add_to_cart_label: Some(format!("{add_to_cart}")),
                        on_add_to_cart: move |q: u32| {
                            cart.write().add_item(CartItem {
                                id: add_id.clone(),
                                name: add_name.clone(),
                                price: t_price,
                                quantity: q,
                                image_url: None,
                                item_type: CartItemType::Tea,
                                fulfillment: None,
                            });
                            crate::ui::telegram::TelegramApp::init().haptic_notification(crate::ui::telegram::HapticNotification::Success);
                        },
                        on_share: Some(EventHandler::new(move |_| share_product(ProductKind::Tea, &share_id, &share_name))),
                        on_close: move |_| selected_tea.set(None),
                    }
                }
            })}

            BottomNav {}
        }
    }
}
