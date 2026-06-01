// Lightweight navigation prefetch — warms HTTP cache before user taps a link.
//
// On desktop `onmouseenter` fires ~100-300 ms before click, giving the
// browser a head-start on the connection + cache. On mobile we skip
// prefetch (touch is too late to help) but the code is harmless.

use crate::ui::api::context::api_base_url;
use crate::ui::routes::Route;

fn endpoint_for(route: &Route) -> Option<String> {
    let base = api_base_url();
    let path = match route {
        Route::Menu {} => "/api/strains",
        Route::Sets {} => "/api/sets",
        Route::Accessories {} => "/api/accessories",
        Route::Tea {} => "/api/tea",
        Route::Orders {} => "/api/orders",
        Route::Profile {} => "/api/user/profile",
        Route::Garden {} => "/api/garden",
        _ => return None,
    };
    Some(format!("{}{}", base, path))
}

/// Fire a background GET to warm the HTTP cache for the given route.
/// Safe to call on every hover/tap — the request is fire-and-forget.
pub fn prefetch_route(route: &Route) {
    #[cfg(target_arch = "wasm32")]
    {
        let Some(url) = endpoint_for(route) else {
            return;
        };
        wasm_bindgen_futures::spawn_local(async move {
            let _ = gloo_net::http::Request::get(&url).send().await;
        });
    }
}
