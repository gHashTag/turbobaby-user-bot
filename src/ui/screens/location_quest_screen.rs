// Location Quest Screen — GPS-based quest with location purchases
use dioxus::prelude::*;

#[component]
pub fn LocationQuestScreen() -> Element {
    rsx! {
        div { style: "
            min-height: 100vh;
            background: #0f0f1a;
            color: #e8e8e8;
            font-family: 'Press Start 2P', monospace;
            padding: 20px 16px;
            padding-bottom: 80px;
        ",
            div { style: "text-align: center; margin-bottom: 20px;",
                h1 { style: "font-size: 18px; color: #4caf50; text-shadow: 0 0 8px rgba(76,175,80,0.5);",
                    "\u{1F4CD} Location Quests"
                }
                p { style: "font-size: 18px; color: #8b8b9e; margin-top: 6px;",
                    "Complete quests at real locations around the island"
                }
            }

            // Progress
            div { style: "
                background: #1a1a2e; border: 2px solid #2a2a4a; border-radius: 8px;
                padding: 12px; margin-bottom: 16px; box-shadow: 4px 4px 0 #000;
            ",
                div { style: "display: flex; justify-content: space-between; margin-bottom: 8px;",
                    span { style: "font-size: 18px; color: #4caf50;", "\u{1F3AF} Quests" }
                    span { style: "font-size: 18px; color: #39ff14;", "2/5" }
                }
                div { style: "background: #0f0f1a; border-radius: 8px; height: 8px; overflow: hidden;",
                    div { style: "background: linear-gradient(90deg, #4caf50, #8bc34a); height: 100%; width: 40%; border-radius: 8px;" }
                }
            }

            // Quest cards
            div { style: "display: flex; flex-direction: column; gap: 10px;",
                // Beach Cleanup - completed
                div { style: "background: #1a1a2e; border: 2px solid #39ff14; border-radius: 8px; padding: 12px; box-shadow: 4px 4px 0 #000;",
                    div { style: "display: flex; align-items: center; gap: 10px;",
                        span { style: "font-size: 28px;", "\u{1F33F}" }
                        div { style: "flex: 1;",
                            div { style: "font-size: 14px; font-weight: 700; color: #fff; margin-bottom: 4px;", "Beach Cleanup" }
                            div { style: "font-size: 18px; color: #c9c9d4; margin-bottom: 4px;", "Collect 5 items of trash on the beach" }
                            span { style: "font-size: 18px; color: #39ff14;", "\u{2705} Completed!" }
                        }
                    }
                }
                // Sunset Photo - completed
                div { style: "background: #1a1a2e; border: 2px solid #39ff14; border-radius: 8px; padding: 12px; box-shadow: 4px 4px 0 #000;",
                    div { style: "display: flex; align-items: center; gap: 10px;",
                        span { style: "font-size: 28px;", "\u{1F304}" }
                        div { style: "flex: 1;",
                            div { style: "font-size: 14px; font-weight: 700; color: #fff; margin-bottom: 4px;", "Sunset Photo" }
                            div { style: "font-size: 18px; color: #c9c9d4; margin-bottom: 4px;", "Take a photo at the viewpoint during sunset" }
                            span { style: "font-size: 18px; color: #39ff14;", "\u{2705} Completed!" }
                        }
                    }
                }
                // Temple Visit - locked
                div { style: "background: #1a1a2e; border: 2px solid #2a2a4a; border-radius: 8px; padding: 12px; box-shadow: 4px 4px 0 #000;",
                    div { style: "display: flex; align-items: center; gap: 10px;",
                        span { style: "font-size: 28px;", "\u{26E9}\u{FE0F}" }
                        div { style: "flex: 1;",
                            div { style: "font-size: 14px; font-weight: 700; color: #fff; margin-bottom: 4px;", "Temple Visit" }
                            div { style: "font-size: 18px; color: #c9c9d4; margin-bottom: 4px;", "Visit the ancient temple and light incense" }
                            span { style: "font-size: 18px; color: #8b8b9e;", "\u{1F4CD} 800m away \u{2022} \u{1F4B0} 100 pts" }
                        }
                    }
                }
                // Market Haggler - locked
                div { style: "background: #1a1a2e; border: 2px solid #2a2a4a; border-radius: 8px; padding: 12px; box-shadow: 4px 4px 0 #000;",
                    div { style: "display: flex; align-items: center; gap: 10px;",
                        span { style: "font-size: 28px;", "\u{1F6CD}\u{FE0F}" }
                        div { style: "flex: 1;",
                            div { style: "font-size: 14px; font-weight: 700; color: #fff; margin-bottom: 4px;", "Market Haggler" }
                            div { style: "font-size: 18px; color: #c9c9d4; margin-bottom: 4px;", "Buy 3 items at the night market" }
                            span { style: "font-size: 18px; color: #8b8b9e;", "\u{1F4CD} 1.2km away \u{2022} \u{1F396}\u{FE0F} Trader badge" }
                        }
                    }
                }
                // Jungle Trek - locked
                div { style: "background: #1a1a2e; border: 2px solid #2a2a4a; border-radius: 8px; padding: 12px; box-shadow: 4px 4px 0 #000;",
                    div { style: "display: flex; align-items: center; gap: 10px;",
                        span { style: "font-size: 28px;", "\u{1F333}" }
                        div { style: "flex: 1;",
                            div { style: "font-size: 14px; font-weight: 700; color: #fff; margin-bottom: 4px;", "Jungle Trek" }
                            div { style: "font-size: 18px; color: #c9c9d4; margin-bottom: 4px;", "Complete the jungle trail to the waterfall" }
                            span { style: "font-size: 18px; color: #8b8b9e;", "\u{1F4CD} 3.0km away \u{2022} \u{1F4B0} 200 pts + Rare seed" }
                        }
                    }
                }
            }
        }
    }
}
