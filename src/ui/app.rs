// Root application component — Provides global Cart state
//
// Cart signal is shared across all screens via context.

use crate::ui::api::context::ApiClientProvider;
use crate::ui::components::{install_error_handlers, ErrorOverlay, JsErrorItem};
use crate::ui::routes::Routes;
use crate::ui::share::{parse_start_param, SharedProduct};
use crate::ui::state::Cart;
use crate::ui::telegram::TelegramProvider;
use dioxus::prelude::*;
use web_sys;

const CART_STORAGE_KEY: &str = "wwb_cart";

#[component]
pub fn App() -> Element {
    #[cfg(target_arch = "wasm32")]
    web_sys::console::log_1(&"[WASM] Step 5: App component rendering".into());
    #[cfg(target_arch = "wasm32")]
    web_sys::console::log_1(&"[WASM] Step 6: App mounted".into());
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

    // Product deep-link target parsed from Telegram.WebApp.initDataUnsafe.start_param.
    // Catalog screens read this signal to navigate/open the shared product modal.
    use_context_provider(|| Signal::new(None::<SharedProduct>));
    let pending_shared = use_context::<Signal<Option<SharedProduct>>>();

    use_effect(move || {
        install_error_handlers(errors);
    });

    // Telegram WebApp initDataUnsafe may not be populated on the very first
    // render, so poll start_param briefly instead of reading it once.
    use_hook(move || {
        let mut pending = pending_shared.clone();
        spawn(async move {
            for _ in 0..30 {
                if pending.read().is_some() {
                    return;
                }
                if let Some(param) = crate::ui::telegram::TelegramApp::init().start_param() {
                    if let Some(product) = parse_start_param(&param) {
                        #[cfg(target_arch = "wasm32")]
                        web_sys::console::log_1(
                            &format!("[deeplink] resolved {:?} {}", product.kind, product.id)
                                .into(),
                        );
                        pending.set(Some(product));
                        return;
                    }
                }
                #[cfg(target_arch = "wasm32")]
                {
                    gloo_timers::future::TimeoutFuture::new(100).await;
                }
                #[cfg(not(target_arch = "wasm32"))]
                {
                    break;
                }
            }
        });
    });

    rsx! {
        TelegramProvider {
            ApiClientProvider {
                ErrorOverlay {}
                Routes {}
            }
        }
    }
}
