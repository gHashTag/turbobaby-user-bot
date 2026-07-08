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
            padding-bottom: 80px;
        ",
            WoodyShop {}
            BottomNav {}
        }
    }
}
