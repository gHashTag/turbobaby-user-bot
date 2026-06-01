use crate::ui::components::bottom_nav::BottomNav;
use crate::ui::components::lazy_screen::LazyScreen;
use crate::ui::game::WoodyCatch;
use dioxus::prelude::*;

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
            LazyScreen { heavy: true, WoodyCatch {} }
            BottomNav {}
        }
    }
}
