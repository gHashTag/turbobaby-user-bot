//! PromptPay QR payload generator for Thailand cashless payments.
//!
//! Implements the Thai QR / PromptPay EMVCo static payload enough for a
//! shop to publish a scannable QR for each order. Falls back to a plain
//! text QR when no merchant account is configured.
//!
//! References:
//! - EMVCo QR Code Specification for Payment Systems (MPM)
//! - Bank of Thailand PromptPay QR guidelines

const PAYLOAD_FORMAT: &str = "000201";
const POINT_OF_INITIATION_STATIC: &str = "010211";
const CURRENCY_THB: &str = "5303764";
const COUNTRY_TH: &str = "5802TH";
const CRC_PLACEHOLDER: &str = "6304";

#[derive(Debug, Clone, Default)]
pub struct QrConfig {
    /// Merchant PromptPay ID. Either:
    /// - a 10-digit Thai mobile number starting with 0 (e.g. 0812345678)
    /// - a 13-digit Thai national ID number.
    pub promptpay_id: Option<String>,
}

impl QrConfig {
    pub fn from_env() -> Self {
        Self {
            promptpay_id: std::env::var("PROMPTPAY_ID").ok().filter(|s| !s.trim().is_empty()),
        }
    }
}

/// Build a PromptPay / Thai QR payload for the given amount and optional
/// reference. Returns a fallback text payload if no merchant ID is
/// configured or the configured ID is invalid.
pub(crate) fn build_payload(cfg: &QrConfig, amount: f64, order_ref: &str) -> String {
    if let Some(id) = cfg.promptpay_id.as_ref() {
        match normalize_promptpay_id(id) {
            Some(normalized) => return emvco_promptpay_payload(&normalized, amount),
            None => {
                tracing::warn!("PROMPTPAY_ID invalid (expected 10-digit phone or 13-digit ID)");
            }
        }
    }
    fallback_payload(amount, order_ref)
}

fn normalize_promptpay_id(raw: &str) -> Option<String> {
    let digits: String = raw.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.len() == 10 && digits.starts_with('0') {
        // Thai mobile in national form: 0812345678 -> 66812345678
        Some(format!("66{}", &digits[1..]))
    } else if digits.len() == 11 && digits.starts_with("66") {
        // Already international mobile form (e.g. +66 81 234 5678).
        Some(digits)
    } else if digits.len() == 13 {
        // National ID treated as-is for PromptPay.
        Some(digits)
    } else {
        None
    }
}

fn emvco_promptpay_payload(normalized_id: &str, amount: f64) -> String {
    let mut payload = String::new();
    payload.push_str(PAYLOAD_FORMAT);
    payload.push_str(POINT_OF_INITIATION_STATIC);
    payload.push_str(&merchant_account_info(normalized_id));
    payload.push_str(CURRENCY_THB);
    payload.push_str(&amount_tlv(amount));
    payload.push_str(COUNTRY_TH);
    payload.push_str(CRC_PLACEHOLDER);

    let crc = crc16_ccitt_false(payload.as_bytes());
    payload.push_str(&format!("{:04X}", crc));
    payload
}

fn merchant_account_info(normalized_id: &str) -> String {
    // PromptPay AID (Thai QR standard).
    let sub_aid = tlv("00", "A000000677010111");
    // Mobile or national ID.
    let sub_id = tlv("01", normalized_id);
    let value = format!("{sub_aid}{sub_id}");
    tlv("29", &value)
}

fn amount_tlv(amount: f64) -> String {
    // PromptPay expects amount with 2 decimal places.
    let value = format!("{:.2}", amount.max(0.0));
    tlv("54", &value)
}

fn fallback_payload(amount: f64, order_ref: &str) -> String {
    format!("Woody Weed | Order: {order_ref} | Total: {:.2} THB", amount)
}

/// EMVCo-style TLV: 2-digit tag + 2-digit zero-padded length + value.
fn tlv(tag: &str, value: &str) -> String {
    format!("{}{:02}{}", tag, value.len(), value)
}

/// CRC-16/CCITT-FALSE as required by Thai QR/PromptPay.
/// Polynomial 0x1021, initial value 0xFFFF, no input/output reflection.
fn crc16_ccitt_false(data: &[u8]) -> u16 {
    let mut crc: u16 = 0xFFFF;
    for &byte in data {
        crc ^= (byte as u16) << 8;
        for _ in 0..8 {
            if (crc & 0x8000) != 0 {
                crc = (crc << 1) ^ 0x1021;
            } else {
                crc <<= 1;
            }
        }
    }
    crc
}

/// Generate an SVG QR code string for `data` using the `qrcode` crate.
pub(crate) fn svg_qr(data: &str) -> Result<String, String> {
    let code = qrcode::QrCode::new(data).map_err(|e| format!("qr encode: {}", e))?;
    let svg = code
        .render::<qrcode::render::svg::Color>()
        .min_dimensions(200, 200)
        .max_dimensions(400, 400)
        .quiet_zone(false)
        .build();
    Ok(svg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_thai_mobile() {
        assert_eq!(normalize_promptpay_id("0812345678"), Some("66812345678".into()));
        assert_eq!(
            normalize_promptpay_id("+66 81 234 5678"),
            Some("66812345678".into())
        );
    }

    #[test]
    fn normalize_national_id() {
        assert_eq!(
            normalize_promptpay_id("1234567890123"),
            Some("1234567890123".into())
        );
    }

    #[test]
    fn reject_invalid_id() {
        assert_eq!(normalize_promptpay_id("12345"), None);
        assert_eq!(normalize_promptpay_id("abc"), None);
        assert_eq!(normalize_promptpay_id("12345678901"), None); // 11 digits but not Thai
        assert_eq!(normalize_promptpay_id("12345678901234"), None); // 14 digits
    }

    #[test]
    fn tlv_format() {
        assert_eq!(tlv("00", "A000000677010111"), "0016A000000677010111");
        assert_eq!(tlv("54", "350.00"), "5406350.00");
    }

    #[test]
    fn emvco_payload_contains_expected_tags() {
        let cfg = QrConfig {
            promptpay_id: Some("0812345678".into()),
        };
        let payload = build_payload(&cfg, 350.0, "abc");
        assert!(payload.starts_with("000201010211"));
        assert!(payload.contains("29"));
        assert!(payload.contains("0016A000000677010111"));
        assert!(payload.contains("5303764"));
        assert!(payload.contains("5406350.00"));
        assert!(payload.contains("5802TH"));
        assert!(payload.contains("6304"));
        assert_eq!(payload.len() % 2, 0);
    }

    #[test]
    fn fallback_when_unconfigured() {
        let cfg = QrConfig::default();
        let payload = build_payload(&cfg, 250.0, "order-123");
        assert!(payload.contains("Woody Weed"));
        assert!(payload.contains("250.00"));
        assert!(payload.contains("order-123"));
    }

    #[test]
    fn crc16_known_vector() {
        // Sanity vector: CRC-16/CCITT-FALSE for "123456789" = 0x29B1.
        let crc = crc16_ccitt_false(b"123456789");
        assert_eq!(crc, 0x29B1);
    }

    #[test]
    fn svg_qr_renders_non_empty() {
        let svg = svg_qr("Woody Weed").unwrap();
        assert!(svg.contains("<svg"));
        assert!(svg.contains("</svg>"));
    }
}
