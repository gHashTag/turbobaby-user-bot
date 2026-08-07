// Dioxus router configuration
//
// Routes wired to pure UI screens (no business logic)
// All screens are wrapped in LazyScreen for deferred rendering.

use crate::ui::components::lazy_screen::LazyScreen;
use crate::ui::screens::{
    ARHuntScreen, AccessoriesScreen, AdminScreen, CartScreen, CheckoutScreen, EventDetailScreen,
    EventsScreen, GameScreen, GardenScreen, HomeScreen, LocationQuestScreen, MenuScreen,
    MyBookingsScreen, OrderDetailScreen, OrdersScreen, ProfileScreen, QuestScreen, ReferralsScreen,
    SetsScreen, SommelierScreen, SuccessScreen, TeaScreen, TechTreeScreen, TreasureHuntScreen,
};
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
    #[route("/garden")]
    Garden {},
    #[route("/quest/:id")]
    Quest { id: String },
    #[route("/game")]
    Game {},
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
    rsx! {
        LazyScreen {
            HomeScreen {}
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

#[component]
fn Sets() -> Element {
    rsx! {
        LazyScreen {
            SetsScreen {}
        }
    }
}

#[component]
fn Sommelier() -> Element {
    rsx! {
        LazyScreen {
            SommelierScreen {}
        }
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

#[component]
fn Events() -> Element {
    rsx! {
        LazyScreen {
            EventsScreen {}
        }
    }
}

#[component]
fn EventDetail(id: String) -> Element {
    rsx! {
        LazyScreen {
            EventDetailScreen { id }
        }
    }
}

#[component]
fn MyBookings() -> Element {
    rsx! {
        LazyScreen {
            MyBookingsScreen {}
        }
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
fn Garden() -> Element {
    rsx! {
        LazyScreen { heavy: true,
            GardenScreen {}
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
