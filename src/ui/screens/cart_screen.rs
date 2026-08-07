// Cart Screen — Interactive with global Cart signal
use crate::trios::i18n::{
    t, tf,
    T_CART_BACK_MENU, T_CART_BROWSE_MENU, T_CART_BROWSE_SETS, T_CART_CHECKOUT,
    T_CART_DECREASE_QTY, T_CART_DELIVERY, T_CART_DELIVERY_FREE, T_CART_DINE_IN,
    T_CART_EMPTY, T_CART_EMPTY_DESC, T_CART_IMAGE_ALT, T_CART_ITEMS, T_CART_REMOVE,
    T_CART_SUBTOTAL, T_CART_TAKEAWAY, T_CART_TITLE, T_PLACE_ORDER, T_TOTAL,
};
use crate::ui::components::bottom_nav::BottomNav;
use crate::ui::components::empty_state::EmptyState;
use crate::ui::routes::Route;
use crate::ui::state::{Cart, CartItem, CartItemType};
use crate::ui::telegram::{TelegramApp, HapticNotification};
use dioxus::prelude::*;

fn format_price(price: f64) -> String {
    // Single source of truth (was identical in menu/home, narrowing to i32).
    crate::trios::pricing::format_baht(price)
}

#[component]
pub fn CartScreen() -> Element {
    let cart = use_context::<Signal<Cart>>();
    let items = cart.read().items.clone();
    let total = cart.read().total;
    let item_count: u32 = items.iter().map(|i| i.quantity).sum();
    let lang = crate::ui::lang::current_lang();
    let cart_title = t(lang, T_CART_TITLE);
    let cart_empty = t(lang, T_CART_EMPTY);
    let cart_empty_desc = t(lang, T_CART_EMPTY_DESC);
    let browse_menu = t(lang, T_CART_BROWSE_MENU);
    let browse_sets = t(lang, T_CART_BROWSE_SETS);
    let items_label = tf(lang, T_CART_ITEMS, &[item_count.to_string()]);
    let tg = TelegramApp::init();
    let total_str = crate::trios::pricing::format_baht(total);
    if !items.is_empty() {
        tg.set_main_button_text(&format!("{} — {}", t(lang, T_PLACE_ORDER), total_str));
        tg.show_back_button();
    } else {
        tg.hide_main_button();
        tg.hide_back_button();
    }
    // NOTE: MainButton has no reliable onclick bridge via document::eval; we keep
    // the in-app checkout button as the actionable element. Telegram MainButton here
    // acts as a visible price/status hint and back navigation affordance.

    rsx! {
        div { style: "
            min-height: 100vh;
            background: #0f0f1a;
            color: #e8e8e8;
            padding-bottom: 80px;
        ",
            // Header
            div { style: "padding: 20px 16px 16px; text-align: center;",
                h1 { style: "font-size: 24px; font-weight: 800; color: #39ff14; text-shadow: 3px 3px 0 #000, 0 0 10px rgba(57,255,20,0.5); letter-spacing: 2px;", "{cart_title}" }
                if !items.is_empty() {
                    p { style: "font-size: 15px; color: #8b8b9e; margin-top: 4px;", "{items_label}" }
                }
            }

            // Cart content
            div { style: "padding: 0 16px;",
                {
                    if items.is_empty() {
                        rsx! {
                            div { style: "padding: 40px 0;",
                                EmptyState {
                                    icon: "🛒".to_string(),
                                    title: cart_empty.to_string(),
                                    description: cart_empty_desc.to_string(),
                                    glass: true,
                                    action: rsx! {
                                        Link { to: Route::Menu {},
                                            button { style: "
                                                font-size: 15px; font-weight: 700; padding: 14px 20px;
                                                background: #39ff14; color: #000;
                                                border: 4px solid #2d9e0f; border-radius: 0;
                                                cursor: pointer; box-shadow: 3px 3px 0 #000;
                                                width: 100%; max-width: 320px;
                                            ", "{browse_menu} →" }
                                        }
                                    },
                                }
                                div { style: "text-align:center;margin-top:16px;",
                                    Link { to: Route::Sets {},
                                        button { style: "
                                            font-size: 15px; font-weight: 700; padding: 14px 20px;
                                            background: transparent; color: #e8e8e8;
                                            border: 4px solid #2a2a4a; border-radius: 0;
                                            cursor: pointer; box-shadow: 3px 3px 0 #000;
                                            width: 100%; max-width: 320px;
                                        ", "{browse_sets} →" }
                                    }
                                }
                            }
                        }
                    } else {
                        rsx! {
                            {
                                items.into_iter().map(|item| {
                                    cart_item_row(item, lang)
                                })
                            }

                            // Summary
                            div { style: "
                                background: #16213e; border: 4px solid #2a2a4a;
                                border-radius: 0; padding: 14px;
                                box-shadow: 4px 4px 0 #000; margin-top: 8px;
                            ",
                                div { style: "display: flex; justify-content: space-between; margin-bottom: 6px; font-size: 15px;",
                                    span { style: "color: #8b8b9e;", "{t(lang, T_CART_SUBTOTAL)}" }
                                    span { "{format_price(total)}" }
                                }
                                div { style: "display: flex; justify-content: space-between; margin-bottom: 6px; font-size: 15px;",
                                    span { style: "color: #8b8b9e;", "{t(lang, T_CART_DELIVERY)}" }
                                    span { style: "color: #39ff14;", "{t(lang, T_CART_DELIVERY_FREE)}" }
                                }
                                div { style: "display: flex; justify-content: space-between; font-size: 15px; font-weight: 800; padding-top: 6px; border-top: 1px solid #2a2a4a; margin-top: 6px;",
                                    span { "{t(lang, T_TOTAL)}" }
                                    span { style: "font-size: 20px; font-weight: 800; color: #ffe600; text-shadow: 2px 2px 0 #000;", "{format_price(total)}" }
                                }
                            }

                            // Actions
                            div { style: "display: flex; gap: 10px; margin-top: 16px;",
                                Link { to: Route::Menu {},
                                    button { style: "
                                        font-size: 14px; font-weight: 700; flex: 1; padding: 12px 20px;
                                        background: transparent; color: #e8e8e8;
                                        border: 4px solid #2a2a4a; border-radius: 0;
                                        cursor: pointer; box-shadow: 3px 3px 0 #000;
                                        transition: transform 0.1s, box-shadow 0.1s;
                                    ", "{t(lang, T_CART_BACK_MENU)}" }
                                }
                                Link { to: Route::Checkout {},
                                    button { style: "
                                        font-size: 14px; font-weight: 700; flex: 2; padding: 12px 20px;
                                        background: #39ff14; color: #000;
                                        border: 4px solid #2d9e0f; border-radius: 0;
                                        cursor: pointer; box-shadow: 3px 3px 0 #000;
                                        transition: transform 0.1s, box-shadow 0.1s;
                                    ", "{t(lang, T_CART_CHECKOUT)}" }
                                }
                            }
                        }
                    }
                }
            }

            BottomNav {}
        }
    }
}

fn cart_item_row(item: CartItem, lang: crate::trios::core::Lang) -> Element {
    let mut cart = use_context::<Signal<Cart>>();
    let item_id_for_minus = item.id.clone();
    let item_id_for_plus = item.id.clone();
    let item_id_for_remove = item.id.clone();
    let price_str = format_price(item.price);
    let qty = item.quantity;
    let line_total_str = format_price(item.price * item.quantity as f64);
    let row_key = item.id.clone();
    let image_alt = tf(lang, T_CART_IMAGE_ALT, &[item.name.clone()]);

    rsx! {
        div {
            key: "{row_key}",
            style: "
            display: flex; align-items: center; gap: 10px;
            background: #16213e; border: 4px solid #2a2a4a;
            border-radius: 0; padding: 10px; margin-bottom: 8px;
            box-shadow: 4px 4px 0 #000;
        ",
            if let Some(ref url) = item.image_url {
                img {
                    src: "{url}",
                    alt: "{image_alt}",
                    style: "width:44px;height:44px;object-fit:cover;background:#0f0f1a;border:2px solid #2a2a4a;flex-shrink:0;"
                }
            } else {
                div { style: "
                    width: 44px; height: 44px; background: #16213e;
                    border-radius: 0; display: flex; align-items: center;
                    justify-content: center; font-size: 22px; flex-shrink: 0;
                ", "🌿" }
            }
            div { style: "flex: 1;",
                div { style: "font-size: 17px; font-weight: 700; margin-bottom: 2px;", "{item.name}" }
                div { style: "font-size: 15px; color: #8b8b9e;", "{price_str} each · {line_total_str}" }
                // A3: per-drink dine-in / takeaway toggle (drinks only).
                if item.item_type == CartItemType::Tea {
                    {
                        let cur = item.fulfillment.clone().unwrap_or_else(|| "takeaway".to_string());
                        let din_active = cur == "dine_in";
                        let id_din = item.id.clone();
                        let id_take = item.id.clone();
                        let din_style = format!(
                            "font-size:12px;padding:4px 8px;border:3px solid {};background:{};color:{};border-radius:0;cursor:pointer;white-space:nowrap;",
                            if din_active { "#39ff14" } else { "#2a2a4a" },
                            if din_active { "#39ff14" } else { "transparent" },
                            if din_active { "#000" } else { "#8b8b9e" },
                        );
                        let take_style = format!(
                            "font-size:12px;padding:4px 8px;border:3px solid {};background:{};color:{};border-radius:0;cursor:pointer;white-space:nowrap;",
                            if !din_active { "#39ff14" } else { "#2a2a4a" },
                            if !din_active { "#39ff14" } else { "transparent" },
                            if !din_active { "#000" } else { "#8b8b9e" },
                        );
                        rsx! {
                            div { style: "display:flex;gap:6px;margin-top:6px;",
                                button {
                                    style: "{din_style}",
                                    onclick: move |_| {
                                        let mut c = cart.write();
                                        if let Some(it) = c.items.iter_mut().find(|i| i.id == id_din) {
                                            it.fulfillment = Some("dine_in".to_string());
                                        }
                                    },
                                    "{t(lang, T_CART_DINE_IN)}"
                                }
                                button {
                                    style: "{take_style}",
                                    onclick: move |_| {
                                        let mut c = cart.write();
                                        if let Some(it) = c.items.iter_mut().find(|i| i.id == id_take) {
                                            it.fulfillment = Some("takeaway".to_string());
                                        }
                                    },
                                    "{t(lang, T_CART_TAKEAWAY)}"
                                }
                            }
                        }
                    }
                }
            }
            div { style: "display: flex; align-items: center; gap: 6px;",
                button {
                    style: "
                        width: 44px; height: 44px;
                        border: 4px solid #2a2a4a; background: #16213e;
                        color: #e8e8e8; border-radius: 0; cursor: pointer;
                        font-size: 16px; display: flex; align-items: center; justify-content: center;
                        box-shadow: 3px 3px 0 #000;
                        transition: transform 0.1s, box-shadow 0.1s;
                    ",
                    "aria-label": "{t(lang, T_CART_DECREASE_QTY)}",
                    onclick: move |_| {
                        let mut c = cart.write();
                        if let Some(item) = c.items.iter_mut().find(|i| i.id == item_id_for_minus) {
                            if item.quantity > 1 {
                                item.quantity -= 1;
                            } else {
                                let id = item_id_for_minus.clone();
                                drop(c);
                                cart.write().remove_item(&id);
                                return;
                            }
                        }
                        c.recalculate_total();
                    },
                    "−"
                }
                span { style: "font-size: 15px; min-width: 18px; text-align: center;", "{qty}" }
                button {
                    style: "
                        width: 44px; height: 44px;
                        border: 4px solid #2a2a4a; background: #16213e;
                        color: #e8e8e8; border-radius: 0; cursor: pointer;
                        font-size: 16px; display: flex; align-items: center; justify-content: center;
                        box-shadow: 3px 3px 0 #000;
                        transition: transform 0.1s, box-shadow 0.1s;
                    ",
                    onclick: move |_| {
                        cart.write().add_item(CartItem {
                            id: item_id_for_plus.clone(),
                            name: item.name.clone(),
                            price: item.price,
                            quantity: 1,
                            image_url: None,
                            item_type: item.item_type.clone(),
                            fulfillment: None,
                        });
                        crate::ui::telegram::TelegramApp::init().haptic_notification(crate::ui::telegram::HapticNotification::Success);
                    },
                    "+"
                }
                button {
                    style: "
                        width: 44px; height: 44px;
                        border: 4px solid #ff4757; background: rgba(255,71,87,0.1);
                        color: #ff4757; border-radius: 0; cursor: pointer;
                        font-size: 16px; display: flex; align-items: center; justify-content: center;
                        box-shadow: 3px 3px 0 #000;
                        transition: transform 0.1s, box-shadow 0.1s;
                    ",
                    "aria-label": "{t(lang, T_CART_REMOVE)}",
                    onclick: move |_| {
                        let id = item_id_for_remove.clone();
                        cart.write().remove_item(&id);
                        TelegramApp::init().haptic_notification(HapticNotification::Warning);
                    },
                    "🗑"
                }
            }
        }
    }
}
