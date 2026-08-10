use crate::ui::components::bottom_nav::BottomNav;
use crate::ui::game::quest::Quest;
use dioxus::prelude::*;

#[component]
pub fn QuestScreen(id: String) -> Element {
    rsx! {
        div { style: "min-height: 100vh; background: #0f0f1a; color: #e8e8e8; font-family: 'Press Start 2P', monospace; padding-bottom: calc(96px + env(safe-area-inset-bottom));",
            Quest {}
            BottomNav {}
        }
    }
}
