use dioxus::prelude::*;
use crate::ui::routes::Route;

#[component]
pub fn QuestScreen(id: String) -> Element {
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
                h1 { style: "font-size: 12px; color: #00e5ff; text-shadow: 0 0 8px rgba(0,229,255,0.5);", "🎯 Quest Mode" }
                p { style: "font-size: 7px; color: #8b8b9e; margin-top: 4px;", "Scan QR codes at locations to earn rewards!" }
            }

            // Scan area
            div { style: "
                max-width: 380px; margin: 0 auto 20px;
                background: rgba(0,229,255,0.05);
                border: 2px dashed #00e5ff;
                border-radius: 16px;
                padding: 32px;
                text-align: center;
            ",
                div { style: "font-size: 48px; margin-bottom: 12px;", "📷" }
                div { style: "font-size: 10px; color: #00e5ff; text-shadow: 0 0 6px rgba(0,229,255,0.4); margin-bottom: 8px;", "Scan QR Code" }
                div { style: "font-size: 7px; color: #666;", "Point camera at location QR to check in" }
            }

            // Quest cards
            div { style: "max-width: 380px; margin: 0 auto; padding: 0 16px;",

                // Completed quest
                div { style: "
                    background: #1a1a2e;
                    border: 2px solid #39ff14;
                    border-radius: 12px;
                    padding: 16px;
                    display: flex; gap: 12px; align-items: center;
                    margin-bottom: 12px;
                    box-shadow: 0 0 12px rgba(57,255,20,0.2);
                ",
                    div { style: "font-size: 32px; min-width: 48px; text-align: center;", "🏪" }
                    div { style: "flex: 1;",
                        div { style: "font-size: 10px; color: #e0e0e0; margin-bottom: 4px;", "Woody Central" }
                        div { style: "font-size: 7px; color: #888; margin-bottom: 6px;", "Visit the main dispensary" }
                        div { style: "font-size: 7px; color: #ffd700; margin-bottom: 6px;", "🎁 Reward: 50 XP + Free Seed" }
                        div { style: "height: 6px; background: rgba(0,0,0,0.4); border-radius: 3px; overflow: hidden;",
                            div { style: "height: 100%; width: 100%; border-radius: 3px; background: linear-gradient(90deg, #00e5ff, #39ff14);" }
                        }
                    }
                    span { style: "
                        font-size: 7px; padding: 4px 8px;
                        border-radius: 4px; white-space: nowrap;
                        background: rgba(57,255,20,0.2); color: #39ff14;
                    ", "✓ DONE" }
                }

                // Active quest
                div { style: "
                    background: #1a1a2e;
                    border: 2px solid rgba(255,255,255,0.1);
                    border-radius: 12px;
                    padding: 16px;
                    display: flex; gap: 12px; align-items: center;
                    margin-bottom: 12px;
                ",
                    div { style: "font-size: 32px; min-width: 48px; text-align: center;", "🌿" }
                    div { style: "flex: 1;",
                        div { style: "font-size: 10px; color: #e0e0e0; margin-bottom: 4px;", "Green Lab" }
                        div { style: "font-size: 7px; color: #888; margin-bottom: 6px;", "Find the hidden grow lab" }
                        div { style: "font-size: 7px; color: #ffd700; margin-bottom: 6px;", "🎁 Reward: 100 XP + Rare Strain" }
                        div { style: "height: 6px; background: rgba(0,0,0,0.4); border-radius: 3px; overflow: hidden;",
                            div { style: "height: 100%; width: 60%; border-radius: 3px; background: linear-gradient(90deg, #00e5ff, #39ff14);" }
                        }
                    }
                    span { style: "
                        font-size: 7px; padding: 4px 8px;
                        border-radius: 4px; white-space: nowrap;
                        background: rgba(0,229,255,0.2); color: #00e5ff;
                    ", "3/5" }
                }

                // Active quest 2
                div { style: "
                    background: #1a1a2e;
                    border: 2px solid rgba(255,255,255,0.1);
                    border-radius: 12px;
                    padding: 16px;
                    display: flex; gap: 12px; align-items: center;
                    margin-bottom: 12px;
                ",
                    div { style: "font-size: 32px; min-width: 48px; text-align: center;", "🏔️" }
                    div { style: "flex: 1;",
                        div { style: "font-size: 10px; color: #e0e0e0; margin-bottom: 4px;", "Mountain Peak" }
                        div { style: "font-size: 7px; color: #888; margin-bottom: 6px;", "Reach the summit grow site" }
                        div { style: "font-size: 7px; color: #ffd700; margin-bottom: 6px;", "🎁 Reward: 200 XP + Legend Badge" }
                        div { style: "height: 6px; background: rgba(0,0,0,0.4); border-radius: 3px; overflow: hidden;",
                            div { style: "height: 100%; width: 20%; border-radius: 3px; background: linear-gradient(90deg, #00e5ff, #39ff14);" }
                        }
                    }
                    span { style: "
                        font-size: 7px; padding: 4px 8px;
                        border-radius: 4px; white-space: nowrap;
                        background: rgba(0,229,255,0.2); color: #00e5ff;
                    ", "1/5" }
                }

                // Locked quest
                div { style: "
                    background: #1a1a2e;
                    border: 2px solid rgba(255,255,255,0.1);
                    border-radius: 12px;
                    padding: 16px;
                    display: flex; gap: 12px; align-items: center;
                    margin-bottom: 12px;
                    opacity: 0.5;
                ",
                    div { style: "font-size: 32px; min-width: 48px; text-align: center;", "🔒" }
                    div { style: "flex: 1;",
                        div { style: "font-size: 10px; color: #e0e0e0; margin-bottom: 4px;", "Secret Garden" }
                        div { style: "font-size: 7px; color: #888; margin-bottom: 6px;", "Complete 3 quests to unlock" }
                        div { style: "font-size: 7px; color: #ffd700;", "🎁 Reward: ???" }
                    }
                    span { style: "
                        font-size: 7px; padding: 4px 8px;
                        border-radius: 4px; white-space: nowrap;
                        background: rgba(255,255,255,0.05); color: #666;
                    ", "LOCKED" }
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
