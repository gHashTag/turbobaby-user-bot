// Pure UI Screens — no business logic, no API calls, no state management
// Each screen is self-contained with inline styles matching HTML prototypes

pub mod accessories_screen;
pub mod admin_screen;
pub mod ar_hunt_screen;
pub mod cart_screen;
pub mod checkout_screen;
pub mod events_screen;
pub mod game_screen;
pub mod garden_screen;
pub mod home_screen;
pub mod location_quest_screen;
pub mod menu_screen;
pub mod order_detail_screen;
pub mod orders_screen;
pub mod profile_screen;
pub mod quest_screen;
pub mod referrals_screen;
pub mod sets_screen;
pub mod sommelier_screen;
pub mod success_screen;
pub mod tea_screen;
pub mod tech_tree_screen;
pub mod treasure_hunt_screen;

pub use accessories_screen::AccessoriesScreen;
pub use admin_screen::AdminScreen;
pub use ar_hunt_screen::ARHuntScreen;
pub use cart_screen::CartScreen;
pub use checkout_screen::CheckoutScreen;
pub use events_screen::{EventDetailScreen, EventsScreen, MyBookingsScreen};
pub use game_screen::GameScreen;
pub use garden_screen::GardenScreen;
pub use home_screen::HomeScreen;
pub use location_quest_screen::LocationQuestScreen;
pub use menu_screen::MenuScreen;
pub use order_detail_screen::OrderDetailScreen;
pub use orders_screen::OrdersScreen;
pub use profile_screen::ProfileScreen;
pub use quest_screen::QuestScreen;
pub use referrals_screen::ReferralsScreen;
pub use sets_screen::SetsScreen;
pub use sommelier_screen::SommelierScreen;
pub use success_screen::SuccessScreen;
pub use tea_screen::TeaScreen;
pub use tech_tree_screen::TechTreeScreen;
pub use treasure_hunt_screen::TreasureHuntScreen;
