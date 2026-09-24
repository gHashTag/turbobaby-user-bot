// Dioxus router configuration
//
// Routes wired to pure UI screens (no business logic)
// All screens are wrapped in LazyScreen for deferred rendering.

use crate::ui::components::lazy_screen::LazyScreen;
use crate::ui::screens::{
    ARHuntScreen, AccessoriesScreen, AdminScreen, CartScreen, CatalogScreen, CheckoutScreen,
    GameScreen, HomeScreen, LocationQuestScreen, MenuScreen, OrderDetailScreen, OrdersScreen,
    ProfileScreen, QuestScreen, ReferralsScreen, RideScreen, SuccessScreen, TeaScreen,
    TechTreeScreen, TreasureHuntScreen,
};
use crate::ui::share::{PendingOrder, PendingReorder, SharedProduct};
use dioxus::prelude::*;

/// Routes component that renders router
#[component]
pub fn Routes() -> Element {
    rsx! {
        Router::<Route> {}
    }
}

/// Application route enum (must be public for use in other modules)
#[derive(Clone, Routable, PartialEq, Debug)]
pub enum Route {
    #[route("/")]
    Home {},
    #[route("/menu")]
    Menu {},
    #[route("/sets")]
    Sets {},
    #[route("/sommelier")]
    Sommelier {},
    #[route("/accessories")]
    Accessories {},
    #[route("/tea")]
    Tea {},
    #[route("/events")]
    Events {},
    #[route("/events/:id")]
    EventDetail { id: String },
    #[route("/my-bookings")]
    MyBookings {},
    #[route("/cart")]
    Cart {},
    #[route("/checkout")]
    Checkout {},
    #[route("/success/:id")]
    Success { id: String },
    #[route("/orders")]
    Orders {},
    #[route("/orders/:id")]
    OrderDetail { id: String },
    #[route("/profile")]
    Profile {},
    // `/garden` stood here. The garden mechanic is removed, not repointed
    // (D5): its tables are dropped by migration 083 and no `/api/garden/*`
    // router was ever merged, so the route rendered a screen that could only
    // fail. There is deliberately no alias — unlike `/skate`, which still has
    // a renderer to fall back on, a garden has nothing left to show.
    #[route("/quest/:id")]
    Quest { id: String },
    #[route("/game")]
    Game {},
    #[route("/ride")]
    Ride {},
    /// Historical compatibility alias. It renders the Ride screen and never
    /// imports the retired skate asset.
    #[route("/skate")]
    Skate {},
    #[route("/referrals")]
    Referrals {},
    #[route("/treasure-hunt")]
    TreasureHunt {},
    #[route("/ar-hunt")]
    ArHunt {},
    #[route("/location-quest")]
    LocationQuest {},
    #[route("/tech-tree")]
    TechTree {},
    #[route("/admin")]
    Admin {},
    #[route("/:..route")]
    NotFound { route: Vec<String> },
}

// Route handlers using screen components — each wrapped in LazyScreen
// so the browser paints a skeleton before the heavy screen mounts.

#[component]
fn Home() -> Element {
    // The public landing page is the live bike catalog. HomeScreen remains a
    // narrow compatibility controller for historical Telegram deep links; it
    // mounts only long enough to route a pending cart/order/reorder/product
    // payload, so cannabis-era home content is no longer reachable at `/`
    // during an ordinary visit. A garden payload was a fifth reason to mount
    // it; that payload no longer parses (D5).
    let pending_product = use_context::<Signal<Option<SharedProduct>>>();
    let pending_order = use_context::<Signal<PendingOrder>>();
    let pending_cart = use_context::<Signal<bool>>();
    let pending_reorder = use_context::<Signal<PendingReorder>>();
    let has_compatibility_redirect = pending_product.read().is_some()
        || pending_order.read().0.is_some()
        || *pending_cart.read()
        || pending_reorder.read().0.is_some();

    rsx! {
        LazyScreen {
            if has_compatibility_redirect {
                HomeScreen {}
            } else {
                CatalogScreen {}
            }
        }
    }
}

#[component]
fn Menu() -> Element {
    rsx! {
        LazyScreen {
            MenuScreen {}
        }
    }
}

/// `/sets` survives as a compatibility alias, exactly as `/sommelier` does.
///
/// The screen behind it is deleted. `/api/sets` answers `{"sets":[]}` here —
/// the combos were the previous shop's, and no TurboBaby set has ever been
/// created — so the screen could only render an empty list. The *path* stays
/// because months of `startapp=sets` deep links sit in customers' Telegram
/// histories forever and a router miss is worse than the catalog.
#[component]
fn Sets() -> Element {
    rsx! {
        CatalogScreen {}
    }
}

