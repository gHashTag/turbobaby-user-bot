/// Custom business metrics for Woody Weed Bot.
///
/// Uses the `metrics` crate which is already wired under the hood by
/// `axum-prometheus`.  All counters declared here are automatically
/// included in the `/metrics` Prometheus scrape endpoint.
use metrics::counter;

/// Increment when a new order is successfully persisted in `create_order`.
pub fn order_created() {
    counter!("orders_created_total").increment(1);
}

/// Increment when a quest entity is created.
///
/// `kind` should be `"place"` (quest_places) or `"treasure"` (treasure_hunts).
pub fn quest_created(kind: &str) {
    counter!("quests_created_total", "kind" => kind.to_string()).increment(1);
}

/// Increment when a new user registers (first `/start` with no prior lang record).
pub fn user_registered() {
    counter!("users_registered_total").increment(1);
}

/// Increment when a garden reward is successfully claimed.
pub fn garden_reward_claimed() {
    counter!("garden_rewards_claimed_total").increment(1);
}

/// Increment when a QR code is scanned in the location quest.
///
/// `is_final` indicates whether this was the last location in the quest.
pub fn qr_scanned(is_final: bool) {
    counter!("qr_scans_total", "final" => is_final.to_string()).increment(1);
}
