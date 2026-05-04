use dioxus::prelude::*;
use crate::ui::routes::Route;
use crate::ui::assets;

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

            // Member Card — tier-based image
            {
                // Default tier: silver. In a real app this comes from user context.
                let tier = "silver";
                let card_url = match tier {
                    "gold" | "platinum" | "diamond" | "woody" => assets::member_cards::GOLD_WEBP,
                    "silver" => assets::member_cards::SILVER_WEBP,
                    _ => assets::member_cards::BRONZE_WEBP,
                };
                rsx! {
                    div { style: "max-width: 380px; margin: 0 auto 20px; padding: 0 16px;",
                        img {
                            src: "{card_url}",
                            alt: "Member Card",
                            style: "width: 100%; max-width: 320px; border-radius: 16px; display: block; margin: 0 auto; box-shadow: 0 0 24px rgba(192,192,192,0.25);",
                        }
                        // Stats row below the card image
                        div { style: "display: grid; grid-template-columns: repeat(3, 1fr); gap: 8px; margin-top: 12px;",
                            div { style: "text-align: center; padding: 8px 4px; background: #16213e; border: 2px solid #2a2a4a; border-radius: 8px;",
                                div { style: "font-size: 12px; color: #c0c0c0; margin-bottom: 4px;", "$750" }
                                div { style: "font-size: 6px; color: #888;", "SPENT" }
                            }
                            div { style: "text-align: center; padding: 8px 4px; background: #16213e; border: 2px solid #2a2a4a; border-radius: 8px;",
                                div { style: "font-size: 12px; color: #c0c0c0; margin-bottom: 4px;", "24" }
                                div { style: "font-size: 6px; color: #888;", "ORDERS" }
                            }
                            div { style: "text-align: center; padding: 8px 4px; background: #16213e; border: 2px solid #2a2a4a; border-radius: 8px;",
                                div { style: "font-size: 12px; color: #c0c0c0; margin-bottom: 4px;", "7" }
                                div { style: "font-size: 6px; color: #888;", "PLANTS" }
                            }
                        }
                        // Progress bar toward next tier
                        div { style: "margin-top: 10px;",
                            div { style: "height: 8px; background: rgba(0,0,0,0.4); border-radius: 4px; overflow: hidden;",
                                div { style: "height: 100%; width: 62%; border-radius: 4px; background: linear-gradient(90deg, #c0c0c0, #d8d8d8);" }
                            }
                            div { style: "display: flex; justify-content: space-between; font-size: 7px; margin-top: 6px;",
                                span { style: "color: #888;", "$750 / $1200" }
                                span { style: "color: #c0c0c0;", "→ Gold" }
                            }
                        }
                    }
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
