use dioxus::prelude::*;
use crate::ui::routes::Route;

#[component]
pub fn AccessoriesScreen() -> Element {
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
                h1 { style: "font-size: 12px; color: #00e5ff; text-shadow: 0 0 8px rgba(0,229,255,0.5);", "💨 Accessories" }
                p { style: "font-size: 7px; color: #8b8b9e; margin-top: 4px;", "Essential gear for your experience" }
            }

            // Filter tabs
            div { style: "display: flex; gap: 6px; padding: 0 16px 12px; overflow-x: auto;",
                button { style: "
                    font-family: 'Press Start 2P', monospace;
                    font-size: 6px; padding: 6px 10px;
                    background: #00e5ff; color: #0f0f1a;
                    border: 2px solid #00e5ff; border-radius: 4px;
                    cursor: pointer; white-space: nowrap;
                ", "All" }
                button { style: "
                    font-family: 'Press Start 2P', monospace;
                    font-size: 6px; padding: 6px 10px;
                    background: transparent; color: #8b8b9e;
                    border: 2px solid #2a2a4a; border-radius: 4px;
                    cursor: pointer; white-space: nowrap;
                ", "Pipes" }
                button { style: "
                    font-family: 'Press Start 2P', monospace;
                    font-size: 6px; padding: 6px 10px;
                    background: transparent; color: #8b8b9e;
                    border: 2px solid #2a2a4a; border-radius: 4px;
                    cursor: pointer; white-space: nowrap;
                ", "Grinders" }
                button { style: "
                    font-family: 'Press Start 2P', monospace;
                    font-size: 6px; padding: 6px 10px;
                    background: transparent; color: #8b8b9e;
                    border: 2px solid #2a2a4a; border-radius: 4px;
                    cursor: pointer; white-space: nowrap;
                ", "Rolling" }
            }

            // Products grid (2 columns)
            div { style: "display: grid; grid-template-columns: 1fr 1fr; gap: 10px; padding: 0 16px;",
                // Grinder
                div { style: "
                    background: #16213e; border: 2px solid #2a2a4a;
                    border-radius: 8px; overflow: hidden; box-shadow: 4px 4px 0 #000;
                ",
                    div { style: "height: 80px; background: linear-gradient(135deg, #1a1a2e, #16213e); display: flex; align-items: center; justify-content: center; font-size: 36px;", "⚙️" }
                    div { style: "padding: 8px;",
                        div { style: "font-size: 6px; color: #00e5ff; margin-bottom: 2px;", "GRINDER" }
                        div { style: "font-size: 9px; font-weight: bold; margin-bottom: 4px;", "SharpStone 4pc" }
                        div { style: "font-size: 9px; color: #39ff14; margin-bottom: 6px;", "฿800" }
                    }
                    div { style: "padding: 0 8px 8px;",
                        button { style: "
                            font-family: 'Press Start 2P', monospace;
                            font-size: 6px; width: 100%; padding: 6px;
                            background: #39ff14; color: #0f0f1a;
                            border: none; border-radius: 4px; cursor: pointer;
                        ", "Add to Cart 🛒" }
                    }
                }

                // Rolling Papers
                div { style: "
                    background: #16213e; border: 2px solid #2a2a4a;
                    border-radius: 8px; overflow: hidden; box-shadow: 4px 4px 0 #000;
                ",
                    div { style: "height: 80px; background: linear-gradient(135deg, #1a1a2e, #16213e); display: flex; align-items: center; justify-content: center; font-size: 36px;", "📜" }
                    div { style: "padding: 8px;",
                        div { style: "font-size: 6px; color: #00e5ff; margin-bottom: 2px;", "ROLLING" }
                        div { style: "font-size: 9px; font-weight: bold; margin-bottom: 4px;", "Raw Papers King" }
                        div { style: "font-size: 9px; color: #39ff14; margin-bottom: 6px;", "฿120" }
                    }
                    div { style: "padding: 0 8px 8px;",
                        button { style: "
                            font-family: 'Press Start 2P', monospace;
                            font-size: 6px; width: 100%; padding: 6px;
                            background: #39ff14; color: #0f0f1a;
                            border: none; border-radius: 4px; cursor: pointer;
                        ", "Add to Cart 🛒" }
                    }
                }

                // Pipe
                div { style: "
                    background: #16213e; border: 2px solid #2a2a4a;
                    border-radius: 8px; overflow: hidden; box-shadow: 4px 4px 0 #000;
                ",
                    div { style: "height: 80px; background: linear-gradient(135deg, #1a1a2e, #16213e); display: flex; align-items: center; justify-content: center; font-size: 36px;", "🪈" }
                    div { style: "padding: 8px;",
                        div { style: "font-size: 6px; color: #00e5ff; margin-bottom: 2px;", "PIPE" }
                        div { style: "font-size: 9px; font-weight: bold; margin-bottom: 4px;", "Glass Spoon Pipe" }
                        div { style: "font-size: 9px; color: #39ff14; margin-bottom: 6px;", "฿600" }
                    }
                    div { style: "padding: 0 8px 8px;",
                        button { style: "
                            font-family: 'Press Start 2P', monospace;
                            font-size: 6px; width: 100%; padding: 6px;
                            background: #39ff14; color: #0f0f1a;
                            border: none; border-radius: 4px; cursor: pointer;
                        ", "Add to Cart 🛒" }
                    }
                }

                // Vaporizer
                div { style: "
                    background: #16213e; border: 2px solid #2a2a4a;
                    border-radius: 8px; overflow: hidden; box-shadow: 4px 4px 0 #000;
                    opacity: 0.6;
                ",
                    div { style: "height: 80px; background: linear-gradient(135deg, #1a1a2e, #16213e); display: flex; align-items: center; justify-content: center; font-size: 36px; opacity: 0.5;", "💨" }
                    div { style: "padding: 8px;",
                        div { style: "font-size: 6px; color: #00e5ff; margin-bottom: 2px;", "VAPORIZER" }
                        div { style: "font-size: 9px; font-weight: bold; margin-bottom: 4px;", "Mighty+" }
                        div { style: "font-size: 9px; color: #8b8b9e; text-decoration: line-through; margin-bottom: 6px;", "฿12,000" }
                    }
                    div { style: "padding: 0 8px 8px;",
                        button { style: "
                            font-family: 'Press Start 2P', monospace;
                            font-size: 6px; width: 100%; padding: 6px;
                            background: transparent; color: #8b8b9e;
                            border: 2px solid #2a2a4a; border-radius: 4px; cursor: not-allowed;
                        ", "Sold Out" }
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
