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
//! Two behaviours went out with the strain grid. Both were reported to the
//! orchestrator rather than silently dropped, and one has since been answered:
//!
//! - the pending **share target**, still open. This screen used to read a
//!   stored `ProductKind::Strain` target and open it as a product modal. Every
//!   kind in `share.rs` now lands here, which is the right destination for a
//!   product line that no longer exists but is not yet the right one for a
//!   bike: there is no `ProductKind::Bike` and no `/bikes/:id` route to give it
//!   — `bike_detail.rs` is a modal inside the catalog, not a routable screen —
//!   so a shared bike opens the fleet rather than the bike. Deep-linking a
//!   specific bike needs that route first.
//! - the **prefetch**, closed. This bullet claimed `prefetch.rs` still warmed
//!   `/api/strains` for `Route::Menu {}` long after it had been repointed at
//!   `/api/bikes` (`prefetch.rs:15`, which says so in a comment of its own). A
//!   note describing outstanding work that is already done is worse than no
//!   note: it is a standing invitation to go and re-fix it.

use crate::ui::screens::catalog_screen::CatalogScreen;
use dioxus::prelude::*;

/// The fleet catalog, under its historical route name.
#[component]
pub fn MenuScreen() -> Element {
    rsx! {
        CatalogScreen {}
    }
}
