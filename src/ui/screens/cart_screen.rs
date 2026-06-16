// Cart Screen — Interactive with global Cart signal
use crate::trios::i18n::{t, T_CART_EMPTY, T_CART_EMPTY_DESC, T_CART_TITLE};
use crate::ui::components::bottom_nav::BottomNav;
use crate::ui::routes::Route;
use crate::ui::state::{Cart, CartItem};
use dioxus::prelude::*;

fn format_price(price: f64) -> String {
    let v = if price.is_finite() {
        price.max(0.0)
    } else {
        0.0
    };
    format!("฿{}", v as i32)
}

#[component]
pub fn CartScreen() -> Element {
    let cart = use_context::<Signal<Cart>>();
    let items = cart.read().items.clone();
    let total = cart.read().total;
    let item_count: u32 = items.iter().map(|i| i.quantity).sum();
    let cart_title = t(crate::ui::lang::current_lang(), T_CART_TITLE);
    let cart_empty = t(crate::ui::lang::current_lang(), T_CART_EMPTY);
    let cart_empty_desc = t(crate::ui::lang::current_lang(), T_CART_EMPTY_DESC);

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
                    p { style: "font-size: 15px; color: #8b8b9e; margin-top: 4px;", "{item_count} items" }
                }
            }

            // Cart content
            div { style: "padding: 0 16px;",
                {
                    if items.is_empty() {
                        rsx! {
                            div { style: "text-align: center; padding: 60px 16px;",
                                p { style: "font-size: 70px; margin-bottom: 16px;", "🛒" }
                                p { style: "font-size: 13px; color: #8b8b9e; margin-bottom: 8px;", "{cart_empty}" }
                                p { style: "font-size: 15px; color: #8b8b9e;", "{cart_empty_desc}" }
                            }
                        }
                    } else {
                        rsx! {
                            {
                                items.into_iter().map(|item| {
                                    cart_item_row(item)
                                })
                            }

                            // Summary
                            div { style: "
                                background: #16213e; border: 4px solid #2a2a4a;
                                border-radius: 0; padding: 14px;
                                box-shadow: 4px 4px 0 #000; margin-top: 8px;
                            ",
                                div { style: "display: flex; justify-content: space-between; margin-bottom: 6px; font-size: 15px;",
                                    span { style: "color: #8b8b9e;", "Subtotal:" }
                                    span { "{format_price(total)}" }
                                }
                                div { style: "display: flex; justify-content: space-between; margin-bottom: 6px; font-size: 15px;",
                                    span { style: "color: #8b8b9e;", "Delivery:" }
                                    span { style: "color: #39ff14;", "Free" }
                                }
                                div { style: "display: flex; justify-content: space-between; font-size: 15px; font-weight: 800; padding-top: 6px; border-top: 1px solid #2a2a4a; margin-top: 6px;",
                                    span { "Total:" }
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
                                    ", "← Menu" }
                                }
                                Link { to: Route::Checkout {},
                                    button { style: "
                                        font-size: 14px; font-weight: 700; flex: 2; padding: 12px 20px;
                                        background: #39ff14; color: #000;
                                        border: 4px solid #2d9e0f; border-radius: 0;
                                        cursor: pointer; box-shadow: 3px 3px 0 #000;
                                        transition: transform 0.1s, box-shadow 0.1s;
                                    ", "Checkout →" }
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

fn cart_item_row(item: CartItem) -> Element {
    let mut cart = use_context::<Signal<Cart>>();
    let item_id_for_minus = item.id.clone();
    let item_id_for_plus = item.id.clone();
    let price_str = format_price(item.price);
    let qty = item.quantity;
    let line_total_str = format_price(item.price * item.quantity as f64);
    let row_key = item.id.clone();

    rsx! {
        div {
            key: "{row_key}",
            style: "
            display: flex; align-items: center; gap: 10px;
            background: #16213e; border: 4px solid #2a2a4a;
            border-radius: 0; padding: 10px; margin-bottom: 8px;
            box-shadow: 4px 4px 0 #000;
        ",
            div { style: "
                width: 44px; height: 44px; background: #16213e;
                border-radius: 0; display: flex; align-items: center;
                justify-content: center; font-size: 22px; flex-shrink: 0;
            ", "🌿" }
            div { style: "flex: 1;",
                div { style: "font-size: 17px; font-weight: 700; margin-bottom: 2px;", "{item.name}" }
                div { style: "font-size: 15px; color: #8b8b9e;", "{price_str} each · {line_total_str}" }
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
                    "aria-label": "Убавить количество",
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
                        });
                        crate::ui::telegram::TelegramApp::init().haptic_notification(crate::ui::telegram::HapticNotification::Success);
                    },
                    "+"
                }
            }
        }
    }
}
