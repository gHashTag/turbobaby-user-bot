// API Context Provider for Dioxus
use dioxus::prelude::*;
use std::sync::Arc;
use crate::ui::api::client::ApiClient;

/// Hook to access ApiClient from context
pub fn use_api_client() -> Arc<ApiClient> {
    use_context::<Arc<ApiClient>>()
}

pub fn api_base_url() -> String {
    // Detect if running through MCP proxy (port 9002)
    let href = web_sys::window()
        .and_then(|w| w.location().href().ok())
        .unwrap_or_default();

    // For local development, use backend via same-origin (handled by proxy)
    // The backend is served via Axum which proxies API requests
    if href.contains("127.0.0.1:8080") || href.contains("localhost:8080") {
        // Use same-origin - backend serves both API and frontend via Axum
        return "".to_string();
    }

    // Otherwise use same-origin (production or other setups)
    "".to_string()
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
