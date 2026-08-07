// Root application component — Provides global Cart state
//
// Cart signal is shared across all screens via context.

use crate::ui::api::context::ApiClientProvider;
use crate::ui::api::http::fetch_text_authed;
use crate::ui::api::types::ServerCart;
use crate::ui::components::{install_error_handlers, ErrorOverlay, JsErrorItem};
use crate::ui::routes::Routes;
use crate::ui::share::{parse_order_start_param, parse_start_param, SharedProduct};
use crate::ui::state::{Cart, CartItem, CartItemType};
use crate::ui::telegram::TelegramProvider;
use dioxus::prelude::*;
use web_sys;

const CART_STORAGE_KEY: &str = "wwb_cart";
const CART_CLOUD_KEY: &str = "wwb_cart_cloud";

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

    // Persist cart to localStorage and Telegram CloudStorage on every change.
    let cart = use_context::<Signal<Cart>>();
    let initial_cart_for_cloud = cart.read().clone();
    use_effect(move || {
        let cart_data = cart.read().clone();
        #[cfg(target_arch = "wasm32")]
        {
            let json = serde_json::to_string(&cart_data).unwrap_or_default();
            if let Some(window) = web_sys::window() {
                if let Ok(Some(storage)) = window.local_storage() {
                    let _ = storage.set_item(CART_STORAGE_KEY, &json);
                }
            }
            let tg = crate::ui::telegram::TelegramApp;
            tg.cloud_storage_set(CART_CLOUD_KEY, &json);
        }
    });

    // Cycle #78: restore cart from Telegram CloudStorage asynchronously. We only
    // overwrite the locally-stored cart if the user hasn't modified it yet, so a
    // slow CloudStorage callback doesn't clobber an in-flight shopping session.
    #[cfg(target_arch = "wasm32")]
    use_hook(move || {
        let mut cart_signal = cart.clone();
        let initial = initial_cart_for_cloud;
        spawn(async move {
            let tg = crate::ui::telegram::TelegramApp;
            if let Some(json) = tg.cloud_storage_get(CART_CLOUD_KEY).await {
                if let Ok(parsed) = serde_json::from_str::<Cart>(&json) {
                    let current = cart_signal.read().clone();
                    if current == initial {
                        cart_signal.set(parsed);
                    }
                }
            }
        });
    });

    // Loop #11: recover the server-side cart if the local/CloudStorage cart is
    // empty. This restores the cart after cache loss or cross-device return.
    #[cfg(target_arch = "wasm32")]
    use_hook(move || {
        let mut cart_signal = cart.clone();
        spawn(async move {
            let tg = crate::ui::telegram::TelegramApp::init();
            let Some(tid) = tg.get_user_id() else { return };
            let init_data = tg.get_init_data();
            let url = format!(
                "{}/api/cart?telegram_id={tid}",
                crate::ui::api::context::api_base_url()
            );
            if let Ok(text) = fetch_text_authed(&url, &init_data).await {
                if let Ok(server_cart) = serde_json::from_str::<ServerCart>(&text) {
                    let current = cart_signal.read().clone();
                    if current.items.is_empty() && !server_cart.items.is_empty() {
                        let items: Vec<CartItem> = server_cart
                            .items
                            .into_iter()
                            .filter_map(server_item_to_cart_item)
                            .collect();
                        let mut recovered = Cart::new();
                        for item in items {
                            recovered.add_item(item);
                        }
                        cart_signal.set(recovered);
                    }
                }
            }
        });
    });

    // Global error overlay state
    use_context_provider(|| Signal::new(Vec::<JsErrorItem>::new()));
    let errors = use_context::<Signal<Vec<JsErrorItem>>>();

    // Product deep-link target parsed from Telegram.WebApp.initDataUnsafe.start_param.
    // Catalog screens read this signal to navigate/open the shared product modal.
    use_context_provider(|| Signal::new(None::<SharedProduct>));
    let pending_shared = use_context::<Signal<Option<SharedProduct>>>();

    // Order deep-link target parsed from Telegram.WebApp.initDataUnsafe.start_param.
    // OrdersScreen / OrderDetailScreen read this signal to open the referenced order.
    use_context_provider(|| Signal::new(None::<String>));
    let pending_order = use_context::<Signal<Option<String>>>();

    use_effect(move || {
        install_error_handlers(errors);
    });

    // Telegram WebApp initDataUnsafe may not be populated on the very first
    // render, so poll start_param briefly instead of reading it once.
    use_hook(move || {
        let mut pending = pending_shared.clone();
        let mut pending_order_id = pending_order.clone();
        spawn(async move {
            for _ in 0..30 {
                if pending.read().is_some() || pending_order_id.read().is_some() {
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
                    if let Some(order_id) = parse_order_start_param(&param) {
                        #[cfg(target_arch = "wasm32")]
                        web_sys::console::log_1(
                            &format!("[deeplink] resolved order {}", order_id).into(),
                        );
                        pending_order_id.set(Some(order_id));
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

#[cfg(target_arch = "wasm32")]
fn server_item_to_cart_item(item: crate::ui::api::types::ServerCartItem) -> Option<CartItem> {
    let item_type = match item.kind.as_str() {
        "strain" => CartItemType::Strain,
        "accessory" => CartItemType::Accessory,
        "tea" => CartItemType::Tea,
        "set" => CartItemType::Set,
        _ => return None,
    };
    let quantity = item.quantity.max(0) as u32;
    if quantity == 0 {
        return None;
    }
    Some(CartItem {
        id: item.catalog_id,
        name: item.name,
        price: if item.unit_price.is_finite() { item.unit_price.max(0.0) } else { 0.0 },
        quantity,
        image_url: item.image_url,
        item_type,
        fulfillment: None,
    })
}
