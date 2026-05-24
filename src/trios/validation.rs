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
}
