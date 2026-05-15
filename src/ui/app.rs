// Root application component — Provides global Cart state
//
// Cart signal is shared across all screens via context.

use dioxus::prelude::*;
use crate::ui::routes::Routes;
use crate::ui::state::{Cart, LanguageProvider};
use crate::ui::api::context::ApiClientProvider;

#[component]
pub fn App() -> Element {
    // Provide global cart state
    use_context_provider(|| Signal::new(Cart::new()));

    rsx! {
        ApiClientProvider {
            LanguageProvider {
                Routes {}
            }
        }
    }
}
