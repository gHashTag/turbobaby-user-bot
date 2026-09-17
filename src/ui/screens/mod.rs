// Pure UI Screens — no business logic, no API calls, no state management
// Each screen is self-contained with inline styles matching HTML prototypes

pub mod accessories_screen;
pub mod admin_screen;
pub mod ar_hunt_screen;
// The bike catalog and the single-family detail view. Both files existed on
// disk, fully written, and were declared nowhere — so `menu_screen.rs` imported
// `catalog_screen::CatalogScreen`, which imports `bike_detail::BikeDetail`, and
// the wasm target failed with E0432 on an unresolved module. An undeclared file
// is not merely unused: it is not compiled, not linted and not tested, so every
// other gate in this tree is blind to it. See `orphan_file_tests` in main.rs,
// added with this fix, which now fails on any .rs file its parent never declares.
pub mod bike_detail;
pub mod cart_screen;
pub mod catalog_screen;
pub mod checkout_screen;
pub mod events_screen;
pub mod game_screen;
pub mod home_screen;
pub mod location_quest_screen;
pub mod menu_screen;
pub mod order_detail_screen;
pub mod orders_screen;
pub mod profile_screen;
pub mod quest_screen;
pub mod referrals_screen;
pub mod ride_screen;
// `sommelier_screen` is gone, not retired-in-place. It was a mood/time/effect
// questionnaire that recommended a cannabis strain by fetching `/api/strains`
// — a route 083 retired — and the fleet has no analogue: a bike is chosen by
// engine size, price and licence class, all of which the catalog's filters
// already do. #2.
pub mod success_screen;
pub mod tea_screen;
pub mod tech_tree_screen;
pub mod treasure_hunt_screen;

pub use accessories_screen::AccessoriesScreen;
pub use admin_screen::AdminScreen;
pub use ar_hunt_screen::ARHuntScreen;
pub use bike_detail::BikeDetail;
pub use cart_screen::CartScreen;
pub use catalog_screen::CatalogScreen;
pub use checkout_screen::CheckoutScreen;
pub use events_screen::{EventDetailScreen, EventsScreen, MyBookingsScreen};
pub use game_screen::GameScreen;
pub use home_screen::HomeScreen;
pub use location_quest_screen::LocationQuestScreen;
pub use menu_screen::MenuScreen;
pub use order_detail_screen::OrderDetailScreen;
pub use orders_screen::OrdersScreen;
pub use profile_screen::ProfileScreen;
pub use quest_screen::QuestScreen;
pub use referrals_screen::ReferralsScreen;
pub use ride_screen::RideScreen;
pub use success_screen::SuccessScreen;
pub use tea_screen::TeaScreen;
pub use tech_tree_screen::TechTreeScreen;
pub use treasure_hunt_screen::TreasureHuntScreen;
