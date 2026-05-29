// Root application component — Provides global Cart state
//
// Cart signal is shared across all screens via context.

use dioxus::prelude::*;
use web_sys;
use crate::ui::routes::Routes;
use crate::ui::state::{Cart, LanguageProvider};
use crate::ui::api::context::ApiClientProvider;
use crate::ui::telegram::TelegramProvider;
use crate::ui::components::{ErrorOverlay, JsErrorItem, install_error_handlers};

const CART_STORAGE_KEY: &str = "wwb_cart";

#[component]
pub fn App() -> Element {
    // Provide global cart state — try to restore from localStorage on WASM
    use_context_provider(|| {
        let mut cart = Cart::new();
        #[cfg(target_arch = "wasm32")]
        {
            if let Some(window) = web_sys::window() {
                if let Ok(Some(storage)) = window.local_storage() {
                    if let Ok(Some(json)) = storage.get_item(CART_STORAGE_KEY) {
                        if json.len() <= 1_000_000 {
                            if let Ok(parsed) = serde_json::from_str::<Cart>(&json) {
                                cart = parsed;
                            }
                        }
                    }
                }
            }
        }
        Signal::new(cart)
    });

    // Persist cart to localStorage on every change
    let cart = use_context::<Signal<Cart>>();
    use_effect(move || {
        let cart_data = cart.read().clone();
        #[cfg(target_arch = "wasm32")]
        {
            if let Some(window) = web_sys::window() {
                if let Ok(Some(storage)) = window.local_storage() {
                    let _ = storage.set_item(
                        CART_STORAGE_KEY,
                        &serde_json::to_string(&cart_data).unwrap_or_default(),
                    );
                }
            }
        }
    });

    // Global error overlay state
    use_context_provider(|| Signal::new(Vec::<JsErrorItem>::new()));
    let errors = use_context::<Signal<Vec<JsErrorItem>>>();

    use_effect(move || {
        install_error_handlers(errors.clone());
    });

    rsx! {
        TelegramProvider {
            ApiClientProvider {
                LanguageProvider {
                    ErrorOverlay {}
                    Routes {}
                }
            }
        }
    }
}
