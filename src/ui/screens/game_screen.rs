use crate::ui::components::bottom_nav::BottomNav;
use crate::ui::game::WoodyShop;
use dioxus::prelude::*;

#[component]
pub fn GameScreen() -> Element {
    rsx! {
        div { style: "
            min-height: 100vh;
            background: #0f0f1a;
            display: flex; flex-direction: column;
            align-items: center;
            padding-bottom: calc(96px + env(safe-area-inset-bottom));
        ",
            WoodyShop {}
            BottomNav {}
        }
    }
}
