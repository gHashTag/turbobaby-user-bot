//! Loyalty domain — tier calculation logic.
//!
//! Cycle #82 cleanup: this file historically held wire-shape types
//! (`LoyaltyProfile`, `BonusTransaction`, `LoyaltyConfig`) and a
//! `LoyaltyProfile::from_row(&Row)` parser, but none of them were used
//! anywhere in production code (only the file's own tests). With the
//! generation of the SeaORM `loyalty_profile`, `loyalty_config`, and
//! `bonus_transaction` entities, those wire-shapes are now redundant —
//! consumers should read entity `Model` types directly or define their
//! own focused wire-shapes per-endpoint.
//!
//! What stays here: `calculate_tier`, a pure helper for the tier-vs-
//! progressive-cashback decision. It takes only the four scalars it
//! needs (thresholds and the progressive list) so it doesn't drag in
//! an entity dependency for callers that just want the math.

// 9 args is intentional — collapsing into a config struct would force every
// caller (today: tests; tomorrow: cashback flow when wired) to build that
// struct just to call a pure function. Defer the refactor until a real
// caller appears.
#[allow(dead_code, clippy::too_many_arguments)] // Used by cashback flows once they wire it up; pure helper.
pub fn calculate_tier(
    total_spent: f64,
    order_count: i64,
    bronze_threshold: f64,
    silver_threshold: f64,
    gold_threshold: f64,
    bronze_cashback_pct: f64,
    silver_cashback_pct: f64,
    gold_cashback_pct: f64,
    progressive_cashback: &[f64],
) -> (&'static str, f64) {
    let (tier, tier_pct) = if total_spent >= gold_threshold {
        ("gold", gold_cashback_pct)
    } else if total_spent >= silver_threshold {
        ("silver", silver_cashback_pct)
    } else if total_spent >= bronze_threshold {
        ("bronze", bronze_cashback_pct)
    } else {
        ("none", 0.0)
    };
    let idx =
        (order_count.saturating_sub(1) as usize).min(progressive_cashback.len().saturating_sub(1));
    let progressive_pct = if order_count > 0 {
        progressive_cashback.get(idx).copied().unwrap_or(0.0)
    } else {
        0.0
    };
    (tier, tier_pct.max(progressive_pct))
}

#[cfg(test)]
mod tests {
    use super::calculate_tier;

    // Test thresholds mirror the prod `loyalty_config` JSONB defaults
    // (see migrations/001_initial.sql) — keeps unit-test cases close to
    // what the live system actually sees.
    const BRONZE: f64 = 1000.0;
    const SILVER: f64 = 5000.0;
    const GOLD: f64 = 10000.0;
    const BRONZE_PCT: f64 = 5.0;
    const SILVER_PCT: f64 = 10.0;
    const GOLD_PCT: f64 = 15.0;

    fn calc(total: f64, orders: i64, progressive: &[f64]) -> (&'static str, f64) {
        calculate_tier(
            total,
            orders,
            BRONZE,
            SILVER,
            GOLD,
            BRONZE_PCT,
            SILVER_PCT,
            GOLD_PCT,
            progressive,
        )
    }

    #[test]
    fn test_calculate_tier_none() {
        assert_eq!(calc(0.0, 0, &[1.0, 2.0, 3.0]), ("none", 0.0));
    }

    #[test]
    fn test_calculate_tier_bronze() {
        assert_eq!(calc(1000.0, 1, &[1.0, 2.0, 3.0]), ("bronze", 5.0));
    }

    #[test]
    fn test_calculate_tier_silver() {
        assert_eq!(calc(5000.0, 1, &[1.0, 2.0, 3.0]), ("silver", 10.0));
    }

    #[test]
    fn test_calculate_tier_gold() {
        assert_eq!(calc(10000.0, 1, &[1.0, 2.0, 3.0]), ("gold", 15.0));
    }

    #[test]
    fn test_calculate_tier_progressive_overrides() {
        // 3 orders = index 2 = 3.0%, which is less than bronze 5%
        assert_eq!(calc(1000.0, 3, &[1.0, 2.0, 3.0]), ("bronze", 5.0));
        // 2 orders = index 1 = 2.0%, which is less than silver 10%
        assert_eq!(calc(5000.0, 2, &[1.0, 2.0, 3.0]), ("silver", 10.0));
    }

    #[test]
    fn test_calculate_tier_progressive_wins() {
        // 3 orders = index 2 = 30.0%, which beats bronze 5%
        assert_eq!(calc(1000.0, 3, &[20.0, 25.0, 30.0]), ("bronze", 30.0));
    }

    #[test]
    fn test_calculate_tier_order_count_clamped() {
        // 10 orders clamps to last index (1) = 2.0%, less than bronze 5%
        assert_eq!(calc(1000.0, 10, &[1.0, 2.0]), ("bronze", 5.0));
    }

    #[test]
    fn test_calculate_tier_edge_exact_threshold() {
        assert_eq!(calc(999.99, 1, &[1.0, 2.0, 3.0]), ("none", 1.0));
        assert_eq!(calc(1000.0, 1, &[1.0, 2.0, 3.0]), ("bronze", 5.0));
    }
}
