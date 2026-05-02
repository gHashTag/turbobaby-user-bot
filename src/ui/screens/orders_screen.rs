use dioxus::prelude::*;
use crate::ui::routes::Route;

#[component]
pub fn OrdersScreen() -> Element {
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
                h1 { style: "font-size: 12px; color: #39ff14; text-shadow: 0 0 8px rgba(57,255,20,0.5);", "📋 Orders" }
                p { style: "font-size: 7px; color: #8b8b9e; margin-top: 4px;", "Your order history" }
            }

            div { style: "padding: 0 16px;",

                // Order 1 — Delivered
                div { style: "
                    background: #16213e; border: 2px solid #2a2a4a;
                    border-radius: 10px; padding: 14px; margin-bottom: 10px;
                    box-shadow: 4px 4px 0 #000;
                ",
                    div { style: "display: flex; justify-content: space-between; align-items: center; margin-bottom: 8px;",
                        span { style: "font-size: 8px; color: #e8e8e8;", "Order #ORD-8A2F" }
                        span { style: "
                            font-size: 6px; padding: 3px 8px;
                            border-radius: 4px;
                            background: rgba(57,255,20,0.15); color: #39ff14;
                        ", "Delivered" }
                    }
                    div { style: "margin-bottom: 8px;",
                        div { style: "display: flex; justify-content: space-between; font-size: 7px; margin-bottom: 3px;",
                            span { style: "color: #8b8b9e;", "Northern Lights x2" }
                            span { "฿2,400" }
                        }
                        div { style: "display: flex; justify-content: space-between; font-size: 7px;",
                            span { style: "color: #8b8b9e;", "Sour Diesel x1" }
                            span { "฿1,400" }
                        }
                    }
                    div { style: "display: flex; justify-content: space-between; padding-top: 8px; border-top: 1px solid #2a2a4a; font-size: 7px;",
                        span { style: "color: #8b8b9e;", "2024-12-15" }
                        span { style: "color: #39ff14; font-weight: bold;", "฿3,800" }
                    }
                }

                // Order 2 — In Progress
                div { style: "
                    background: #16213e; border: 2px solid #00e5ff;
                    border-radius: 10px; padding: 14px; margin-bottom: 10px;
                    box-shadow: 4px 4px 0 #000, 0 0 12px rgba(0,229,255,0.1);
                ",
                    div { style: "display: flex; justify-content: space-between; align-items: center; margin-bottom: 8px;",
                        span { style: "font-size: 8px; color: #e8e8e8;", "Order #ORD-7B1E" }
                        span { style: "
                            font-size: 6px; padding: 3px 8px;
                            border-radius: 4px;
                            background: rgba(0,229,255,0.15); color: #00e5ff;
                        ", "In Progress" }
                    }
                    div { style: "margin-bottom: 8px;",
                        div { style: "display: flex; justify-content: space-between; font-size: 7px; margin-bottom: 3px;",
                            span { style: "color: #8b8b9e;", "Girl Scout Cookies x1" }
                            span { "฿1,500" }
                        }
                        div { style: "display: flex; justify-content: space-between; font-size: 7px;",
                            span { style: "color: #8b8b9e;", "SharpStone Grinder x1" }
                            span { "฿800" }
                        }
                    }
                    div { style: "display: flex; justify-content: space-between; padding-top: 8px; border-top: 1px solid #2a2a4a; font-size: 7px;",
                        span { style: "color: #8b8b9e;", "2024-12-18" }
                        span { style: "color: #00e5ff; font-weight: bold;", "฿2,300" }
                    }
                }

                // Order 3 — Cancelled
                div { style: "
                    background: #16213e; border: 2px solid #2a2a4a;
                    border-radius: 10px; padding: 14px; margin-bottom: 10px;
                    box-shadow: 4px 4px 0 #000; opacity: 0.7;
                ",
                    div { style: "display: flex; justify-content: space-between; align-items: center; margin-bottom: 8px;",
                        span { style: "font-size: 8px; color: #e8e8e8;", "Order #ORD-5C9D" }
                        span { style: "
                            font-size: 6px; padding: 3px 8px;
                            border-radius: 4px;
                            background: rgba(255,71,87,0.15); color: #ff4757;
                        ", "Cancelled" }
                    }
                    div { style: "margin-bottom: 8px;",
                        div { style: "display: flex; justify-content: space-between; font-size: 7px;",
                            span { style: "color: #8b8b9e;", "Blue Dream x3" }
                            span { "฿4,500" }
                        }
                    }
                    div { style: "display: flex; justify-content: space-between; padding-top: 8px; border-top: 1px solid #2a2a4a; font-size: 7px;",
                        span { style: "color: #8b8b9e;", "2024-12-10" }
                        span { style: "color: #ff4757; text-decoration: line-through;", "฿4,500" }
                    }
                }
            }

            // Bottom Navigation
            {bottom_nav()}
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
                    div { style: "font-size: 6px; color: #8b8b9e; margin-top: 2px;", "Cart" }
                }
            }
            Link { to: Route::Orders {},
                div { style: "text-align: center; cursor: pointer;",
                    div { style: "font-size: 20px;", "📋" }
                    div { style: "font-size: 6px; color: #39ff14; margin-top: 2px;", "Orders" }
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
