// Garden Screen — Plant growing + Woody Catch game
use dioxus::prelude::*;
use crate::ui::components::bottom_nav::BottomNav;
use crate::ui::game::Garden;
use crate::ui::game::WoodyCatch;

#[component]
pub fn GardenScreen() -> Element {
    let mut show_game = use_signal(|| false);
    let cart_count = 0u32;

    let is_game = *show_game.read();

    let garden_style = if is_game {
        "padding: 10px 16px; background: rgba(22,33,62,0.15); color: #8b8b9e; border: 4px solid #2a2a4a; font-size: 13px; font-weight: 700; cursor: pointer; border-radius: 20px;"
    } else {
        "padding: 10px 16px; background: #39ff14; color: #000; border: 4px solid #2d9e0f; font-size: 13px; font-weight: 700; cursor: pointer; border-radius: 20px;"
    };

    let game_style = if !is_game {
        "padding: 10px 16px; background: rgba(22,33,62,0.15); color: #8b8b9e; border: 4px solid #2a2a4a; font-size: 13px; font-weight: 700; cursor: pointer; border-radius: 20px;"
    } else {
        "padding: 10px 16px; background: #39ff14; color: #000; border: 4px solid #2d9e0f; font-size: 13px; font-weight: 700; cursor: pointer; border-radius: 20px;"
    };

    rsx! {
        div { style: "min-height: 100vh; background: #0f0f1a; color: #e8e8e8; padding-bottom: 80px;",

            // Toggle between Garden and Game
            div { style: "display: flex; gap: 8px; padding: 12px 16px; justify-content: center;",
                button {
                    style: "{garden_style}",
                    onclick: move |_| show_game.set(false),
                    "🌱 Garden"
                }
                button {
                    style: "{game_style}",
                    onclick: move |_| show_game.set(true),
                    "🎮 Game"
                }
            }

            // Show content based on selection
            if !is_game {
                Garden {}
            } else {
                WoodyCatch {}
            }
        }

        BottomNav { cart_count }
    }
}
