use dioxus::prelude::*;
use crate::ui::game::WoodyCatch;
use crate::ui::routes::Route;

#[component]
pub fn GameScreen() -> Element {
    rsx! {
        div { style: "
            min-height: 100vh;
            background: #0c1222;
            display: flex; flex-direction: column;
            align-items: center;
            padding: 8px 8px 80px;
        ",
            WoodyCatch {}
        }

        // Bottom navigation (same pattern as garden_screen)
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
            Link { to: Route::Game {},
                div { style: "text-align: center; cursor: pointer;",
                    div { style: "font-size: 20px;", "🎮" }
                    div { style: "font-size: 6px; color: #39ff14; margin-top: 2px; text-shadow: 0 0 6px rgba(57,255,20,0.5);", "Game" }
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
