//! Marketing-flag price math, shared between WASM customer UI and any future
//! backend price-authority check.
//!
//! **Why this lives in `trios/`:** the module compiles for both `backend`
//! (tokio_postgres handlers in `api/`) and `wasm32` (Dioxus `ui/screens/`).
//! Before this module existed (cycle #54), the precedence rules were inlined
//! into `menu_screen.rs::render_strain_card`. Re-deriving them on the server
//! for an order-tampering check would have introduced two copies of the same
//! decision tree — easy to drift, hard to test. Cycle #55 extracted them
//! once with table-driven unit tests so divergence is impossible.
//!
//! ## Precedence (highest to lowest)
//!
//! 1. **Strain of the Day** with a positive `strain_of_day_discount`.
//! 2. **Active sale window** (`sale_active` AND `sale_until` either NULL or
//!    in the future) — within this branch:
//!    a) `sale_price` if set, finite, positive, and lower than base.
//!    b) `discount_percent` if set and positive.
//! 3. **Base** `price_per_gram`.
//!
//! Non-finite inputs are sanitized to 0 to prevent NaN propagation into UI
//! totals or DB writes.

use chrono::{DateTime, Utc};

/// Inputs needed to compute the effective per-gram price.
///
/// Designed as a plain-data struct so callers can construct it from either
/// `db::strains::Strain` (backend) or `ApiStrain` (wasm) without forcing
/// either crate to depend on the other.
#[derive(Debug, Clone)]
pub struct MarketingFlags<'a> {
    pub price_per_gram: f64,
    pub is_strain_of_day: bool,
    pub strain_of_day_discount: f64,
    pub sale_active: bool,
    /// RFC3339 timestamp string; `None` or empty means "no expiry".
    pub sale_until: Option<&'a str>,
    pub sale_price: Option<f64>,
    pub discount_percent: f64,
    pub is_new_arrival: bool,
    pub new_until: Option<&'a str>,
}

/// Is the given RFC3339 expiry timestamp still in the future?
///
/// Returns `true` when:
///   * `until` is `None`, or
///   * `until` is empty, or
///   * `until` parses as RFC3339 and is strictly greater than `now`, or
///   * `until` fails to parse (permissive — better to show a stale badge
///     than to misrepresent the catalog because of a malformed timestamp).
pub fn is_active_until(until: Option<&str>, now: DateTime<Utc>) -> bool {
    let Some(s) = until else { return true };
    if s.is_empty() {
        return true;
    }
    match DateTime::parse_from_rfc3339(s) {
        Ok(t) => t > now,
        Err(_) => true,
    }
}

/// Output of [`effective_strain_price`].
///
/// `price` is the per-gram price the customer actually pays. `has_discount`
/// is a UI hint — when `true`, the customer card renders the strikethrough
/// original price next to the new one.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PricedStrain {
    pub price: f64,
    pub has_discount: bool,
}

/// Apply the precedence rules in the module docs and return the price the
/// customer should pay.
pub fn effective_strain_price(flags: &MarketingFlags<'_>, now: DateTime<Utc>) -> PricedStrain {
    let base = sanitize_money(flags.price_per_gram);
    let sotd_discount = sanitize_pct(flags.strain_of_day_discount);

    // 1. SOTD wins outright when there's a positive discount.
    if flags.is_strain_of_day && sotd_discount > 0.0 {
        return PricedStrain {
            price: (base * (1.0 - sotd_discount / 100.0)).max(0.0),
            has_discount: true,
        };
    }

    // 2. Active sale window — sale_price overrides, then discount_percent.
    let sale_live = flags.sale_active && is_active_until(flags.sale_until, now);
    if sale_live {
        // 2a) explicit sale_price beats base only when it's actually lower.
        if let Some(sp) = flags
            .sale_price
            .filter(|v| v.is_finite() && *v > 0.0 && *v < base)
        {
            return PricedStrain {
                price: sp.max(0.0),
                has_discount: true,
            };
        }
        // 2b) percentage discount.
        let pct = sanitize_pct(flags.discount_percent);
        if pct > 0.0 {
            return PricedStrain {
                price: (base * (1.0 - pct / 100.0)).max(0.0),
                has_discount: true,
            };
        }
    }

    // 3. Base.
    PricedStrain {
        price: base,
        has_discount: false,
    }
}

fn sanitize_money(v: f64) -> f64 {
    if v.is_finite() {
        v.max(0.0)
    } else {
        0.0
    }
}

fn sanitize_pct(v: f64) -> f64 {
    if v.is_finite() {
        v.clamp(0.0, 100.0)
    } else {
        0.0
    }
}

