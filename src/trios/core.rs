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

/// Best-effort `Lang` lookup for a Telegram-supplied or URL-supplied locale
/// string (cycle #72).
///
/// Steps, in order:
///   1. Lowercase.
///   2. Strip any region suffix after `-` or `_` (BCP 47 / Telegram dialect
///      forms like `en-US`, `pt_BR` → bare primary subtag).
///   3. Direct [`Lang::from_str`] — covers every code we natively support.
///   4. **Neighbor map** — for primary subtags we don't translate yet,
///      pick the closest known locale instead of falling back to the
///      project default. The mappings reflect both linguistic distance
///      and the practical Telegram audience:
///        * `uk` (Ukrainian), `be` (Belarusian), `kk` (Kazakh) → `ru`
///        * `pt` (Portuguese), `it` (Italian), `nl` (Dutch),
///          `ko` (Korean), `ja` (Japanese), `ar` (Arabic), `vi`
///          (Vietnamese), `pl` (Polish) → `en`
///   5. Otherwise `None` — caller picks its own default (typically RU).
///
/// Pure helper. The neighbor table is intentionally conservative — adding
/// a mapping is a one-line change. The goal is to make `pick_lang` do
/// something *useful* for the long tail of Telegram locales without
/// claiming to render in every language.
pub fn normalize_lang_code(code: &str) -> Option<Lang> {
    let lower = code.trim().to_lowercase();
    if lower.is_empty() {
        return None;
    }
    // BCP 47 / dialect: pick the primary subtag before `-` or `_`.
    let primary = lower.split(['-', '_']).next().unwrap_or(lower.as_str());

    if let Ok(lang) = primary.parse::<Lang>() {
        return Some(lang);
    }

    // Neighbor map for primary subtags we don't translate natively yet.
    match primary {
        "uk" | "be" | "kk" => Some(Lang::Russian),
        "pt" | "it" | "nl" | "ko" | "ja" | "ar" | "vi" | "pl" => Some(Lang::English),
        _ => None,
    }
}

/// Pick the rendering `Lang` for a screen by composing two source signals:
/// an explicit `?lang=xx` URL override and the Telegram WebApp's user
/// `language_code`. Pure — both args are owned `Option<&str>`.
///
/// **Precedence (highest first):**
///   1. `query` — explicit user choice via URL, e.g. opening
///      `https://app/checkout?lang=en` from Telegram.
///   2. `tg_lang_code` — `initDataUnsafe.user.language_code`, the
///      passive Telegram-client locale.
///   3. `Lang::Russian` — production default (we ship Russian copy
///      everywhere; English is staged but no other locale yet).
///
/// Cycle #71: cycle #70 only used the URL source, which works on
/// desktop but not on mobile where URL juggling is unfriendly. The
/// Telegram fallback closes that gap without forcing the caller to
/// memorise the resolution order.
pub fn pick_lang(query: Option<&str>, tg_lang_code: Option<&str>) -> Lang {
    if let Some(q) = query {
        if let Some(lang) = detect_lang_from_query(q) {
            return lang;
        }
    }
    if let Some(code) = tg_lang_code {
        // Telegram sends bare ISO 639-1 codes (`ru`, `en`, `th`) plus the
        // occasional dialect (`pt-BR`, `en-US`). `normalize_lang_code`
        // handles both, falls back to a nearest-neighbor map for codes we
        // don't render natively yet (uk→ru, pt→en, …).
        if let Some(lang) = normalize_lang_code(code) {
            return lang;
        }
    }
    Lang::Russian
}

#[cfg(test)]
mod normalize_lang_code_tests {
    use super::*;

    // Direct native matches must keep working.

    #[test]
    fn direct_supported_codes_round_trip() {
        for (code, expected) in [
            ("ru", Lang::Russian),
            ("en", Lang::English),
            ("th", Lang::Thai),
            ("zh", Lang::Chinese),
            ("he", Lang::Hebrew),
            ("de", Lang::German),
            ("fr", Lang::French),
            ("es", Lang::Spanish),
        ] {
            assert_eq!(
                normalize_lang_code(code),
                Some(expected),
                "direct {} → {:?} round-trip failed",
                code,
                expected,
            );
        }
    }

    #[test]
    fn case_insensitive_primary_match() {
        // Telegram tends to ship lowercase but BCP 47 says "case-insensitive".
        assert_eq!(normalize_lang_code("EN"), Some(Lang::English));
        assert_eq!(normalize_lang_code("Ru"), Some(Lang::Russian));
    }

    #[test]
    fn strips_region_suffix_after_dash() {
        // BCP 47 form. en-US / pt-BR are the realistic Telegram dialects.
        assert_eq!(normalize_lang_code("en-US"), Some(Lang::English));
        assert_eq!(normalize_lang_code("en-GB"), Some(Lang::English));
    }

