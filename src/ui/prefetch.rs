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
        // The catalog is the fleet since the rebrand; /api/strains died with
        // the strains table (083) and every warm-up was a 404.
        Route::Home {} | Route::Menu {} => "/api/bikes",
        Route::Sets {} => "/api/sets",
        Route::Accessories {} => "/api/accessories",
        // `tea_screen.rs` fetches `/api/tea-products`, and that is the route
        // `catalog::routes()` registers. `/api/tea` was never one: it warmed a
        // path that falls through to the SPA fallback, so the prefetch cached
        // `index.html` under a URL the screen would then miss on. Third entry
        // in this function to be wrong the same way — hence
        // `tests/ui_endpoints_exist.rs`, which now checks all three.
        Route::Tea {} => "/api/tea-products",
        // The customer's orders live at /api/orders/user/{id} and the
        // prefetcher does not know the id; warming the admin /api/orders
        // instead earned a 401 per hover (production logs, 2026-09-13).
        Route::Orders {} => return None,
        // /api/user/profile is not a route the server exposes. Warming a dead
        // endpoint is log noise, not speed. (`Route::Garden` was listed here
        // too, for the same reason; the route itself is gone now — D5.)
        Route::Profile {} => return None,
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
