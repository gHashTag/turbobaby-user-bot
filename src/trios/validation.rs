//! Validation rules for Trios ecosystem

use crate::trios::core::{Error, QrToken, Result, TelegramId};

/// Check if string is 1-20 digits (replaces regex to shrink WASM)
fn is_telegram_id_str(s: &str) -> bool {
    !s.is_empty() && s.len() <= 20 && s.chars().all(|c| c.is_ascii_digit())
}

/// Purchase amount minimum for location quest
pub const MIN_PURCHASE_AMOUNT: i64 = 300;

/// Validate Telegram ID
pub fn validate_telegram_id(id: i64) -> Result<TelegramId> {
    if id <= 0 {
        return Err(Error::Validation(
            "Telegram ID must be a positive integer".to_string(),
        ));
    }
    // Additional check: ID should be reasonable (less than 2^53 for JS safety)
    if id > 9_007_199_254_740_991 {
        return Err(Error::Validation(
            "Telegram ID exceeds maximum safe integer".to_string(),
        ));
    }
    Ok(id)
}

/// Validate Telegram ID from string
pub fn validate_telegram_id_str(s: &str) -> Result<TelegramId> {
    if !is_telegram_id_str(s) {
        return Err(Error::Validation(
            "Telegram ID must be a 1-20 digit number".to_string(),
        ));
    }
    let id: i64 = s
        .parse()
        .map_err(|_| Error::Validation("Telegram ID must be a valid integer".to_string()))?;
    validate_telegram_id(id)
}

/// Validate QR token (16 alphanumeric characters)
pub fn validate_qr_token(s: &str) -> Result<QrToken> {
    if s.len() != 16 {
        return Err(Error::Validation(
            "QR token must be exactly 16 characters".to_string(),
        ));
    }
    if !s.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Err(Error::Validation(
            "QR token must contain only alphanumeric characters".to_string(),
        ));
    }
    Ok(s.to_string())
}

/// Validate purchase amount for location quest
pub fn validate_purchase_amount(amount: i64) -> Result<i64> {
    if amount < MIN_PURCHASE_AMOUNT {
        return Err(Error::Validation(format!(
            "Purchase amount must be at least {} THB",
            MIN_PURCHASE_AMOUNT
        )));
    }
    if amount > 1_000_000 {
        return Err(Error::Validation(
            "Purchase amount exceeds maximum allowed".to_string(),
        ));
    }
    Ok(amount)
}

/// Validate plant water count
pub fn validate_water_count(count: u32) -> Result<u32> {
    if count > 100 {
        return Err(Error::Validation(
            "Water count cannot exceed 100".to_string(),
        ));
    }
    Ok(count)
}

/// Validate location checkpoint order (1-based)
pub fn validate_checkpoint_order(order: u32) -> Result<u32> {
    if order == 0 {
        return Err(Error::Validation(
            "Checkpoint order must be at least 1".to_string(),
        ));
    }
    if order > 100 {
        return Err(Error::Validation(
            "Checkpoint order cannot exceed 100".to_string(),
        ));
    }
    Ok(order)
}

/// Validate location ID (1-based for location quest)
pub fn validate_location_id(id: u32) -> Result<u32> {
    if id == 0 {
        return Err(Error::Validation(
            "Location ID must be at least 1".to_string(),
        ));
    }
    if id > 1000 {
        return Err(Error::Validation("Location ID too large".to_string()));
    }
    Ok(id)
}

/// Cycle #139: integer companion to `parse_finite_float_in_range`.
/// Same shape — empty / malformed / out-of-range -> labelled `Err`,
/// valid `i32` in `[min, max]` -> `Ok(v)`. Used for admin-form stock
/// fields where a typo previously fell to `.unwrap_or(0)` and the
/// downstream check would only catch the `< 0` case, missing
/// "user typed `abc`".
pub fn parse_int_in_range(
    raw: &str,
    label: &str,
    min: i32,
    max: i32,
) -> std::result::Result<i32, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(format!("{}: empty", label));
    }
    let v: i32 = trimmed
        .parse()
        .map_err(|_| format!("{}: not a number ({:?})", label, raw))?;
    if v < min || v > max {
        return Err(format!("{}: out of range {}..={}", label, min, max));
    }
    Ok(v)
}

