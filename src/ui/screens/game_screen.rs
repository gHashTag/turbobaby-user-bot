use dioxus::prelude::*;
use crate::ui::game::WoodyCatch;
use crate::ui::components::bottom_nav::BottomNav;

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
            BottomNav {}
        }
    }
}
