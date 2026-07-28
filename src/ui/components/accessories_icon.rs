use dioxus::prelude::*;

/// Coffeeshop-style grinder icon for the Accessories tab.
/// Replaces the previous "tools/hardware" emoji so the tab feels like
/// papers / grinders / lighters rather than a wrench set.
#[component]
pub fn AccessoriesIcon() -> Element {
    rsx! {
        svg {
            width: "22",
            height: "22",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            // Top cap of a two-piece grinder.
            path { d: "M5 10V8a1 1 0 0 1 1-1h12a1 1 0 0 1 1 1v2" }
            // Main grinding chamber.
            path { d: "M4 10h16a2 2 0 0 1 2 2v2a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2v-2a2 2 0 0 1 2-2z" }
            // Bottom collection chamber.
            path { d: "M5 14v2a1 1 0 0 0 1 1h12a1 1 0 0 0 1-1v-2" }
            // Center magnet / grinding teeth hint.
            line { x1: "12", y1: "10", x2: "12", y2: "14" }
            line { x1: "9", y1: "12", x2: "15", y2: "12" }
        }
    }
}
