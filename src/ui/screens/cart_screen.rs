// Cart Screen — Interactive with global Cart signal
use dioxus::prelude::*;
use crate::ui::routes::Route;
use crate::ui::state::{Cart, CartItem};

fn format_price(price: f64) -> String {
    format!("฿{}", price as i32)
}

#[component]
pub fn CartScreen() -> Element {
    let cart = use_context::<Signal<Cart>>();
    let items = cart.read().items.clone();
    let total = cart.read().total;
    let item_count: u32 = items.iter().map(|i| i.quantity).sum();

    rsx! {
        div { style: "
            min-height: 100vh;
            background: #0f0f1a;
            color: #e8e8e8;
            font-family: 'Press Start 2P', monospace;
            padding-bottom: 80px;
        ",
            // Header
            div { style: "padding: 20px 16px 12px; text-align: center;",
                h1 { style: "font-size: 12px; color: #39ff14; text-shadow: 0 0 8px rgba(57,255,20,0.5);", "🛒 Cart" }
                if !items.is_empty() {
                    p { style: "font-size: 7px; color: #8b8b9e; margin-top: 4px;", "{item_count} items" }
                }
            }

            // Cart content
            div { style: "padding: 0 16px;",
                {
                    if items.is_empty() {
                        rsx! {
                            div { style: "text-align: center; padding: 60px 16px;",
                                p { style: "font-size: 32px; margin-bottom: 16px;", "🛒" }
                                p { style: "font-size: 9px; color: #8b8b9e; margin-bottom: 8px;", "Your cart is empty" }
                                p { style: "font-size: 7px; color: #8b8b9e;", "Browse the menu to add strains" }
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
                                background: #16213e; border: 2px solid #2a2a4a;
                                border-radius: 8px; padding: 14px;
                                box-shadow: 4px 4px 0 #000; margin-top: 8px;
                            ",
                                div { style: "display: flex; justify-content: space-between; margin-bottom: 6px; font-size: 9px;",
                                    span { style: "color: #8b8b9e;", "Subtotal:" }
                                    span { "{format_price(total)}" }
                                }
                                div { style: "display: flex; justify-content: space-between; margin-bottom: 6px; font-size: 9px;",
                                    span { style: "color: #8b8b9e;", "Delivery:" }
                                    span { style: "color: #39ff14;", "Free" }
                                }
                                div { style: "display: flex; justify-content: space-between; font-size: 10px; font-weight: bold; padding-top: 6px; border-top: 1px solid #2a2a4a; margin-top: 6px;",
                                    span { "Total:" }
                                    span { style: "color: #39ff14;", "{format_price(total)}" }
                                }
                            }

                            // Actions
                            div { style: "display: flex; gap: 10px; margin-top: 16px;",
                                Link { to: Route::Menu {},
                                    button { style: "
                                        font-family: 'Press Start 2P', monospace;
                                        font-size: 7px; flex: 1; padding: 10px;
                                        background: transparent; color: #e8e8e8;
                                        border: 2px solid #2a2a4a; border-radius: 6px;
                                        cursor: pointer;
                                    ", "← Menu" }
                                }
                                Link { to: Route::Checkout {},
                                    button { style: "
                                        font-family: 'Press Start 2P', monospace;
                                        font-size: 7px; flex: 2; padding: 10px;
                                        background: #39ff14; color: #0f0f1a;
                                        border: none; border-radius: 6px;
                                        cursor: pointer; font-weight: bold;
                                    ", "Checkout →" }
                                }
                            }
                        }
                    }
                }
            }

            // Bottom Navigation
            {bottom_nav()}
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
    let _row_key = item.id.clone();

    rsx! {
        div {
            key: row_key,
            style: "
            display: flex; align-items: center; gap: 10px;
            background: #16213e; border: 2px solid #2a2a4a;
            border-radius: 8px; padding: 10px; margin-bottom: 8px;
            box-shadow: 4px 4px 0 #000;
        ",
            div { style: "
                width: 44px; height: 44px; background: #1a1a2e;
                border-radius: 4px; display: flex; align-items: center;
                justify-content: center; font-size: 22px; flex-shrink: 0;
            ", "🌿" }
            div { style: "flex: 1;",
                div { style: "font-size: 9px; font-weight: bold; margin-bottom: 2px;", "{item.name}" }
                div { style: "font-size: 7px; color: #8b8b9e;", "{price_str} each · {line_total_str}" }
            }
            div { style: "display: flex; align-items: center; gap: 6px;",
                button {
                    style: "
                        width: 26px; height: 26px;
                        border: 2px solid #2a2a4a; background: #1a1a2e;
                        color: #e8e8e8; border-radius: 4px; cursor: pointer;
                        font-size: 14px; display: flex; align-items: center; justify-content: center;
                        font-family: 'Press Start 2P', monospace;
                    ",
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
                span { style: "font-size: 9px; min-width: 18px; text-align: center;", "{qty}" }
                button {
                    style: "
                        width: 26px; height: 26px;
                        border: 2px solid #2a2a4a; background: #1a1a2e;
                        color: #e8e8e8; border-radius: 4px; cursor: pointer;
                        font-size: 14px; display: flex; align-items: center; justify-content: center;
                        font-family: 'Press Start 2P', monospace;
                    ",
                    onclick: move |_| {
                        cart.write().add_item(CartItem {
                            id: item_id_for_plus.clone(),
                            name: item.name.clone(),
                            price: item.price,
                            quantity: 1,
                            image_url: None,
                        });
                    },
                    "+"
                }
            }
        }
    }
}

fn bottom_nav() -> Element {
    rsx! {
        nav { style: "
            position: fixed; bottom: 0; left: 0; right: 0;
            background: #1a1a2e; border-top: 2px solid #2a2a4a;
            display: flex; justify-content: space-around;
            padding: 10px 0; z-index: 100;
        ",
            Link { to: Route::Home {},
                div { style: "text-align: center; cursor: pointer;",
                    div { style: "font-size: 20px;", "🏠" }
                    div { style: "font-size: 6px; color: #8b8b9e; margin-top: 2px;", "Home" }
                }
            }
            Link { to: Route::Menu {},
                div { style: "text-align: center; cursor: pointer;",
                    div { style: "font-size: 20px;", "🌿" }
                    div { style: "font-size: 6px; color: #8b8b9e; margin-top: 2px;", "Menu" }
                }
            }
            Link { to: Route::Cart {},
                div { style: "text-align: center; cursor: pointer;",
                    div { style: "font-size: 20px;", "🛒" }
                    div { style: "font-size: 6px; color: #39ff14; margin-top: 2px;", "Cart" }
                }
            }
            Link { to: Route::Orders {},
                div { style: "text-align: center; cursor: pointer;",
                    div { style: "font-size: 20px;", "📋" }
                    div { style: "font-size: 6px; color: #8b8b9e; margin-top: 2px;", "Orders" }
                }
            }
            Link { to: Route::Profile {},
                div { style: "text-align: center; cursor: pointer;",
                    div { style: "font-size: 20px;", "👤" }
                    div { style: "font-size: 6px; color: #8b8b9e; margin-top: 2px;", "Profile" }
                }
            }
        }
    }
}
