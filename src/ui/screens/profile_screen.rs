use dioxus::prelude::*;
use crate::ui::routes::Route;

#[component]
pub fn ProfileScreen() -> Element {
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
                h1 { style: "font-size: 12px; color: #39ff14; text-shadow: 0 0 8px rgba(57,255,20,0.5);", "👤 Profile" }
            }

            // Silver Member Card
            div { style: "
                max-width: 380px;
                margin: 0 auto 20px;
                border-radius: 16px;
                padding: 24px;
                position: relative;
                overflow: hidden;
                background: linear-gradient(135deg, #151520, #1f1f2a);
                border: 2px solid #c0c0c0;
                box-shadow: 0 0 20px rgba(192,192,192,0.3);
            ",
                // Scanline overlay
                div { style: "
                    position: absolute; top: 0; left: 0; right: 0; bottom: 0;
                    opacity: 0.05;
                    background: repeating-linear-gradient(0deg, transparent, transparent 4px, rgba(255,255,255,0.1) 4px, rgba(255,255,255,0.1) 8px);
                " }
                // Card header
                div { style: "display: flex; justify-content: space-between; align-items: center; margin-bottom: 16px; position: relative;",
                    div { style: "
                        width: 48px; height: 48px; border-radius: 50%;
                        display: flex; align-items: center; justify-content: center;
                        font-size: 20px;
                        background: rgba(192,192,192,0.2); border: 2px solid #c0c0c0;
                    ", "🍃" }
                    span { style: "
                        font-size: 8px; padding: 4px 12px;
                        border-radius: 4px; text-transform: uppercase;
                        background: #c0c0c0; color: #0f0f1a;
                    ", "Silver" }
                }
                // Name & ID
                div { style: "font-size: 12px; color: #c0c0c0; margin-bottom: 4px; position: relative;", "Silver Grower" }
                div { style: "font-size: 7px; color: #666; margin-bottom: 16px; position: relative;", "ID: SV-00187 • Member since 2023" }
                // Stats
                div { style: "display: grid; grid-template-columns: repeat(3, 1fr); gap: 8px; margin-bottom: 16px; position: relative;",
                    div { style: "text-align: center; padding: 8px 4px; background: rgba(0,0,0,0.3); border-radius: 8px;",
                        div { style: "font-size: 12px; color: #c0c0c0; margin-bottom: 4px;", "$750" }
                        div { style: "font-size: 6px; color: #888;", "SPENT" }
                    }
                    div { style: "text-align: center; padding: 8px 4px; background: rgba(0,0,0,0.3); border-radius: 8px;",
                        div { style: "font-size: 12px; color: #c0c0c0; margin-bottom: 4px;", "24" }
                        div { style: "font-size: 6px; color: #888;", "ORDERS" }
                    }
                    div { style: "text-align: center; padding: 8px 4px; background: rgba(0,0,0,0.3); border-radius: 8px;",
                        div { style: "font-size: 12px; color: #c0c0c0; margin-bottom: 4px;", "7" }
                        div { style: "font-size: 6px; color: #888;", "PLANTS" }
                    }
                }
                // Progress bar
                div { style: "
                    height: 8px; background: rgba(0,0,0,0.4);
                    border-radius: 4px; overflow: hidden; position: relative;
                ",
                    div { style: "height: 100%; width: 62%; border-radius: 4px; background: linear-gradient(90deg, #c0c0c0, #d8d8d8);" } }
                div { style: "display: flex; justify-content: space-between; font-size: 7px; margin-top: 6px; position: relative;",
                    span { style: "color: #888;", "$750 / $1200" }
                    span { style: "color: #c0c0c0;", "→ Gold" }
                }
            }

            // Quick actions
            div { style: "padding: 0 16px;",
                div { style: "font-size: 8px; color: #00e5ff; margin-bottom: 10px; text-transform: uppercase;", "Quick Actions" }

                Link { to: Route::Orders {},
                    div { style: "
                        background: #16213e; border: 2px solid #2a2a4a;
                        border-radius: 8px; padding: 12px 14px; margin-bottom: 8px;
                        display: flex; justify-content: space-between; align-items: center;
                        box-shadow: 3px 3px 0 #000; cursor: pointer;
                    ",
                        div { style: "display: flex; align-items: center; gap: 10px;",
                            span { style: "font-size: 18px;", "📋" }
                            span { style: "font-size: 8px;", "My Orders" }
                        }
                        span { style: "font-size: 12px; color: #8b8b9e;", "→" }
                    }
                }
                Link { to: Route::Garden {},
                    div { style: "
                        background: #16213e; border: 2px solid #2a2a4a;
                        border-radius: 8px; padding: 12px 14px; margin-bottom: 8px;
                        display: flex; justify-content: space-between; align-items: center;
                        box-shadow: 3px 3px 0 #000; cursor: pointer;
                    ",
                        div { style: "display: flex; align-items: center; gap: 10px;",
                            span { style: "font-size: 18px;", "🌱" }
                            span { style: "font-size: 8px;", "My Garden" }
                        }
                        span { style: "font-size: 12px; color: #8b8b9e;", "→" }
                    }
                }
                Link { to: Route::Quest { id: "daily".to_string() },
                    div { style: "
                        background: #16213e; border: 2px solid #2a2a4a;
                        border-radius: 8px; padding: 12px 14px; margin-bottom: 8px;
                        display: flex; justify-content: space-between; align-items: center;
                        box-shadow: 3px 3px 0 #000; cursor: pointer;
                    ",
                        div { style: "display: flex; align-items: center; gap: 10px;",
                            span { style: "font-size: 18px;", "🎯" }
                            span { style: "font-size: 8px;", "Quests" }
                        }
                        span { style: "font-size: 12px; color: #8b8b9e;", "→" }
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
                    div { style: "font-size: 6px; color: #39ff14; margin-top: 2px;", "Profile" }
                }
            }
        }
    }
}
