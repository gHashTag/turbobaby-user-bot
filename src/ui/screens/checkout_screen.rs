use dioxus::prelude::*;
use web_sys;
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
        ("🏠 Woody Phangan", "Koh Phangan, Thailand"),
        ("🏠 Woody Haad Rin", "Haad Rin, Koh Phangan"),
        ("🏠 Woody Srithanu", "Srithanu, Koh Phangan"),
    ];

    rsx! {
        div { style: "
            min-height: 100vh;
            background: #0f0f1a;
            color: #e8e8e8;
            padding-bottom: 80px;
        ",
            div { style: "padding: 20px 16px 16px; text-align: center;",
                h1 { style: "font-size: 24px; font-weight: 800; color: #39ff14; text-shadow: 3px 3px 0 #000, 0 0 10px rgba(57,255,20,0.5); letter-spacing: 2px;", "{checkout_title}" }
            }

            div { style: "padding: 0 16px;",
                // Order summary from cart
                div { style: "
                    background: #16213e; border: 4px solid #2a2a4a;
                    border-radius: 0; padding: 14px; margin-bottom: 12px;
                    box-shadow: 4px 4px 0 #000;
                ",
                    h2 { style: "font-size: 13px; font-weight: 700; color: #00e5ff; text-transform: uppercase; letter-spacing: 1px; text-shadow: 2px 2px 0 #000; margin-bottom: 10px;", "{your_order}" }
                    if cart_items.is_empty() {
                        p { style: "font-size: 15px; color: #8b8b9e; text-align: center; padding: 10px;", "Cart is empty" }
                    } else {
                        for item in cart_items.iter() {
                            div { style: "display: flex; justify-content: space-between; margin-bottom: 6px; font-size: 13px;",
                                span { "{item.name}" }
                                span { style: "color: #8b8b9e;", "x{item.quantity}" }
                                span { "฿{(item.price * item.quantity as f64) as i32}" }
                            }
                        }
                        div { style: "display: flex; justify-content: space-between; font-size: 15px; font-weight: 800; padding-top: 8px; border-top: 1px solid #2a2a4a; margin-top: 8px;",
                            span { "{total_label}" }
                            span { style: "font-size: 20px; font-weight: 800; color: #ffe600; text-shadow: 2px 2px 0 #000;", "฿{cart_total as i32}" }
                        }
                    }
                }

                // Customer info
                div { style: "
                    background: #16213e; border: 4px solid #2a2a4a;
                    border-radius: 0; padding: 14px; margin-bottom: 12px;
                    box-shadow: 4px 4px 0 #000;
                ",
                    h2 { style: "font-size: 13px; font-weight: 700; color: #00e5ff; text-transform: uppercase; letter-spacing: 1px; text-shadow: 2px 2px 0 #000; margin-bottom: 10px;", "{your_info}" }
                    div { style: "margin-bottom: 8px;",
                        label { style: "font-size: 13px; color: #8b8b9e; display: block; margin-bottom: 4px;", "Name *" }
                        input {
                            style: "
                                font-size: 15px; width: 100%; padding: 10px 12px;
                                background: #0f0f1a; color: #e8e8e8;
                                border: 4px solid #2a2a4a; border-radius: 0;
                                box-sizing: border-box;
                            ",
                            r#type: "text",
                            placeholder: "Enter your name",
                            value: "{customer_name}",
                            oninput: move |e| customer_name.set(e.value()),
                        }
                    }
                    div { style: "margin-bottom: 8px;",
                        label { style: "font-size: 13px; color: #8b8b9e; display: block; margin-bottom: 4px;", "Phone *" }
                        input {
                            style: "
                                font-size: 15px; width: 100%; padding: 10px 12px;
                                background: #0f0f1a; color: #e8e8e8;
                                border: 4px solid #2a2a4a; border-radius: 0;
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
                    background: #16213e; border: 4px solid #2a2a4a;
                    border-radius: 0; padding: 14px; margin-bottom: 12px;
                    box-shadow: 4px 4px 0 #000;
                ",
                    h2 { style: "font-size: 13px; font-weight: 700; color: #00e5ff; text-transform: uppercase; letter-spacing: 1px; text-shadow: 2px 2px 0 #000; margin-bottom: 10px;", "{pickup_location}" }
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
                                        background: {bg}; border: 4px solid {border};
                                        border-radius: 0; padding: 10px; margin-bottom: 6px;
                                        cursor: pointer;
                                    ",
                                    onclick: move |_| shop_selected.set(idx_val),
                                    div { style: "font-size: 15px; margin-bottom: 2px;", "{shop_name}" }
                                    div { style: "font-size: 13px; color: #8b8b9e;", "{shop_addr}" }
                                }
                            }
                        }
                    }
                    div {
                        style: "margin-top:8px;cursor:pointer;font-size:13px;color:#00e5ff;text-decoration:underline;text-align:center;",
                        onclick: move |_| {
                            let _ = web_sys::window().and_then(|w| w.open_with_url_and_target("https://www.google.com/maps/place/Woody+Weed+Pecker/@9.7124562,99.9877309,17z/data=!3m1!4b1!4m6!3m5!1s0x3054ffe9f6df4edf:0xf8735a84f5193e1a!8m2!3d9.7124562!4d99.9877309!16s%2Fg%2F11x314fym6!18m1!1e1?entry=ttu&g_ep=EgoyMDI2MDUxMy4wIKXMDSoASAFQAw%3D%3D", "_blank").ok());
                        },
                        "📍 Открыть на карте"
                    }
                }

                // Delivery info
                div { style: "
                    background: #16213e; border: 4px solid #2a2a4a;
                    border-radius: 0; padding: 14px; margin-bottom: 12px;
                    box-shadow: 4px 4px 0 #000;
                ",
                    h2 { style: "font-size: 13px; font-weight: 700; color: #00e5ff; text-transform: uppercase; letter-spacing: 1px; text-shadow: 2px 2px 0 #000; margin-bottom: 10px;", "{delivery}" }
                    div { style: "
                        background: rgba(0,229,255,0.05);
                        border: 4px solid rgba(0,229,255,0.2);
                        border-radius: 0; padding: 10px;
                    ",
                        div { style: "font-size: 13px; margin-bottom: 6px;", "⏰ 30-45 min pickup" }
                        div { style: "font-size: 15px; color: #39ff14;", "💰 Free delivery over ฿1,000" }
                    }
                }

                // Payment method
                div { style: "
                    background: #16213e; border: 4px solid #2a2a4a;
                    border-radius: 0; padding: 14px; margin-bottom: 16px;
                    box-shadow: 4px 4px 0 #000;
                ",
                    h2 { style: "font-size: 13px; font-weight: 700; color: #00e5ff; text-transform: uppercase; letter-spacing: 1px; text-shadow: 2px 2px 0 #000; margin-bottom: 10px;", "{payment}" }
                    div { style: "
                        display: flex; align-items: center; gap: 8px;
                        background: rgba(57,255,20,0.05);
                        border: 4px solid #39ff14; border-radius: 0; padding: 10px;
                    ",
                        div { style: "font-size: 13px;", "💳" }
                        div { style: "flex: 1;",
                            div { style: "font-size: 15px; color: #39ff14;", "Cash on Delivery" }
                            div { style: "font-size: 13px; color: #8b8b9e; margin-top: 2px;", "Pay when you receive" }
                        }
                    }
                }

                // Legal disclaimer
                div { style: "
                    background: rgba(255,71,87,0.05);
                    border: 4px solid rgba(255,71,87,0.2);
                    border-radius: 0; padding: 10px; margin-bottom: 16px;
                ",
                    div { style: "font-size: 15px; color: #ff4757; margin-bottom: 4px;", "⚠️ Disclaimer" }
                    div { style: "font-size: 13px; color: #8b8b9e;", "By placing this order you confirm you are 20+ years old and aware of local regulations." }
                }

                // Actions
                div { style: "display: flex; gap: 10px;",
                    Link { to: Route::Cart {},
                        button { style: "
                            font-size: 14px; font-weight: 700; flex: 1; padding: 12px 20px;
                            background: transparent; color: #e8e8e8;
                            border: 4px solid #2a2a4a; border-radius: 0;
                            cursor: pointer; box-shadow: 3px 3px 0 #000;
                            transition: transform 0.1s, box-shadow 0.1s;
                        ", "{back}" }
                    }
                    Link { to: Route::Success { id: "ORD-DEMO-001".to_string() },
                        {
                            let trios_items = to_trios_items(&cart_items);
                            let can_order = validate_checkout(&customer_name(), &customer_phone(), &trios_items).is_ok();
                            let btn_bg = if can_order { "#39ff14" } else { "#2a2a4a" };
                            let btn_color = if can_order { "#000" } else { "#8b8b9e" };
                            let btn_cursor = if can_order { "pointer" } else { "not-allowed" };
                            rsx! {
                                button {
                                    style: "
                                        font-size: 14px; font-weight: 700; flex: 2; padding: 12px 20px;
                                        background: {btn_bg};
                                        color: {btn_color};
                                        border: 4px solid #2d9e0f; border-radius: 0;
                                        cursor: {btn_cursor};
                                        box-shadow: 3px 3px 0 #000;
                                        transition: transform 0.1s, box-shadow 0.1s;
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
