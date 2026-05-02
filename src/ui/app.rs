// Root application component — Provides global Cart state
//
// Cart signal is shared across all screens via context.

use dioxus::prelude::*;
use crate::ui::routes::Routes;
use crate::ui::state::Cart;

#[component]
pub fn App() -> Element {
    // Provide global cart state
    use_context_provider(|| Signal::new(Cart::new()));

    rsx! {
        Routes {}
    }
}
