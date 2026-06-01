//! Core types for Trios ecosystem

use serde::{Deserialize, Serialize};

/// Language support
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Lang {
    Russian,
    English,
    Thai,
    Chinese,
    Hebrew,
    German,
    French,
    Spanish,
}

impl Lang {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Russian => "ru",
            Self::English => "en",
            Self::Thai => "th",
            Self::Chinese => "zh",
            Self::Hebrew => "he",
            Self::German => "de",
            Self::French => "fr",
            Self::Spanish => "es",
        }
    }
}

impl std::str::FromStr for Lang {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "ru" | "russian" => Ok(Self::Russian),
            "en" | "english" => Ok(Self::English),
            "th" | "thai" => Ok(Self::Thai),
            "zh" | "chinese" => Ok(Self::Chinese),
            "he" | "hebrew" => Ok(Self::Hebrew),
            "de" | "german" => Ok(Self::German),
            "fr" | "french" => Ok(Self::French),
            "es" | "spanish" => Ok(Self::Spanish),
            _ => Err(format!("Invalid language code: {}", s)),
        }
    }
}

impl std::fmt::Display for Lang {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Parse `?lang=xx` (or `&lang=xx`) out of a URL query string and resolve
/// it via [`Lang::from_str`]. Returns `None` if the param is missing,
/// empty, or unrecognised — callers can then apply their own default.
///
/// Pure helper, used by the WASM checkout screen (cycle #70 / C) to
/// pick up `Lang` from the Telegram WebApp launch URL without forcing
/// every component to thread a Signal<Lang> through context. The full
/// Lang signal migration is bigger than one cycle's worth.
pub fn detect_lang_from_query(query: &str) -> Option<Lang> {
    // Strip leading "?" if the caller passed window.location.search verbatim.
    let q = query.strip_prefix('?').unwrap_or(query);
    for pair in q.split('&') {
        let mut parts = pair.splitn(2, '=');
        if parts.next() == Some("lang") {
            if let Some(value) = parts.next() {
                if !value.is_empty() {
                    return value.parse::<Lang>().ok();
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod detect_lang_tests {
    use super::*;

    #[test]
    fn detect_lang_picks_up_ru() {
        assert_eq!(detect_lang_from_query("?lang=ru"), Some(Lang::Russian));
    }

    #[test]
    fn detect_lang_picks_up_en_with_other_params() {
        // Real query strings carry several keys — make sure we don't
        // accidentally match `?initData=...&lang=en` as if `lang` were
        // a substring of `initData`.
        assert_eq!(
            detect_lang_from_query("?initData=abc&lang=en&theme=dark"),
            Some(Lang::English),
        );
    }

    #[test]
    fn detect_lang_strips_leading_question_mark_optional() {
        // window.location.search keeps the "?", but a caller composing
        // the query string by hand might omit it. Both must work.
        assert_eq!(detect_lang_from_query("lang=th"), Some(Lang::Thai));
        assert_eq!(detect_lang_from_query("?lang=th"), Some(Lang::Thai));
    }

    #[test]
    fn detect_lang_returns_none_when_param_missing() {
        assert_eq!(detect_lang_from_query("?initData=xyz"), None);
        assert_eq!(detect_lang_from_query(""), None);
    }

    #[test]
    fn detect_lang_returns_none_for_empty_value() {
        // `?lang=` (no value) is a malformed param — bail rather than
        // pick a silent default and confuse the caller.
        assert_eq!(detect_lang_from_query("?lang="), None);
    }

    #[test]
    fn detect_lang_returns_none_for_unknown_code() {
        // Lang::from_str doesn't know "klingon" — propagate the None so
        // the caller falls back to its own default (typically Russian).
        assert_eq!(detect_lang_from_query("?lang=klingon"), None);
    }

    #[test]
    fn detect_lang_doesnt_mismatch_lookalike_keys() {
        // "language=en" must NOT match: the key is exactly "lang".
        // Splitn(2, '=') splits "language" -> ("language", "en"), so the
        // first-token check correctly rejects it.
        assert_eq!(detect_lang_from_query("?language=en"), None);
    }
}

/// Result type alias
pub type Result<T> = std::result::Result<T, Error>;

/// Core error type
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Invalid state: {0}")]
    InvalidState(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Network error: {0}")]
    Network(String),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Database error: {0}")]
    Database(String),

    #[error("Config error: {0}")]
    Config(String),

    #[error("Other error: {0}")]
    Other(String),
}

// Manually implement Clone since std::io::Error and serde_json::Error don't implement it
impl Clone for Error {
    fn clone(&self) -> Self {
        match self {
            Error::InvalidState(s) => Error::InvalidState(s.clone()),
            Error::Validation(s) => Error::Validation(s.clone()),
            Error::NotFound(s) => Error::NotFound(s.clone()),
            Error::Network(s) => Error::Network(s.clone()),
            Error::Serialization(s) => Error::Serialization(s.clone()),
            Error::Io(_) => Error::Io(std::io::Error::other("cloned io error")),
            Error::Json(_) => {
                Error::Json(serde_json::from_str::<serde_json::Value>("").unwrap_err())
            }
            Error::Other(s) => Error::Other(s.clone()),
            Error::Database(s) => Error::Database(s.clone()),
            Error::Config(s) => Error::Config(s.clone()),
        }
    }
}

/// Telegram ID type
pub type TelegramId = i64;

/// QR Token type
pub type QrToken = String;

/// Timestamp type (milliseconds since epoch)
pub type Timestamp = i64;

/// Order ID
#[allow(dead_code)]
pub type OrderId = String;

/// Product ID
#[allow(dead_code)]
pub type ProductId = String;

/// Store ID
#[allow(dead_code)]
pub type StoreId = String;

/// Checkout ID
#[allow(dead_code)]
pub type CheckoutId = String;

/// Transaction ID
#[allow(dead_code)]
pub type TransactionId = String;

/// Currency code
#[allow(dead_code)]
pub type Currency = String;

/// Amount (in smallest currency unit, e.g., satoshis)
#[allow(dead_code)]
pub type Amount = i64;

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_lang_serialization() {
        assert_eq!(Lang::Russian.as_str(), "ru");
        assert_eq!(Lang::English.as_str(), "en");
        assert_eq!(Lang::Thai.as_str(), "th");
        assert_eq!(Lang::Chinese.as_str(), "zh");
        assert_eq!(Lang::Hebrew.as_str(), "he");
        assert_eq!(Lang::German.as_str(), "de");
        assert_eq!(Lang::French.as_str(), "fr");
        assert_eq!(Lang::Spanish.as_str(), "es");
    }

    #[test]
    fn test_lang_from_str() {
        assert_eq!(Lang::from_str("ru").unwrap(), Lang::Russian);
        assert_eq!(Lang::from_str("en").unwrap(), Lang::English);
        assert_eq!(Lang::from_str("th").unwrap(), Lang::Thai);
        assert!(Lang::from_str("invalid").is_err());
    }

    #[test]
    fn test_lang_display() {
        assert_eq!(Lang::Russian.to_string(), "ru");
        assert_eq!(Lang::English.to_string(), "en");
    }
}
