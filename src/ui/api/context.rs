// API Context Provider for Dioxus
use dioxus::prelude::*;
use std::sync::Arc;
use crate::ui::api::client::ApiClient;

/// Hook to access ApiClient from context
pub fn use_api_client() -> Arc<ApiClient> {
    use_context::<Arc<ApiClient>>()
}

pub fn api_base_url() -> String {
    web_sys::window()
        .and_then(|w| w.location().origin().ok())
        .map(|origin| {
            if origin.contains(":8080") || origin.contains(":3001") {
                "https://woody-weed-bot-production.up.railway.app".to_string()
            } else {
                origin
            }
        })
        .unwrap_or_else(|| "https://woody-weed-bot-production.up.railway.app".to_string())
}

fn get_base_url() -> String {
    api_base_url()
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
