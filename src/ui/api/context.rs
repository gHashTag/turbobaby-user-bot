// API Context Provider for Dioxus
use crate::ui::api::client::ApiClient;
use dioxus::prelude::*;
use std::sync::Arc;

/// Hook to access ApiClient from context
pub fn use_api_client() -> Arc<ApiClient> {
    use_context::<Arc<ApiClient>>()
}

pub fn api_base_url() -> String {
    // Reqwest 0.11 in WASM requires absolute URLs ("builder error: relative URL
    // without a base"). Always return window.location.origin so /api/* calls
    // resolve to a real https://host URL on the current page's origin.
    web_sys::window()
        .and_then(|w| w.location().origin().ok())
        .unwrap_or_default()
}

fn get_base_url() -> String {
    api_base_url()
}

/// Provider component for ApiClient
///
/// The provided client carries no initData: every screen that needs a proof
/// reads it from the bridge itself (`use_telegram_init_data`) and goes through
/// the free helpers in `api::http`. `ApiClient::anonymous` says that out loud,
/// which is why this call cannot fail — `ApiClient::new` is fallible only
/// because an oversize proof is a refusal now, and there is no proof here.
#[component]
pub fn ApiClientProvider(children: Element) -> Element {
    let api_client = use_hook(|| Arc::new(ApiClient::anonymous(get_base_url())));

    provide_context(api_client);
    rsx! {
        { children }
    }
}
