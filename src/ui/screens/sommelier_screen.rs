use dioxus::prelude::*;
use crate::ui::routes::Route;

#[component]
pub fn SommelierScreen() -> Element {
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
                h1 { style: "font-size: 12px; color: #00e5ff; text-shadow: 0 0 8px rgba(0,229,255,0.5);", "🍷 Sommelier" }
                p { style: "font-size: 7px; color: #8b8b9e; margin-top: 4px;", "AI-powered strain recommendations" }
            }

            // Question card
            div { style: "
                margin: 0 16px 16px;
                background: linear-gradient(135deg, rgba(0,229,255,0.08), rgba(57,255,20,0.08));
                border: 2px solid #00e5ff;
                border-radius: 12px;
                padding: 16px;
                box-shadow: 0 0 16px rgba(0,229,255,0.1), 3px 3px 0 #000;
            ",
                div { style: "font-size: 8px; color: #00e5ff; margin-bottom: 10px;", "🤔 What's your mood?" }
                div { style: "display: grid; grid-template-columns: 1fr 1fr; gap: 8px;",
                    button { style: "
                        font-family: 'Press Start 2P', monospace;
                        font-size: 6px;
                        padding: 10px 8px;
                        background: #16213e;
                        color: #e8e8e8;
                        border: 2px solid #2a2a4a;
                        border-radius: 6px;
                        cursor: pointer;
                        text-align: center;
                    ", "😌 Relax" }
                    button { style: "
                        font-family: 'Press Start 2P', monospace;
                        font-size: 6px;
                        padding: 10px 8px;
                        background: #16213e;
                        color: #e8e8e8;
                        border: 2px solid #2a2a4a;
                        border-radius: 6px;
                        cursor: pointer;
                        text-align: center;
                    ", "⚡ Energy" }
                    button { style: "
                        font-family: 'Press Start 2P', monospace;
                        font-size: 6px;
                        padding: 10px 8px;
                        background: #16213e;
                        color: #e8e8e8;
                        border: 2px solid #2a2a4a;
                        border-radius: 6px;
                        cursor: pointer;
                        text-align: center;
                    ", "🎨 Creative" }
                    button { style: "
                        font-family: 'Press Start 2P', monospace;
                        font-size: 6px;
                        padding: 10px 8px;
                        background: #16213e;
                        color: #e8e8e8;
                        border: 2px solid #2a2a4a;
                        border-radius: 6px;
                        cursor: pointer;
                        text-align: center;
                    ", "😴 Sleep" }
                }
            }

            // Recommendations
            div { style: "font-size: 8px; color: #39ff14; padding: 0 16px 8px; text-transform: uppercase;", "✨ Recommended for you" }

            div { style: "display: flex; flex-direction: column; gap: 10px; padding: 0 16px;",
                // Rec 1
                div { style: "
                    background: #16213e;
                    border: 2px solid #2a2a4a;
                    border-radius: 10px;
                    padding: 12px;
                    display: flex;
                    gap: 12px;
                    align-items: center;
                    box-shadow: 3px 3px 0 #000;
                ",
                    div { style: "font-size: 36px; min-width: 48px; text-align: center;", "🌿" }
                    div { style: "flex: 1;",
                        div { style: "font-size: 9px; font-weight: bold; margin-bottom: 4px;", "Northern Lights" }
                        div { style: "font-size: 6px; color: #00e5ff; margin-bottom: 2px;", "🌙 Indica • 18% THC" }
                        div { style: "font-size: 6px; color: #8b8b9e; margin-bottom: 6px;", "Perfect for deep relaxation" }
                        div { style: "display: flex; justify-content: space-between; align-items: center;",
                            span { style: "font-size: 9px; color: #39ff14;", "฿1,200" }
                            span { style: "font-size: 6px; color: #39ff14; background: rgba(57,255,20,0.15); padding: 2px 6px; border-radius: 3px;", "95% match" }
                        }
                    }
                }

                // Rec 2
                div { style: "
                    background: #16213e;
                    border: 2px solid #2a2a4a;
                    border-radius: 10px;
                    padding: 12px;
                    display: flex;
                    gap: 12px;
                    align-items: center;
                    box-shadow: 3px 3px 0 #000;
                ",
                    div { style: "font-size: 36px; min-width: 48px; text-align: center;", "🌙" }
                    div { style: "flex: 1;",
                        div { style: "font-size: 9px; font-weight: bold; margin-bottom: 4px;", "Granddaddy Purple" }
                        div { style: "font-size: 6px; color: #00e5ff; margin-bottom: 2px;", "🌙 Indica • 20% THC" }
                        div { style: "font-size: 6px; color: #8b8b9e; margin-bottom: 6px;", "Classic body high, grape flavor" }
                        div { style: "display: flex; justify-content: space-between; align-items: center;",
                            span { style: "font-size: 9px; color: #39ff14;", "฿1,400" }
                            span { style: "font-size: 6px; color: #00e5ff; background: rgba(0,229,255,0.15); padding: 2px 6px; border-radius: 3px;", "88% match" }
                        }
                    }
                }

                // Rec 3
                div { style: "
                    background: #16213e;
                    border: 2px solid #2a2a4a;
                    border-radius: 10px;
                    padding: 12px;
                    display: flex;
                    gap: 12px;
                    align-items: center;
                    box-shadow: 3px 3px 0 #000;
                ",
                    div { style: "font-size: 36px; min-width: 48px; text-align: center;", "💜" }
                    div { style: "flex: 1;",
                        div { style: "font-size: 9px; font-weight: bold; margin-bottom: 4px;", "Blueberry Kush" }
                        div { style: "font-size: 6px; color: #ffe600; margin-bottom: 2px;", "⚖️ Hybrid • 22% THC" }
                        div { style: "font-size: 6px; color: #8b8b9e; margin-bottom: 6px;", "Sweet berry aroma, balanced effects" }
                        div { style: "display: flex; justify-content: space-between; align-items: center;",
                            span { style: "font-size: 9px; color: #39ff14;", "฿1,600" }
                            span { style: "font-size: 6px; color: #ffe600; background: rgba(255,230,0,0.15); padding: 2px 6px; border-radius: 3px;", "82% match" }
                        }
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