/// `/sommelier` survives as a compatibility alias, exactly as `/skate` does.
///
/// The screen behind it is deleted — it recommended cannabis strains from a
/// retired endpoint (#2). The *path* stays because the bot has been sending
/// `startapp=sommelier` deep links for months and they sit in customers'
/// Telegram histories forever; dropping the route would turn every one of them
/// into a router miss. It lands on the catalog, which is where somebody asking
/// "what should I take" now belongs.
#[component]
fn Sommelier() -> Element {
    rsx! {
        CatalogScreen {}
    }
}

#[component]
fn Accessories() -> Element {
    rsx! {
        LazyScreen {
            AccessoriesScreen {}
        }
    }
}

#[component]
fn Tea() -> Element {
    rsx! {
        LazyScreen {
            TeaScreen {}
        }
    }
}

/// `/events`, `/events/:id` and `/my-bookings` survive as compatibility
/// aliases, exactly as `/sets` does. Events left every customer surface on the
/// owner's ruling of 2026-09-24 (rental only, Phuket only); the rows stay, the
/// events screens stay compiled and unmounted, and an old link or bookmark
/// lands on the catalog instead of a router miss or a hidden calendar.
#[component]
fn Events() -> Element {
    rsx! {
        CatalogScreen {}
    }
}

#[component]
fn EventDetail(id: String) -> Element {
    let _ = id;
    rsx! {
        CatalogScreen {}
    }
}

#[component]
fn MyBookings() -> Element {
    rsx! {
        CatalogScreen {}
    }
}

#[component]
fn Cart() -> Element {
    rsx! {
        LazyScreen {
            CartScreen {}
        }
    }
}

#[component]
fn Checkout() -> Element {
    rsx! {
        LazyScreen {
            CheckoutScreen {}
        }
    }
}

#[component]
fn Success(id: String) -> Element {
    rsx! {
        LazyScreen {
            SuccessScreen { id }
        }
    }
}

#[component]
fn Orders() -> Element {
    rsx! {
        LazyScreen {
            OrdersScreen {}
        }
    }
}

#[component]
fn OrderDetail(id: String) -> Element {
    rsx! {
        LazyScreen {
            OrderDetailScreen { id }
        }
    }
}

#[component]
fn Profile() -> Element {
    rsx! {
        LazyScreen {
            ProfileScreen {}
        }
    }
}

#[component]
fn Quest(id: String) -> Element {
    rsx! {
        LazyScreen {
            QuestScreen { id }
        }
    }
}

#[component]
fn Ride() -> Element {
    // Not wrapped in LazyScreen: the screen already defers its own weight by
    // importing three.js at runtime, and a second loading shell just delays
    // the canvas the player is waiting for.
    rsx! {
        RideScreen {}
    }
}

#[component]
fn Skate() -> Element {
    // Old shared links keep working, but there is no Skate renderer or asset:
    // this route is deliberately only a compatibility alias for Ride.
    rsx! {
        RideScreen {}
    }
}

#[component]
fn Game() -> Element {
    rsx! {
        LazyScreen { heavy: true,
            GameScreen {}
        }
    }
}

#[component]
fn Referrals() -> Element {
    rsx! {
        LazyScreen {
            ReferralsScreen {}
        }
    }
}

#[component]
fn TreasureHunt() -> Element {
    rsx! {
        LazyScreen { heavy: true,
            TreasureHuntScreen {}
        }
    }
}

#[component]
fn ArHunt() -> Element {
    rsx! {
        LazyScreen { heavy: true,
            ARHuntScreen {}
        }
    }
}

#[component]
fn LocationQuest() -> Element {
    rsx! {
        LazyScreen { heavy: true,
            LocationQuestScreen {}
        }
    }
}

#[component]
fn TechTree() -> Element {
    rsx! {
        LazyScreen { heavy: true,
            TechTreeScreen {}
        }
    }
}

#[component]
fn Admin() -> Element {
    rsx! {
        LazyScreen { heavy: true,
            AdminScreen {}
        }
    }
}

#[component]
fn NotFound(route: Vec<String>) -> Element {
    let path = route
        .iter()
        .map(|s| s.as_str())
        .collect::<Vec<&str>>()
        .join("/");
    rsx! {
        div { style: "
            min-height: 100vh;
            background: #0f0f1a;
            color: #e8e8e8;
            font-family: 'Press Start 2P', monospace;
            display: flex; flex-direction: column;
            align-items: center; justify-content: center;
            text-align: center; padding: 24px;
        ",
            div { style: "font-size: 48px; margin-bottom: 16px;", "🔍" }
            h1 { style: "font-size: 18px; color: #ff4757; margin-bottom: 8px;", "404" }
            p { style: "font-size: 12px; color: #8b8b9e;", "Page not found: {path}" }
        }
    }
}