    #[test]
    fn strips_region_suffix_after_underscore() {
        // POSIX-style locale form (some user agents normalise differently).
        assert_eq!(normalize_lang_code("ru_RU"), Some(Lang::Russian));
        assert_eq!(normalize_lang_code("th_TH"), Some(Lang::Thai));
    }

    // Neighbor mappings — sociolinguistic falls.

    #[test]
    fn slavic_neighbors_map_to_russian() {
        // Ukrainian, Belarusian, Kazakh — Telegram audience overlap with RU
        // is significant; serving them Cyrillic copy is closer than EN.
        assert_eq!(normalize_lang_code("uk"), Some(Lang::Russian));
        assert_eq!(normalize_lang_code("be"), Some(Lang::Russian));
        assert_eq!(normalize_lang_code("kk"), Some(Lang::Russian));
    }

    #[test]
    fn romance_and_other_long_tail_codes_map_to_english() {
        // Portuguese, Italian, Dutch, Korean, Japanese, Arabic, Vietnamese,
        // Polish — no native copy yet. English is the safest universal.
        for code in ["pt", "it", "nl", "ko", "ja", "ar", "vi", "pl"] {
            assert_eq!(
                normalize_lang_code(code),
                Some(Lang::English),
                "{} should fall back to English",
                code,
            );
        }
    }

    #[test]
    fn neighbor_lookup_also_strips_region() {
        // pt-BR primary is "pt" → English; un-mapped dialect must still
        // resolve through the same neighbor path.
        assert_eq!(normalize_lang_code("pt-BR"), Some(Lang::English));
        assert_eq!(normalize_lang_code("uk-UA"), Some(Lang::Russian));
    }

    // Unknown / malformed inputs.

    #[test]
    fn truly_unknown_code_returns_none() {
        // `klingon` isn't in any branch — caller falls back to its own default.
        assert_eq!(normalize_lang_code("klingon"), None);
        assert_eq!(normalize_lang_code("zz"), None);
    }

    #[test]
    fn empty_or_whitespace_returns_none() {
        assert_eq!(normalize_lang_code(""), None);
        assert_eq!(normalize_lang_code("   "), None);
    }

    #[test]
    fn unknown_primary_with_known_region_still_returns_none() {
        // `xx-US` — primary is unknown and we don't peek at the region.
        // Returning None forces caller's explicit default, which is safer
        // than silently picking English because of an unrelated region tag.
        assert_eq!(normalize_lang_code("xx-US"), None);
    }
}

#[cfg(test)]
mod pick_lang_tests {
    use super::*;

    #[test]
    fn query_wins_over_telegram_when_both_present() {
        // User explicitly opened ?lang=en — even if Telegram says ru,
        // honour the URL override (= user just made a deliberate choice).
        assert_eq!(pick_lang(Some("?lang=en"), Some("ru")), Lang::English,);
    }

    #[test]
    fn falls_back_to_telegram_when_query_missing() {
        // Mobile case: no URL param, Telegram client locale "en".
        assert_eq!(pick_lang(None, Some("en")), Lang::English,);
    }

    #[test]
    fn falls_back_to_telegram_when_query_present_but_no_lang_param() {
        // Real query has initData + theme but no lang= key.
        assert_eq!(
            pick_lang(Some("?initData=abc&theme=dark"), Some("th")),
            Lang::Thai,
        );
    }

    #[test]
    fn defaults_to_russian_when_both_sources_empty() {
        // Cold launch with no init data — RU is what we ship.
        assert_eq!(pick_lang(None, None), Lang::Russian);
    }

    #[test]
    fn skips_unknown_telegram_code_and_returns_default() {
        // Telegram could send any locale code; we only know a subset.
        // Don't pick a silent wrong locale — default to RU.
        assert_eq!(pick_lang(None, Some("klingon")), Lang::Russian,);
    }

    #[test]
    fn telegram_neighbor_code_flows_through_pick_lang() {
        // Cycle #72 wired normalize_lang_code into pick_lang. A Ukrainian
        // Telegram user without a `?lang=` override should now see Russian
        // copy (closest neighbor) instead of the project default — same
        // outcome on the surface, but the *reason* matters for tracing
        // future regressions. Explicit `pt-BR → English` is the other end
        // of the spectrum and worth pinning separately.
        assert_eq!(pick_lang(None, Some("uk")), Lang::Russian);
        assert_eq!(pick_lang(None, Some("pt-BR")), Lang::English);
    }

    #[test]
    fn skips_unknown_query_and_telegram_code_together() {
        // Both sources present but neither parses → default to RU. This
        // is the path that catches the "?lang=xx" typo case where the
        // URL is otherwise well-formed.
        assert_eq!(
            pick_lang(Some("?lang=elvish"), Some("dwarvish")),
            Lang::Russian,
        );
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
            Error::Json(_) => Error::Json(
                serde_json::from_str::<serde_json::Value>("")
                    .expect_err("empty string is always invalid JSON"),
            ),
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
