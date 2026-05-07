use dioxus::prelude::*;
use crate::ui::routes::Route;
use crate::ui::state::Cart;
use crate::trios::store::validate_checkout;
use crate::trios::core::Lang;
use crate::trios::i18n::{t, T_CHECKOUT_TITLE, T_YOUR_ORDER, T_YOUR_INFO, T_PICKUP_LOCATION, T_DELIVERY, T_PAYMENT, T_PLACE_ORDER, T_BACK, T_TOTAL};

fn to_trios_items(items: &[crate::ui::state::CartItem]) -> Vec<crate::trios::store::CartItem> {
    items.iter().map(|i| crate::trios::store::CartItem::new_strain(i.id.clone(), i.quantity)).collect()
}

#[component]
pub fn CheckoutScreen() -> Element {
    let mut cart = use_context::<Signal<Cart>>();
    let cart_items = cart.read().items.clone();
    let cart_total = cart.read().total;
    let mut customer_name = use_signal(|| String::new());
    let mut customer_phone = use_signal(|| String::new());
    let mut shop_selected = use_signal(|| 0usize);

    let checkout_title = t(Lang::Russian, T_CHECKOUT_TITLE);
    let your_order = t(Lang::Russian, T_YOUR_ORDER);
    let your_info = format!("👤 {}", t(Lang::Russian, T_YOUR_INFO));
    let pickup_location = t(Lang::Russian, T_PICKUP_LOCATION);
    let delivery = t(Lang::Russian, T_DELIVERY);
    let payment = t(Lang::Russian, T_PAYMENT);
    let place_order = t(Lang::Russian, T_PLACE_ORDER);
    let back = t(Lang::Russian, T_BACK);
    let total_label = t(Lang::Russian, T_TOTAL);

    let shops = [
        ("🏠 Woody Sukhumvit", "Sukhumvit Soi 23, Bangkok"),
        ("🏠 Woody Thonglor", "Thonglor Soi 5, Bangkok"),
        ("🏠 Woody Silom", "Silom Road Soi 12, Bangkok"),
    ];

    rsx! {
        div { style: "
            min-height: 100vh;
            background: #0f0f1a;
            color: #e8e8e8;
            font-family: 'Press Start 2P', monospace;
            padding-bottom: 40px;
        ",
            div { style: "padding: 20px 16px 12px; text-align: center;",
                h1 { style: "font-size: 18px; color: #39ff14; text-shadow: 0 0 8px rgba(57,255,20,0.5);", "{checkout_title}" }
            }

            div { style: "padding: 0 16px;",
                // Order summary from cart
                div { style: "
                    background: #1a1a2e; border: 2px solid #2a2a4a;
                    border-radius: 8px; padding: 14px; margin-bottom: 12px;
                    box-shadow: 4px 4px 0 #000;
                ",
                    h2 { style: "font-size: 14px; color: #00e5ff; margin-bottom: 10px;", "{your_order}" }
                    if cart_items.is_empty() {
                        p { style: "font-size: 18px; color: #8b8b9e; text-align: center; padding: 10px;", "Cart is empty" }
                    } else {
                        for item in cart_items.iter() {
                            div { style: "display: flex; justify-content: space-between; margin-bottom: 6px; font-size: 12px;",
                                span { "{item.name}" }
                                span { style: "color: #8b8b9e;", "x{item.quantity}" }
                                span { "฿{(item.price * item.quantity as f64) as i32}" }
                            }
                        }
                        div { style: "display: flex; justify-content: space-between; font-size: 16px; font-weight: bold; padding-top: 8px; border-top: 1px solid #2a2a4a; margin-top: 8px;",
                            span { "{total_label}" }
                            span { style: "color: #39ff14;", "฿{cart_total as i32}" }
                        }
                    }
                }

                // Customer info
                div { style: "
                    background: #1a1a2e; border: 2px solid #2a2a4a;
                    border-radius: 8px; padding: 14px; margin-bottom: 12px;
                    box-shadow: 4px 4px 0 #000;
                ",
                    h2 { style: "font-size: 14px; color: #00e5ff; margin-bottom: 10px;", "{your_info}" }
                    div { style: "margin-bottom: 8px;",
                        label { style: "font-size: 18px; color: #8b8b9e; display: block; margin-bottom: 4px;", "Name *" }
                        input {
                            style: "
                                font-family: 'Press Start 2P', monospace;
                                font-size: 12px; width: 100%; padding: 8px 10px;
                                background: #0f0f1a; color: #e8e8e8;
                                border: 2px solid #2a2a4a; border-radius: 8px;
                                box-sizing: border-box;
                            ",
                            r#type: "text",
                            placeholder: "Enter your name",
                            value: "{customer_name}",
                            oninput: move |e| customer_name.set(e.value()),
                        }
                    }
                    div { style: "margin-bottom: 8px;",
                        label { style: "font-size: 18px; color: #8b8b9e; display: block; margin-bottom: 4px;", "Phone *" }
                        input {
                            style: "
                                font-family: 'Press Start 2P', monospace;
                                font-size: 12px; width: 100%; padding: 8px 10px;
                                background: #0f0f1a; color: #e8e8e8;
                                border: 2px solid #2a2a4a; border-radius: 8px;
                                box-sizing: border-box;
                            ",
                            r#type: "tel",
                            placeholder: "+66 xxx xxx xxxx",
                            value: "{customer_phone}",
                            oninput: move |e| customer_phone.set(e.value()),
                        }
                    }
                }

                // Shop selection
                div { style: "
                    background: #1a1a2e; border: 2px solid #2a2a4a;
                    border-radius: 8px; padding: 14px; margin-bottom: 12px;
                    box-shadow: 4px 4px 0 #000;
                ",
                    h2 { style: "font-size: 14px; color: #00e5ff; margin-bottom: 10px;", "{pickup_location}" }
                    for (idx, (name, address)) in shops.iter().enumerate() {
                        {
                            let is_selected = shop_selected() == idx;
                            let border = if is_selected { "#39ff14" } else { "#2a2a4a" };
                            let bg = if is_selected { "rgba(57,255,20,0.08)" } else { "transparent" };
                            let shop_name = name.to_string();
                            let shop_addr = address.to_string();
                            let idx_val = idx;
                            rsx! {
                                div {
                                    style: "
                                        background: {bg}; border: 2px solid {border};
                                        border-radius: 6px; padding: 10px; margin-bottom: 6px;
                                        cursor: pointer;
                                    ",
                                    onclick: move |_| shop_selected.set(idx_val),
                                    div { style: "font-size: 12px; margin-bottom: 2px;", "{shop_name}" }
                                    div { style: "font-size: 18px; color: #8b8b9e;", "{shop_addr}" }
                                }
                            }
                        }
                    }
                }

                // Delivery info
                div { style: "
                    background: #1a1a2e; border: 2px solid #2a2a4a;
                    border-radius: 8px; padding: 14px; margin-bottom: 12px;
                    box-shadow: 4px 4px 0 #000;
                ",
                    h2 { style: "font-size: 14px; color: #00e5ff; margin-bottom: 10px;", "{delivery}" }
                    div { style: "
                        background: rgba(0,229,255,0.05);
                        border: 2px solid rgba(0,229,255,0.2);
                        border-radius: 6px; padding: 10px;
                    ",
                        div { style: "font-size: 10px; margin-bottom: 6px;", "⏰ 30-45 min pickup" }
                        div { style: "font-size: 18px; color: #39ff14;", "💰 Free delivery over ฿1,000" }
                    }
                }

                // Payment method
                div { style: "
                    background: #1a1a2e; border: 2px solid #2a2a4a;
                    border-radius: 8px; padding: 14px; margin-bottom: 16px;
                    box-shadow: 4px 4px 0 #000;
                ",
                    h2 { style: "font-size: 14px; color: #00e5ff; margin-bottom: 10px;", "{payment}" }
                    div { style: "
                        display: flex; align-items: center; gap: 8px;
                        background: rgba(57,255,20,0.05);
                        border: 2px solid #39ff14; border-radius: 6px; padding: 10px;
                    ",
                        div { style: "font-size: 10px;", "💳" }
                        div { style: "flex: 1;",
                            div { style: "font-size: 12px; color: #39ff14;", "Cash on Delivery" }
                            div { style: "font-size: 18px; color: #8b8b9e; margin-top: 2px;", "Pay when you receive" }
                        }
                    }
                }

                // Legal disclaimer
                div { style: "
                    background: rgba(255,71,87,0.05);
                    border: 2px solid rgba(255,71,87,0.2);
                    border-radius: 6px; padding: 10px; margin-bottom: 16px;
                ",
                    div { style: "font-size: 18px; color: #ff4757; margin-bottom: 4px;", "⚠️ Disclaimer" }
                    div { style: "font-size: 9px; color: #8b8b9e;", "By placing this order you confirm you are 20+ years old and aware of local regulations." }
                }

                // Actions
                div { style: "display: flex; gap: 10px;",
                    Link { to: Route::Cart {},
                        button { style: "
                            font-family: 'Press Start 2P', monospace;
                            font-size: 10px; flex: 1; padding: 12px;
                            background: transparent; color: #e8e8e8;
                            border: 2px solid #2a2a4a; border-radius: 6px;
                            cursor: pointer;
                        ", "{back}" }
                    }
                    Link { to: Route::Success { id: "ORD-DEMO-001".to_string() },
                        {
                            let trios_items = to_trios_items(&cart_items);
                            let can_order = validate_checkout(&customer_name(), &customer_phone(), &trios_items).is_ok();
                            let btn_bg = if can_order { "#39ff14" } else { "#2a2a4a" };
                            let btn_color = if can_order { "#0f0f1a" } else { "#8b8b9e" };
                            let btn_cursor = if can_order { "pointer" } else { "not-allowed" };
                            rsx! {
                                button {
                                    style: "
                                        font-family: 'Press Start 2P', monospace;
                                        font-size: 10px; flex: 2; padding: 12px;
                                        background: {btn_bg};
                                        color: {btn_color};
                                        border: none; border-radius: 6px;
                                        cursor: {btn_cursor};
                                        font-weight: bold;
                                    ",
                                    onclick: move |_| {
                                        let trios_items = to_trios_items(&cart_items);
                                        if let Err(_) = validate_checkout(&customer_name(), &customer_phone(), &trios_items) {
                                            return;
                                        }
                                        let mut c = cart.write();
                                        c.clear();
                                    },
                                    "{place_order}"
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
