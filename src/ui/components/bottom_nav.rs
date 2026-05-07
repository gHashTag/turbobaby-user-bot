// Shared 9-tab Bottom Navigation — matching old TS project NavTabs
use dioxus::prelude::*;
use crate::ui::routes::Route;

/// 9-tab bottom navigation matching old woody-woodpecker NavTabs.tsx:
/// 🪵 Home · 🌿 Menu · 🎁 Sets · 🛠️ Gear · 🍵 Tea · 🌱 Garden · 🗺️ Quest · 🛒 Cart · 👤 Profile
#[component]
pub fn BottomNav(#[props(default)] cart_count: u32) -> Element {
    rsx! {
        nav { style: "
            position: fixed;
            bottom: 0;
            left: 0;
            right: 0;
            background: #1a1a2e;
            border-top: 1px solid #2a2a4a;
            display: flex;
            justify-content: space-around;
            overflow-x: auto;
            padding: 8px 0 6px;
            z-index: 100;
            -webkit-overflow-scrolling: touch;
        ",
            // 🪵 Home
            Link { to: Route::Home {},
                div { style: "text-align: center; cursor: pointer; min-width: 36px;",
                    div { style: "font-size: 14px;", "🪵" }
                    div { style: "font-size: 9px; color: #8b8b9e; margin-top: 2px;", "Home" }
                }
            }
            // 🌿 Menu
            Link { to: Route::Menu {},
                div { style: "text-align: center; cursor: pointer; min-width: 36px;",
                    div { style: "font-size: 14px;", "🌿" }
                    div { style: "font-size: 9px; color: #8b8b9e; margin-top: 2px;", "Menu" }
                }
            }
            // 🎁 Sets
            Link { to: Route::Sets {},
                div { style: "text-align: center; cursor: pointer; min-width: 36px;",
                    div { style: "font-size: 14px;", "🎁" }
                    div { style: "font-size: 9px; color: #8b8b9e; margin-top: 2px;", "Sets" }
                }
            }
            // 🛠️ Gear
            Link { to: Route::Accessories {},
                div { style: "text-align: center; cursor: pointer; min-width: 36px;",
                    div { style: "font-size: 14px;", "🛠️" }
                    div { style: "font-size: 9px; color: #8b8b9e; margin-top: 2px;", "Gear" }
                }
            }
            // 🍵 Tea
            Link { to: Route::Tea {},
                div { style: "text-align: center; cursor: pointer; min-width: 36px;",
                    div { style: "font-size: 14px;", "🍵" }
                    div { style: "font-size: 9px; color: #8b8b9e; margin-top: 2px;", "Tea" }
                }
            }
            // 🌱 Garden
            Link { to: Route::Garden {},
                div { style: "text-align: center; cursor: pointer; min-width: 36px;",
                    div { style: "font-size: 14px;", "🌱" }
                    div { style: "font-size: 9px; color: #8b8b9e; margin-top: 2px;", "Garden" }
                }
            }
            // 🗺️ Quest
            Link { to: Route::Quest { id: "daily".to_string() },
                div { style: "text-align: center; cursor: pointer; min-width: 36px;",
                    div { style: "font-size: 14px;", "🗺️" }
                    div { style: "font-size: 9px; color: #8b8b9e; margin-top: 2px;", "Quest" }
                }
            }
            // 🛒 Cart
            Link { to: Route::Cart {},
                div { style: "text-align: center; cursor: pointer; position: relative; min-width: 36px;",
                    div { style: "font-size: 14px;", "🛒" }
                    if cart_count > 0 {
                        div { style: "
                            position: absolute; top: -4px; right: -6px;
                            background: #ff4757; color: white;
                            font-size: 9px; padding: 1px 5px;
                            border-radius: 8px; min-width: 14px; text-align: center;
                        ", "{cart_count}" }
                    }
                    div { style: "font-size: 9px; color: #8b8b9e; margin-top: 2px;", "Cart" }
                }
            }
            // 👤 Profile
            Link { to: Route::Profile {},
                div { style: "text-align: center; cursor: pointer; min-width: 36px;",
                    div { style: "font-size: 14px;", "👤" }
                    div { style: "font-size: 9px; color: #8b8b9e; margin-top: 2px;", "Profile" }
                }
            }
        }
    }
}
