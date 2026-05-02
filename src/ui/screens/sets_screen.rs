use dioxus::prelude::*;
use crate::ui::routes::Route;

#[component]
pub fn SetsScreen() -> Element {
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
                h1 { style: "font-size: 12px; color: #b388ff; text-shadow: 0 0 8px rgba(179,136,255,0.5);", "📦 Sets" }
                p { style: "font-size: 7px; color: #8b8b9e; margin-top: 4px;", "Curated collections & bundles" }
            }

            // Sets grid
            div { style: "display: flex; flex-direction: column; gap: 12px; padding: 0 16px;",

                // Starter Set
                div { style: "
                    background: #16213e;
                    border: 2px solid #2a2a4a;
                    border-radius: 12px;
                    overflow: hidden;
                    box-shadow: 4px 4px 0 #000;
                ",
                    div { style: "
                        height: 100px;
                        background: linear-gradient(135deg, #1a1a2e, #16213e);
                        display: flex; align-items: center; justify-content: center;
                        font-size: 48px;
                        position: relative;
                    ",
                        "🎁"
                        span { style: "
                            position: absolute; top: 6px; right: 6px;
                            font-size: 5px; background: #39ff14; color: #0f0f1a;
                            padding: 2px 6px; border-radius: 3px;
                        ", "BEST VALUE" }
                    }
                    div { style: "padding: 12px;",
                        div { style: "font-size: 10px; font-weight: bold; margin-bottom: 4px;", "Starter Pack" }
                        div { style: "font-size: 7px; color: #8b8b9e; margin-bottom: 8px;", "3 strains + grinder + papers" }
                        div { style: "display: flex; justify-content: space-between; align-items: center;",
                            div { style: "
                                font-size: 9px; color: #8b8b9e; text-decoration: line-through;
                            ", "฿4,200" }
                            span { style: "font-size: 12px; color: #39ff14;", "฿3,200" }
                        }
                    }
                    div { style: "padding: 0 12px 12px;",
                        button { style: "
                            font-family: 'Press Start 2P', monospace;
                            font-size: 7px;
                            width: 100%;
                            padding: 8px;
                            background: #39ff14;
                            color: #0f0f1a;
                            border: none;
                            border-radius: 4px;
                            cursor: pointer;
                        ", "Add to Cart 🛒" }
                    }
                }

                // Connoisseur Set
                div { style: "
                    background: #16213e;
                    border: 2px solid #b388ff;
                    border-radius: 12px;
                    overflow: hidden;
                    box-shadow: 4px 4px 0 #000, 0 0 16px rgba(179,136,255,0.15);
                ",
                    div { style: "
                        height: 100px;
                        background: linear-gradient(135deg, #1a1a2e, #16213e);
                        display: flex; align-items: center; justify-content: center;
                        font-size: 48px;
                        position: relative;
                    ",
                        "👑"
                        span { style: "
                            position: absolute; top: 6px; right: 6px;
                            font-size: 5px; background: #b388ff; color: #0f0f1a;
                            padding: 2px 6px; border-radius: 3px;
                        ", "PREMIUM" }
                    }
                    div { style: "padding: 12px;",
                        div { style: "font-size: 10px; font-weight: bold; margin-bottom: 4px; color: #b388ff;", "Connoisseur Collection" }
                        div { style: "font-size: 7px; color: #8b8b9e; margin-bottom: 8px;", "5 premium strains + vaporizer + case" }
                        div { style: "display: flex; justify-content: space-between; align-items: center;",
                            div { style: "font-size: 9px; color: #8b8b9e; text-decoration: line-through;", "฿12,000" }
                            span { style: "font-size: 12px; color: #b388ff;", "฿8,500" }
                        }
                    }
                    div { style: "padding: 0 12px 12px;",
                        button { style: "
                            font-family: 'Press Start 2P', monospace;
                            font-size: 7px;
                            width: 100%;
                            padding: 8px;
                            background: #b388ff;
                            color: #0f0f1a;
                            border: none;
                            border-radius: 4px;
                            cursor: pointer;
                        ", "Add to Cart 🛒" }
                    }
                }

                // Party Set
                div { style: "
                    background: #16213e;
                    border: 2px solid #2a2a4a;
                    border-radius: 12px;
                    overflow: hidden;
                    box-shadow: 4px 4px 0 #000;
                ",
                    div { style: "
                        height: 100px;
                        background: linear-gradient(135deg, #1a1a2e, #16213e);
                        display: flex; align-items: center; justify-content: center;
                        font-size: 48px;
                    ", "🎉" }
                    div { style: "padding: 12px;",
                        div { style: "font-size: 10px; font-weight: bold; margin-bottom: 4px;", "Party Pack" }
                        div { style: "font-size: 7px; color: #8b8b9e; margin-bottom: 8px;", "10 pre-rolls + rolling kit + lighter" }
                        div { style: "font-size: 12px; color: #39ff14; margin-bottom: 8px;", "฿2,800" }
                    }
                    div { style: "padding: 0 12px 12px;",
                        button { style: "
                            font-family: 'Press Start 2P', monospace;
                            font-size: 7px;
                            width: 100%;
                            padding: 8px;
                            background: #39ff14;
                            color: #0f0f1a;
                            border: none;
                            border-radius: 4px;
                            cursor: pointer;
                        ", "Add to Cart 🛒" }
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
            position: fixed;
            bottom: 0;
            left: 0;
            right: 0;
            background: #1a1a2e;
            border-top: 2px solid #2a2a4a;
            display: flex;
            justify-content: space-around;
            padding: 10px 0;
            z-index: 100;
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
