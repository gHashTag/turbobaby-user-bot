// Dioxus router configuration
//
// Routes wired to pure UI screens (no business logic)

use dioxus::prelude::*;
use crate::ui::screens::{
    HomeScreen,
    MenuScreen,
    SetsScreen,
    SommelierScreen,
    AccessoriesScreen,
    TeaScreen,
    CartScreen,
    CheckoutScreen,
    SuccessScreen,
    OrdersScreen,
    ProfileScreen,
    GardenScreen,
    QuestScreen,
    GameScreen,
    ReferralsScreen,
    TreasureHuntScreen,
    ARHuntScreen,
    LocationQuestScreen,
    TechTreeScreen,
    AdminScreen,
};

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
    #[route("/cart")]
    Cart {},
    #[route("/checkout")]
    Checkout {},
    #[route("/success/:id")]
    Success { id: String },
    #[route("/orders")]
    Orders {},
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

// Route handlers using screen components
#[component]
fn Home() -> Element {
    rsx! { HomeScreen {} }
}

#[component]
fn Menu() -> Element {
    rsx! { MenuScreen {} }
}

#[component]
fn Sets() -> Element {
    rsx! { SetsScreen {} }
}

#[component]
fn Sommelier() -> Element {
    rsx! { SommelierScreen {} }
}

#[component]
fn Accessories() -> Element {
    rsx! { AccessoriesScreen {} }
}

#[component]
fn Tea() -> Element {
    rsx! { TeaScreen {} }
}

#[component]
fn Cart() -> Element {
    rsx! { CartScreen {} }
}

#[component]
fn Checkout() -> Element {
    rsx! { CheckoutScreen {} }
}

#[component]
fn Success(id: String) -> Element {
    rsx! { SuccessScreen { id } }
}

#[component]
fn Orders() -> Element {
    rsx! { OrdersScreen {} }
}

#[component]
fn Profile() -> Element {
    rsx! { ProfileScreen {} }
}

#[component]
fn Garden() -> Element {
    rsx! { GardenScreen {} }
}

#[component]
fn Quest(id: String) -> Element {
    rsx! { QuestScreen { id } }
}

#[component]
fn Game() -> Element {
    rsx! { GameScreen {} }
}

#[component]
fn Referrals() -> Element {
    rsx! { ReferralsScreen {} }
}

#[component]
fn TreasureHunt() -> Element {
    rsx! { TreasureHuntScreen {} }
}

#[component]
fn ArHunt() -> Element {
    rsx! { ARHuntScreen {} }
}

#[component]
fn LocationQuest() -> Element {
    rsx! { LocationQuestScreen {} }
}

#[component]
fn TechTree() -> Element {
    rsx! { TechTreeScreen {} }
}

#[component]
fn Admin() -> Element {
    rsx! { AdminScreen {} }
}

#[component]
fn NotFound(route: Vec<String>) -> Element {
    let path = route.iter().map(|s| s.as_str()).collect::<Vec<&str>>().join("/");
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
