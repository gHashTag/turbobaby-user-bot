use dioxus::prelude::*;
use crate::ui::components::bottom_nav::BottomNav;
use crate::ui::game::garden::Garden;

#[component]
pub fn GardenScreen() -> Element {
    rsx! {
        div { style: "min-height: 100vh; background: #0f0f1a; color: #e8e8e8; font-family: 'Press Start 2P', monospace; padding-bottom: 80px;",
            Garden {}
            BottomNav {}
        }
    }
}
