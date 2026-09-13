//! `/menu` — kept as a route, repointed at the fleet.
//!
//! This file used to be the cannabis catalog: 777 lines that fetched
//! `/api/strains`, sorted by THC, rendered a "strain of the day" hero and
//! sativa / indica / hybrid filter tabs, and added grams to a cart. None of
//! that has a motorbike counterpart, and `strains` is dropped by the bike
//! migrations, so leaving it would have shipped a grid of spinners over a dead
//! endpoint.
//!
//! It is now a delegation. `/menu` is wired in three places outside this
//! worker's paths — `routes.rs`, `screens/mod.rs` and the bottom nav — so the
//! component name survives and the body lives in
//! [`CatalogScreen`](crate::ui::screens::catalog_screen::CatalogScreen). When
//! the orchestrator adds a route of its own for the fleet, this file becomes a
//! redirect and then nothing at all.
//!
//! Two behaviours went out with the strain grid. They are reported to the
//! orchestrator rather than silently dropped:
//!
//! - the pending **share target**. This screen used to read a stored
//!   `ProductKind::Strain` target and open it as a product modal; `share.rs`
//!   still maps that kind to `Route::Menu {}`. Deep links need a `Bike` kind
//!   pointing at the bike detail route before they work again.
//! - `prefetch.rs` still warms `/api/strains` for `Route::Menu {}`.

use crate::ui::screens::catalog_screen::CatalogScreen;
use dioxus::prelude::*;

/// The fleet catalog, under its historical route name.
#[component]
pub fn MenuScreen() -> Element {
    rsx! {
        CatalogScreen {}
    }
}
