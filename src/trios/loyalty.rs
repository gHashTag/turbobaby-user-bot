//! The shop's standing loyalty policy, declared once (D15).
//!
//! `loyalty_config` is a single JSONB row the shop edits. Every number in it
//! has a standing default for when the row is missing, half-migrated, or
//! carries a key the admin has not filled in — and until 2026-09-16 those
//! defaults were written out by hand in four places that could not see each
//! other:
//!
//! - `api/loyalty.rs` wrote each of eight numbers **twice**: once in the row it
//!   synthesises when the table is empty, and again as the `.unwrap_or(…)`
//!   behind the matching read.
//! - `db/orders.rs::cashback_pct_for_tier` held its own `gold => 10.0, silver
//!   => 7.0, bronze => 5.0`.
//! - `api/orders.rs::load_max_bonus_usage_pct` held `30.0`, itself twice — once
//!   for the missing row and once for the unusable value.
//!
//! Four copies is three that can be edited alone, and the shape had already
//! produced a live defect: the profile screen promised `฿100` per referral
//! against a configured default of `200`, in a translation string no compiler
//! could relate to the number it was about.
//!
//! These are policy, not measurement. Unlike the bike money fields (D9), a
//! missing value here is substituted rather than rendered as a dash, because
//! the callers credit a percentage and have no dash to credit.

use serde_json::{json, Value};

/// The standing policy: what every `loyalty_config` key means when the stored
/// row does not say.
///
/// `referral_bonus` is what the shop pays for a friend who orders. It reaches
/// the profile screen over the wire precisely so that screen never has to guess
/// — see `api/loyalty.rs`.
pub fn defaults() -> Value {
    json!({
        "bronze_threshold": 3000.0,
        "silver_threshold": 10000.0,
        "gold_threshold": 30000.0,
        "bronze_cashback_pct": 5.0,
        "silver_cashback_pct": 7.0,
        "gold_cashback_pct": 10.0,
        "progressive_cashback": 2.0,
        "max_bonus_usage_pct": 30.0,
        "referral_bonus": 200.0
    })
}

/// Read one finite number out of a `loyalty_config` blob, falling back to
/// [`defaults`].
///
/// A non-finite stored value is treated as absent rather than passed through:
/// NaN reaches the screen as `฿NaN` and compares false against every threshold,
/// which silently locks every tier instead of failing loudly.
///
/// A key neither the config nor [`defaults`] knows is `0.0` — the caller asked
/// about a policy this shop does not have.
pub fn config_f64(config: &Value, key: &str) -> f64 {
    let read = |v: &Value| v.get(key).and_then(Value::as_f64).filter(|n| n.is_finite());
    read(config).or_else(|| read(&defaults())).unwrap_or(0.0)
}

/// The standing default for one key, with no stored config in play.
pub fn default_f64(key: &str) -> f64 {
    config_f64(&Value::Null, key)
}

/// The `loyalty_config` key naming the cashback percent for a tier.
///
/// An unrecognised tier falls to `progressive_cashback`, the per-order-count
/// schedule a customer earns on before reaching bronze.
pub fn cashback_key(tier: &str) -> &'static str {
    match tier {
        "gold" => "gold_cashback_pct",
        "silver" => "silver_cashback_pct",
        "bronze" => "bronze_cashback_pct",
        _ => "progressive_cashback",
    }
}

/// The configured ceiling on what share of an order bonus balance may pay.
///
/// A percentage outside `0..=100` is not a configuration, it is a typo, and the
/// only reading that cannot let an admin's stray zero discount an order to
/// nothing — or a stray `1000` open the ceiling past the whole bill — is the
/// standing default. Both call sites used to carry this range rule themselves.
pub fn max_bonus_usage_pct(config: &Value) -> f64 {
    let stored = config_f64(config, "max_bonus_usage_pct");
    if (0.0..=100.0).contains(&stored) {
        stored
    } else {
        default_f64("max_bonus_usage_pct")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stored_number_wins_over_the_standing_default() {
        let config = json!({ "gold_cashback_pct": 12.5 });
        assert_eq!(config_f64(&config, "gold_cashback_pct"), 12.5);
        assert_eq!(config_f64(&config, "silver_cashback_pct"), 7.0);
    }

    #[test]
    fn a_non_finite_stored_number_is_absent_not_passed_through() {
        // `json!` cannot hold NaN, which is the point: it arrives as a string,
        // a null or a missing key, and all three must read as absent.
        for blob in [json!({ "bronze_threshold": null }), json!({}), Value::Null] {
            assert_eq!(config_f64(&blob, "bronze_threshold"), 3000.0);
        }
    }

    #[test]
    fn a_key_no_policy_covers_is_zero() {
        assert_eq!(config_f64(&json!({}), "unicorn_bonus"), 0.0);
    }

    #[test]
    fn every_default_is_a_finite_number() {
        let defaults = defaults();
        let map = defaults.as_object().expect("defaults is an object");
        assert!(
            map.len() >= 9,
            "only {} defaults declared — a scan over an empty policy passes by \
             default",
            map.len()
        );
        for (key, value) in map {
            let n = value
                .as_f64()
                .unwrap_or_else(|| panic!("{key} is not a number: {value}"));
            assert!(n.is_finite(), "{key} is not finite");
            assert!(n >= 0.0, "{key} is negative: {n}");
        }
    }

    #[test]
    fn the_tiers_rise() {
        assert!(default_f64("bronze_threshold") < default_f64("silver_threshold"));
        assert!(default_f64("silver_threshold") < default_f64("gold_threshold"));
        assert!(default_f64("bronze_cashback_pct") < default_f64("silver_cashback_pct"));
        assert!(default_f64("silver_cashback_pct") < default_f64("gold_cashback_pct"));
        assert!(default_f64("progressive_cashback") < default_f64("bronze_cashback_pct"));
    }

    #[test]
    fn every_tier_name_maps_to_a_declared_key() {
        let defaults = defaults();
        for tier in ["gold", "silver", "bronze", "", "platinum", "ЗОЛОТО"] {
            let key = cashback_key(tier);
            assert!(
                defaults.get(key).is_some(),
                "tier {tier:?} maps to {key}, which no default declares"
            );
        }
        assert_eq!(cashback_key("platinum"), "progressive_cashback");
    }

    #[test]
    fn a_bonus_ceiling_outside_the_range_falls_back() {
        assert_eq!(max_bonus_usage_pct(&json!({})), 30.0);
        assert_eq!(
            max_bonus_usage_pct(&json!({"max_bonus_usage_pct": 50.0})),
            50.0
        );
        // A stray zero would discount nothing; a stray 1000 would discount
        // everything. Neither is a configuration.
        assert_eq!(
            max_bonus_usage_pct(&json!({"max_bonus_usage_pct": 0.0})),
            0.0
        );
        assert_eq!(
            max_bonus_usage_pct(&json!({"max_bonus_usage_pct": -1.0})),
            30.0
        );
        assert_eq!(
            max_bonus_usage_pct(&json!({"max_bonus_usage_pct": 1000.0})),
            30.0
        );
    }
}