/// Cycle #137: parse a user-typed float, rejecting empty, malformed,
/// NaN, ±Infinity, and out-of-range values. Used by the admin form
/// for treasure-hunt lat/lon where the pre-#137 `.unwrap_or(0.0)`
/// silently routed every typo to "(0, 0) — Gulf of Guinea".
///
/// `label` shows up in the error so the toast can name the field.
/// `min..=max` is inclusive (lat ±90, lon ±180 for geo; arbitrary
/// for other numeric fields).
pub fn parse_finite_float_in_range(
    raw: &str,
    label: &str,
    min: f64,
    max: f64,
) -> std::result::Result<f64, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(format!("{}: empty", label));
    }
    let v: f64 = trimmed
        .parse()
        .map_err(|_| format!("{}: not a number ({:?})", label, raw))?;
    if !v.is_finite() {
        return Err(format!("{}: must be finite", label));
    }
    if v < min || v > max {
        return Err(format!("{}: out of range {}..={}", label, min, max));
    }
    Ok(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_telegram_id_valid() {
        assert_eq!(validate_telegram_id(123456789).unwrap(), 123456789);
        assert_eq!(validate_telegram_id(1).unwrap(), 1);
    }

    #[test]
    fn test_validate_telegram_id_invalid() {
        assert!(validate_telegram_id(0).is_err());
        assert!(validate_telegram_id(-1).is_err());
        assert!(validate_telegram_id(9_007_199_254_740_992).is_err());
    }

    #[test]
    fn test_validate_telegram_id_str() {
        assert_eq!(validate_telegram_id_str("123456789").unwrap(), 123456789);
        assert!(validate_telegram_id_str("-1").is_err());
        assert!(validate_telegram_id_str("abc").is_err());
        assert!(validate_telegram_id_str("0").is_err());
    }

    #[test]
    fn test_validate_qr_token() {
        let valid = "ABCDEF1234567890";
        assert!(validate_qr_token(valid).is_ok());
        assert!(validate_qr_token("SHORT").is_err());
        assert!(validate_qr_token("ABCD!@#$%^&*()").is_err());
    }

    #[test]
    fn test_validate_purchase_amount() {
        assert_eq!(validate_purchase_amount(300).unwrap(), 300);
        assert_eq!(validate_purchase_amount(500).unwrap(), 500);
        assert!(validate_purchase_amount(299).is_err());
        assert!(validate_purchase_amount(0).is_err());
        assert!(validate_purchase_amount(1_000_001).is_err());
    }

    #[test]
    fn test_validate_water_count() {
        assert_eq!(validate_water_count(0).unwrap(), 0);
        assert_eq!(validate_water_count(50).unwrap(), 50);
        assert_eq!(validate_water_count(100).unwrap(), 100);
        assert!(validate_water_count(101).is_err());
    }

    #[test]
    fn test_validate_checkpoint_order() {
        assert_eq!(validate_checkpoint_order(1).unwrap(), 1);
        assert_eq!(validate_checkpoint_order(5).unwrap(), 5);
        assert!(validate_checkpoint_order(0).is_err());
        assert!(validate_checkpoint_order(101).is_err());
    }

    // ── parse_finite_float_in_range (cycle #137) ─────────────────────

    #[test]
    fn parse_float_accepts_valid_lat() {
        let v = parse_finite_float_in_range("9.7245", "lat", -90.0, 90.0).unwrap();
        assert!((v - 9.7245).abs() < 1e-9);
    }

    #[test]
    fn parse_float_accepts_trimmed_input() {
        let v = parse_finite_float_in_range("  -45.5  ", "lat", -90.0, 90.0).unwrap();
        assert!((v - -45.5).abs() < 1e-9);
    }

    #[test]
    fn parse_float_rejects_empty() {
        let e = parse_finite_float_in_range("", "lat", -90.0, 90.0).unwrap_err();
        assert!(e.contains("empty"));
    }

    #[test]
    fn parse_float_rejects_whitespace_only() {
        assert!(parse_finite_float_in_range("   ", "lat", -90.0, 90.0).is_err());
    }

    #[test]
    fn parse_float_rejects_non_numeric() {
        // The cycle-#137 motivator: pre-fix this would unwrap_or(0.0) and
        // ship "Gulf of Guinea" coordinates.
        let e = parse_finite_float_in_range("abc", "lat", -90.0, 90.0).unwrap_err();
        assert!(e.contains("not a number"));
    }

    #[test]
    fn parse_float_rejects_nan_and_infinity() {
        assert!(parse_finite_float_in_range("NaN", "x", -100.0, 100.0).is_err());
        assert!(parse_finite_float_in_range("inf", "x", -100.0, 100.0).is_err());
        assert!(parse_finite_float_in_range("-inf", "x", -100.0, 100.0).is_err());
    }

    #[test]
    fn parse_float_rejects_out_of_range() {
        // Boundary just outside.
        assert!(parse_finite_float_in_range("91", "lat", -90.0, 90.0).is_err());
        assert!(parse_finite_float_in_range("-181", "lon", -180.0, 180.0).is_err());
        // Inclusive on the boundary.
        assert_eq!(
            parse_finite_float_in_range("90", "lat", -90.0, 90.0).unwrap(),
            90.0
        );
        assert_eq!(
            parse_finite_float_in_range("-180", "lon", -180.0, 180.0).unwrap(),
            -180.0
        );
    }

    #[test]
    fn parse_float_zero_is_valid_in_range() {
        // 0 is a real coordinate (Gulf of Guinea, prime meridian).
        // The fix isn't "reject 0", it's "require explicit typing of 0".
        let v = parse_finite_float_in_range("0", "lat", -90.0, 90.0).unwrap();
        assert_eq!(v, 0.0);
    }

    // ── parse_int_in_range (cycle #139) ──────────────────────────────

    #[test]
    fn parse_int_accepts_valid() {
        assert_eq!(parse_int_in_range("42", "stock", 0, 1000).unwrap(), 42);
        assert_eq!(parse_int_in_range("  -5  ", "x", -10, 10).unwrap(), -5);
    }

    #[test]
    fn parse_int_rejects_empty() {
        assert!(parse_int_in_range("", "stock", 0, 1000)
            .unwrap_err()
            .contains("empty"));
        assert!(parse_int_in_range("   ", "stock", 0, 1000).is_err());
    }

    #[test]
    fn parse_int_rejects_non_numeric() {
        let e = parse_int_in_range("abc", "stock", 0, 1000).unwrap_err();
        assert!(e.contains("not a number"));
    }

    #[test]
    fn parse_int_rejects_float() {
        // 10.5 is not an i32.
        assert!(parse_int_in_range("10.5", "stock", 0, 1000).is_err());
    }

    #[test]
    fn parse_int_rejects_out_of_range() {
        assert!(parse_int_in_range("-1", "stock", 0, 1000).is_err());
        assert!(parse_int_in_range("1001", "stock", 0, 1000).is_err());
        // Inclusive on boundaries.
        assert_eq!(parse_int_in_range("0", "stock", 0, 1000).unwrap(), 0);
        assert_eq!(parse_int_in_range("1000", "stock", 0, 1000).unwrap(), 1000);
    }
}