// ── Other catalogs (cycle #58 / C). Accessories and tea products have a
//    flat `price` column (no discount machinery). Sets — `sets`,
//    `accessory_sets`, `tea_sets` — share `(total_price, discount_percent)`.
//    All three set tables use UUID primary keys with no overlap, so the
//    server-side lookup can UNION them into a single map.

/// Sanitised per-unit price for an accessory row. Just clamps non-finite /
/// negative values to 0 — accessories have no sale flags.
pub fn effective_accessory_price(price: f64) -> f64 {
    sanitize_money(price)
}

/// Tea products use the same flat-price model as accessories.
pub fn effective_tea_price(price: f64) -> f64 {
    sanitize_money(price)
}

/// Apply a percentage discount to a set's pre-set `total_price`. Shared by
/// the three set tables (`sets`, `accessory_sets`, `tea_sets`) — schema is
/// identical and customer-facing pricing is, too.
pub fn effective_set_price(total_price: f64, discount_percent: f64) -> f64 {
    let base = sanitize_money(total_price);
    let pct = sanitize_pct(discount_percent);
    (base * (1.0 - pct / 100.0)).max(0.0)
}

/// Format a money amount (THB) for customer display: clamp NaN/inf/negative to
/// 0, drop the fractional part (whole-baht display), prefix `฿`. Casts to `i64`
/// (not `i32`) so a large-but-valid total can't saturate at ~2.1B — prices and
/// totals are `i64` in the domain (`ProductPrice.price`, `calculate_cart_total`).
/// Single source of truth: the customer UI previously had three identical
/// `format_price` clones (menu/cart/home) that each narrowed to `i32`.
pub fn format_baht(amount: f64) -> String {
    format!("฿{}", sanitize_money(amount) as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn now_utc() -> DateTime<Utc> {
        Utc::now()
    }

    #[test]
    fn test_format_baht_basic_and_clamps() {
        assert_eq!(format_baht(350.0), "฿350");
        assert_eq!(format_baht(350.99), "฿350"); // whole-baht (truncates)
        assert_eq!(format_baht(0.0), "฿0");
        // NaN / inf / negative clamp to 0.
        assert_eq!(format_baht(f64::NAN), "฿0");
        assert_eq!(format_baht(f64::INFINITY), "฿0");
        assert_eq!(format_baht(-5.0), "฿0");
    }

    #[test]
    fn test_format_baht_large_value_does_not_saturate_like_i32() {
        // A total above i32::MAX (~2.1B) must render its real value, not the
        // saturated 2147483647 the old `as i32` clones produced.
        let big = 3_000_000_000.0_f64; // > i32::MAX
        assert_eq!(format_baht(big), "฿3000000000");
    }

    fn base_flags<'a>() -> MarketingFlags<'a> {
        MarketingFlags {
            price_per_gram: 350.0,
            is_strain_of_day: false,
            strain_of_day_discount: 0.0,
            sale_active: false,
            sale_until: None,
            sale_price: None,
            discount_percent: 0.0,
            is_new_arrival: false,
            new_until: None,
        }
    }

    // ── is_active_until ────────────────────────────────────────────

    #[test]
    fn active_until_none_is_open_ended() {
        assert!(is_active_until(None, now_utc()));
    }

    #[test]
    fn active_until_empty_string_is_open_ended() {
        assert!(is_active_until(Some(""), now_utc()));
    }

    #[test]
    fn active_until_future_passes() {
        // 1h in the future is plenty regardless of clock skew at test time.
        let in_future = (Utc::now() + chrono::Duration::hours(1)).to_rfc3339();
        assert!(is_active_until(Some(&in_future), Utc::now()));
    }

    #[test]
    fn active_until_past_fails() {
        let in_past = (Utc::now() - chrono::Duration::hours(1)).to_rfc3339();
        assert!(!is_active_until(Some(&in_past), Utc::now()));
    }

    #[test]
    fn active_until_malformed_is_permissive() {
        // Better to keep the badge visible than to misrepresent the catalog
        // due to a typo in an admin timestamp.
        assert!(is_active_until(Some("not-a-date"), now_utc()));
    }

    // ── effective_strain_price ─────────────────────────────────────

    #[test]
    fn no_flags_returns_base() {
        let priced = effective_strain_price(&base_flags(), now_utc());
        assert!((priced.price - 350.0).abs() < 1e-9);
        assert!(!priced.has_discount);
    }

    #[test]
    fn sotd_with_discount_wins_over_sale() {
        let mut f = base_flags();
        f.is_strain_of_day = true;
        f.strain_of_day_discount = 30.0;
        // Sale below should be ignored.
        f.sale_active = true;
        f.sale_price = Some(50.0);
        let priced = effective_strain_price(&f, now_utc());
        // 350 * 0.7 = 245
        assert!((priced.price - 245.0).abs() < 1e-9);
        assert!(priced.has_discount);
    }

    #[test]
    fn sotd_without_discount_falls_through() {
        // Marking SOTD without setting a discount must NOT lower the price —
        // SOTD is a visibility flag in that case, not a discount.
        let mut f = base_flags();
        f.is_strain_of_day = true;
        f.strain_of_day_discount = 0.0;
        let priced = effective_strain_price(&f, now_utc());
        assert!((priced.price - 350.0).abs() < 1e-9);
        assert!(!priced.has_discount);
    }

    #[test]
    fn sale_price_overrides_discount_percent() {
        let mut f = base_flags();
        f.sale_active = true;
        f.sale_price = Some(199.0);
        f.discount_percent = 50.0; // would give 175 — but sale_price wins
        let priced = effective_strain_price(&f, now_utc());
        assert!((priced.price - 199.0).abs() < 1e-9);
        assert!(priced.has_discount);
    }

    #[test]
    fn sale_price_ignored_when_not_strictly_lower() {
        // Admin typo: sale_price = base price. UI must not display a fake
        // "discount" that doesn't actually lower the price.
        let mut f = base_flags();
        f.sale_active = true;
        f.sale_price = Some(350.0);
        f.discount_percent = 0.0;
        let priced = effective_strain_price(&f, now_utc());
        assert!((priced.price - 350.0).abs() < 1e-9);
        assert!(!priced.has_discount);
    }

    #[test]
    fn sale_uses_discount_percent_when_no_sale_price() {
        let mut f = base_flags();
        f.sale_active = true;
        f.discount_percent = 25.0;
        let priced = effective_strain_price(&f, now_utc());
        // 350 * 0.75 = 262.5
        assert!((priced.price - 262.5).abs() < 1e-9);
        assert!(priced.has_discount);
    }

    #[test]
    fn expired_sale_falls_through_to_base() {
        let in_past = (Utc::now() - chrono::Duration::hours(1)).to_rfc3339();
        let mut f = base_flags();
        f.sale_active = true;
        f.sale_until = Some(&in_past);
        f.sale_price = Some(50.0);
        let priced = effective_strain_price(&f, now_utc());
        assert!((priced.price - 350.0).abs() < 1e-9);
        assert!(!priced.has_discount);
    }

    #[test]
    fn inactive_sale_flag_ignores_sale_price() {
        let mut f = base_flags();
        f.sale_active = false; // explicitly off
        f.sale_price = Some(50.0);
        f.discount_percent = 50.0;
        let priced = effective_strain_price(&f, now_utc());
        assert!((priced.price - 350.0).abs() < 1e-9);
        assert!(!priced.has_discount);
    }

    #[test]
    fn nan_base_price_sanitized_to_zero() {
        // Defensive: a row with corrupted price_per_gram must not produce NaN
        // downstream (NaN propagates through DB UPDATEs and breaks aggregates).
        let mut f = base_flags();
        f.price_per_gram = f64::NAN;
        let priced = effective_strain_price(&f, now_utc());
        assert_eq!(priced.price, 0.0);
    }

    #[test]
    fn discount_percent_over_100_is_clamped_not_negative() {
        // Admin typo: enters 150%. Effective price clamps to 0, never negative.
        let mut f = base_flags();
        f.sale_active = true;
        f.discount_percent = 150.0;
        let priced = effective_strain_price(&f, now_utc());
        assert_eq!(priced.price, 0.0);
    }

    // ── Other catalogs ────────────────────────────────────────────

    #[test]
    fn accessory_price_sanitises_negatives_and_nan() {
        assert!((effective_accessory_price(150.0) - 150.0).abs() < 1e-9);
        assert_eq!(effective_accessory_price(-5.0), 0.0);
        assert_eq!(effective_accessory_price(f64::NAN), 0.0);
    }

    #[test]
    fn tea_price_uses_same_sanitisation_as_accessory() {
        // The two helpers are intentionally separate symbols (admin reading
        // call sites benefits from explicit catalog naming) but must behave
        // identically — pin that here.
        for &v in &[0.0, 50.0, -1.0, f64::INFINITY] {
            assert_eq!(effective_accessory_price(v), effective_tea_price(v));
        }
    }

    #[test]
    fn set_price_applies_percentage_discount() {
        // 500 baht with 20% off = 400.
        assert!((effective_set_price(500.0, 20.0) - 400.0).abs() < 1e-9);
    }

    #[test]
    fn set_price_clamps_oversized_discount_to_zero_floor() {
        // Admin typo: enters 200%. Price clamps to 0, never goes negative.
        assert_eq!(effective_set_price(500.0, 200.0), 0.0);
    }

    #[test]
    fn set_price_handles_zero_discount() {
        assert!((effective_set_price(500.0, 0.0) - 500.0).abs() < 1e-9);
    }
}
