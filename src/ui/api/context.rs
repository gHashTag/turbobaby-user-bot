// API Context Provider for Dioxus
use dioxus::prelude::*;
use std::sync::Arc;
use crate::ui::api::client::ApiClient;

/// Hook to access ApiClient from context
pub fn use_api_client() -> Arc<ApiClient> {
    use_context::<Arc<ApiClient>>()
}

/// Get the API base URL from the browser's current origin (WASM-compatible)
fn get_base_url() -> String {
    web_sys::window()
        .and_then(|w| w.location().origin().ok())
        .unwrap_or_else(|| "http://localhost:3000".to_string())
}

/// Provider component for ApiClient
#[component]
pub fn ApiClientProvider(children: Element) -> Element {
    let api_client = use_hook(|| Arc::new(ApiClient::new(get_base_url())));

    provide_context(api_client);
    rsx! {
        { children }
    }
}
