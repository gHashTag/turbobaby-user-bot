use dioxus::prelude::*;
use crate::ui::routes::Route;

#[component]
pub fn CheckoutScreen() -> Element {
    rsx! {
        div { style: "
            min-height: 100vh;
            background: #0f0f1a;
            color: #e8e8e8;
            font-family: 'Press Start 2P', monospace;
            padding-bottom: 40px;
        ",
            // Header
            div { style: "padding: 20px 16px 12px; text-align: center;",
                h1 { style: "font-size: 12px; color: #39ff14; text-shadow: 0 0 8px rgba(57,255,20,0.5);", "🛍️ Checkout" }
            }

            div { style: "padding: 0 16px;",
                // Order summary
                div { style: "
                    background: #16213e; border: 2px solid #2a2a4a;
                    border-radius: 8px; padding: 14px; margin-bottom: 12px;
                    box-shadow: 4px 4px 0 #000;
                ",
                    h2 { style: "font-size: 9px; color: #00e5ff; margin-bottom: 10px;", "Your Order" }
                    // Item rows
                    div { style: "display: flex; justify-content: space-between; margin-bottom: 6px; font-size: 8px;",
                        span { "Northern Lights" }
                        span { style: "color: #8b8b9e;", "x2" }
                        span { "฿2,400" }
                    }
                    div { style: "display: flex; justify-content: space-between; margin-bottom: 6px; font-size: 8px;",
                        span { "Sour Diesel" }
                        span { style: "color: #8b8b9e;", "x1" }
                        span { "฿1,400" }
                    }
                    div { style: "display: flex; justify-content: space-between; font-size: 9px; font-weight: bold; padding-top: 8px; border-top: 1px solid #2a2a4a; margin-top: 8px;",
                        span { "Total:" }
                        span { style: "color: #39ff14;", "฿3,800" }
                    }
                }

                // Delivery info
                div { style: "
                    background: #16213e; border: 2px solid #2a2a4a;
                    border-radius: 8px; padding: 14px; margin-bottom: 12px;
                    box-shadow: 4px 4px 0 #000;
                ",
                    h2 { style: "font-size: 9px; color: #00e5ff; margin-bottom: 10px;", "Delivery" }
                    div { style: "
                        background: rgba(0,229,255,0.05);
                        border: 1px solid rgba(0,229,255,0.2);
                        border-radius: 6px; padding: 10px;
                    ",
                        div { style: "font-size: 7px; margin-bottom: 6px;", "📍 Current location" }
                        div { style: "font-size: 7px; margin-bottom: 6px;", "⏰ 30-45 min delivery" }
                        div { style: "font-size: 7px; color: #39ff14;", "💰 Free delivery over ฿1,000" }
                    }
                }

                // Payment method
                div { style: "
                    background: #16213e; border: 2px solid #2a2a4a;
                    border-radius: 8px; padding: 14px; margin-bottom: 16px;
                    box-shadow: 4px 4px 0 #000;
                ",
                    h2 { style: "font-size: 9px; color: #00e5ff; margin-bottom: 10px;", "Payment" }
                    div { style: "
                        display: flex; align-items: center; gap: 8px;
                        background: rgba(57,255,20,0.05);
                        border: 2px solid #39ff14; border-radius: 6px; padding: 10px;
                    ",
                        div { style: "font-size: 20px;", "💳" }
                        div { style: "flex: 1;",
                            div { style: "font-size: 8px; color: #39ff14;", "Cash on Delivery" }
                            div { style: "font-size: 6px; color: #8b8b9e; margin-top: 2px;", "Pay when you receive" }
                        }
                    }
                }

                // Actions
                div { style: "display: flex; gap: 10px;",
                    Link { to: Route::Cart {},
                        button { style: "
                            font-family: 'Press Start 2P', monospace;
                            font-size: 7px; flex: 1; padding: 12px;
                            background: transparent; color: #e8e8e8;
                            border: 2px solid #2a2a4a; border-radius: 6px;
                            cursor: pointer;
                        ", "← Back" }
                    }
                    Link { to: Route::Success { id: "ORD-DEMO-001".to_string() },
                        button { style: "
                            font-family: 'Press Start 2P', monospace;
                            font-size: 7px; flex: 2; padding: 12px;
                            background: #39ff14; color: #0f0f1a;
                            border: none; border-radius: 6px;
                            cursor: pointer; font-weight: bold;
                        ", "Place Order ✓" }
                    }
                }
            }
        }
    }
}
