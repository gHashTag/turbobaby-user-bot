use anyhow::{anyhow, Result};

#[derive(Debug, Clone)]
pub struct Config {
    pub bot_token: String,
    pub web_app_url: String,
    pub port: u16,
    pub webhook_path: String,
    pub app_url: String,
    pub is_production: bool,
    pub admin_ids: Vec<i64>,
    pub bot_username: String,
    pub database_url: String,
    pub grok_api_key: String,
    pub glm_api_key: String,
    pub s3_bucket: Option<String>,
    pub s3_endpoint: Option<String>,
    /// Optional Railway-private endpoint used for the S3 client itself.
    /// When set, uploads go over the internal network (e.g. http://bucket.railway.internal:9000)
    /// while `s3_public_url` keeps producing browser-facing URLs over the public edge.
    /// Falls back to `s3_endpoint` when unset.
    pub s3_internal_endpoint: Option<String>,
    pub s3_public_url: Option<String>,
    pub s3_region: Option<String>,
    pub s3_access_key: Option<String>,
    pub s3_secret_key: Option<String>,
    pub backup_assets: bool,
    pub admin_password: Option<String>,
    /// Cycle #133-B: TZ #2 section 5 — admin-controlled "hide all
    /// marketing badges" toggle. When `true`, customer-facing
    /// `/api/strains` calls strip Sale / Best Seller / New Arrival
    /// flags off each row before serialising. SOTD stays visible —
    /// it's the headline feature, not a promo. Admin endpoints
    /// (`include_hidden=1`) pass through unchanged so editing
    /// flag state remains possible. Env-driven for now
    /// (`HIDE_MARKETING_BADGES=1`); promote to a DB-backed admin
    /// toggle when that's needed.
    pub hide_marketing_badges: bool,
    /// Delivery zones and ETA estimates. Loaded from `DELIVERY_ZONES_JSON`
    /// or built-in Koh Phangan defaults. Used by the order tracker and
    /// checkout to show ETA / fee ranges.
    pub delivery_zones: crate::delivery::DeliveryZones,
    /// PromptPay merchant identifier for cashless Thai QR payments.
    /// Thai mobile numbers (10 digits starting with 0) or 13-digit national
    /// IDs are accepted. Optional — when absent the QR endpoint falls back
    /// to a plain text payment prompt.
    pub promptpay: crate::promptpay::QrConfig,
    /// LINE channel access token for retention broadcasts. Optional —
    /// when absent the admin LINE broadcast endpoint returns 503 and the
    /// capability is listed in disabled_capabilities.
    pub line_channel_access_token: Option<String>,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        dotenvy::dotenv().ok();

        // Cycle #122: collect *all* missing required env vars before
        // returning, instead of failing one-at-a-time. A first-time
        // deploy with neither BOT_TOKEN nor DATABASE_URL set used to
        // require fix → restart → fix → restart; now one error lists
        // everything.
        let required = collect_required_env(&["BOT_TOKEN", "DATABASE_URL"], |name| {
            std::env::var(name).ok()
        })
        .map_err(|missing| {
            anyhow!(
                "missing required env var(s): {} — set all of them and retry",
                missing.join(", ")
            )
        })?;
        let bot_token = required[0].clone();
        let database_url = required[1].clone();

        // Admin IDs from env; if ADMIN_IDS is set it is the only source of truth.
        // Production refuses to start without ADMIN_IDS to avoid default-admin backdoors.
        let is_production = std::env::var("NODE_ENV").unwrap_or_default() == "production"
            || std::env::var("RAILWAY_ENVIRONMENT").is_ok();

        let admin_ids: Vec<i64> = std::env::var("ADMIN_IDS")
            .unwrap_or_default()
            .split(',')
            .filter_map(|s| s.trim().parse::<i64>().ok())
            .collect();
        if admin_ids.is_empty() {
            if is_production {
                return Err(anyhow!(
                    "ADMIN_IDS is required in production — set ADMIN_IDS to a comma-separated \
                     list of Telegram user IDs"
                ));
            } else {
                tracing::warn!("ADMIN_IDS not set — admin endpoints will reject all requests");
            }
        }

        // The backend service serves BOTH the API and the WASM frontend on the same origin.
        // If WEB_APP_URL is empty or points to the broken legacy TMA service, fall back to the
        // canonical backend URL so the Telegram WebApp button always opens a working page.
        let raw_web_app_url = std::env::var("WEB_APP_URL").unwrap_or_default();
        let canonical_web_app_url =
            "https://woody-weed-bot-production-370f.up.railway.app/?cache=180".to_string();
        let web_app_url = if raw_web_app_url.trim().is_empty()
            || raw_web_app_url.contains("woody-woodpecker-tma-production")
        {
            canonical_web_app_url
        } else {
            raw_web_app_url
        };

        Ok(Self {
            bot_token,
            web_app_url,
            port: parse_port_env(std::env::var("PORT").ok())?,
            webhook_path: std::env::var("WEBHOOK_PATH").unwrap_or("/webhook".into()),
            app_url: std::env::var("APP_URL").unwrap_or_default(),
            is_production,
            admin_ids,
            bot_username: {
                let raw = std::env::var("BOT_USERNAME").unwrap_or("Woody_WeedPecker_bot".into());
                let trimmed = raw.trim();
                if trimmed.is_empty() {
                    "Woody_WeedPecker_bot".into()
                } else {
                    trimmed.to_string()
                }
            },
            database_url,
            grok_api_key: std::env::var("GROK_API_KEY").unwrap_or_default(),
            glm_api_key: std::env::var("GLM_API_KEY").unwrap_or_default(),
            s3_bucket: std::env::var("S3_BUCKET").ok(),
            s3_endpoint: std::env::var("S3_ENDPOINT").ok(),
            s3_internal_endpoint: std::env::var("S3_INTERNAL_ENDPOINT")
                .ok()
                .filter(|s| !s.trim().is_empty()),
            s3_public_url: std::env::var("S3_PUBLIC_URL").ok(),
            s3_region: std::env::var("S3_REGION").ok(),
            s3_access_key: std::env::var("S3_ACCESS_KEY_ID").ok(),
            s3_secret_key: std::env::var("S3_SECRET_ACCESS_KEY").ok(),
            backup_assets: std::env::var("BACKUP_ASSETS").unwrap_or("true".into()) != "false",
            admin_password: std::env::var("ADMIN_PASSWORD").ok(),
            hide_marketing_badges: parse_bool_env(
                std::env::var("HIDE_MARKETING_BADGES").ok().as_deref(),
            ),
            delivery_zones: crate::delivery::DeliveryZones::from_env_or_default(),
            promptpay: crate::promptpay::QrConfig::from_env(),
            line_channel_access_token: std::env::var("LINE_CHANNEL_ACCESS_TOKEN")
                .ok()
                .filter(|s| !s.trim().is_empty()),
        })
    }

    pub fn s3_enabled(&self) -> bool {
        self.s3_bucket.is_some() && self.s3_endpoint.is_some()
    }

    /// True when at least one AI provider key is configured — i.e. the
    /// sommelier / Grok features can actually call out. Both empty ⇒ disabled.
    pub fn ai_enabled(&self) -> bool {
        !self.grok_api_key.trim().is_empty() || !self.glm_api_key.trim().is_empty()
    }

    pub fn line_enabled(&self) -> bool {
        self.line_channel_access_token
            .as_deref()
            .map(|s| !s.trim().is_empty())
            .unwrap_or(false)
    }

    /// Optional capabilities that are OFF because their config is absent.
    /// Logged at startup so ops sees "AI disabled (no key)" up front instead
    /// of discovering it from a failed user request (the AI client only warns
    /// per-call). These are NOT faults — an environment may intentionally run
    /// without AI or S3 — so the caller logs a warning, not an alert.
    pub fn disabled_capabilities(&self) -> Vec<&'static str> {
        disabled_capabilities_from(self.ai_enabled(), self.s3_enabled(), self.line_enabled())
    }
}

/// Pure core of [`Config::disabled_capabilities`] — split out so the mapping
/// is unit-testable without constructing a full `Config`.
fn disabled_capabilities_from(
    ai_enabled: bool,
    s3_enabled: bool,
    line_enabled: bool,
) -> Vec<&'static str> {
    let mut off = Vec::new();
    if !ai_enabled {
        off.push("AI sommelier (GROK_API_KEY + GLM_API_KEY both unset)");
    }
    if !s3_enabled {
        off.push("S3 media uploads (S3_BUCKET / S3_ENDPOINT unset)");
    }
    if !line_enabled {
        off.push("LINE retention broadcasts (LINE_CHANNEL_ACCESS_TOKEN unset)");
    }
    off
}

/// Cycle #133-B: parse a boolean env value. `None` (unset) -> `false`.
/// Otherwise treats `"1"`, `"true"`, `"yes"`, `"on"` (case-insensitive,
/// trimmed) as `true`; everything else as `false`. Mirrors the
/// permissive parsing common in container env (`HIDE_MARKETING_BADGES=true`
/// from a `.env`, `=1` from Railway's CLI).
pub(crate) fn parse_bool_env(raw: Option<&str>) -> bool {
    let Some(s) = raw else { return false };
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

/// Cycle #122: read every name in `names` from the supplied env-reader
/// closure, collecting **all** missing values before returning. Allows
/// `Config::from_env` to report every missing required var in one go
/// instead of the fix-restart-fix-restart cycle the previous
/// `.context("X not set")?` produced.
///
/// `reader` is a closure (not direct `std::env::var`) so the helper is
/// unit-testable without touching the process environment.
///
/// `Ok(values)`: all names had non-empty (after-trim) values, returned
/// in the same order as `names`.
/// `Err(missing)`: list of names whose value was absent or whitespace-
/// only.
pub(crate) fn collect_required_env<F>(names: &[&str], reader: F) -> Result<Vec<String>, Vec<String>>
where
    F: Fn(&str) -> Option<String>,
{
    let mut values = Vec::with_capacity(names.len());
    let mut missing = Vec::new();
    for &name in names {
        match reader(name) {
            Some(v) if !v.trim().is_empty() => values.push(v.trim().to_string()),
            _ => missing.push(name.to_string()),
        }
    }
    if missing.is_empty() {
        Ok(values)
    } else {
        Err(missing)
    }
}

/// Cycle #118: parse `PORT` env var with **strict** semantics — `None`
/// (env var unset) returns the default 3000, but `Some(s)` requires a
/// valid u16 or errors out. The previous `.unwrap_or("3000".into()).parse().unwrap_or(3000)`
/// double-fallback silently swallowed malformed values like
/// `"PORT=8443 "` (trailing space, ascii not parseable to u16) or
/// `"PORT=eight-thousand"`, defaulting to 3000 without surfacing the
/// typo — at best confusing, at worst the bot listens on the wrong
/// port and traffic gets blackholed.
pub(crate) fn parse_port_env(raw: Option<String>) -> Result<u16> {
    match raw {
        None => Ok(3000),
        Some(s) => {
            let trimmed = s.trim();
            // Empty PORT="" is treated like unset, matching the previous
            // tolerant behaviour for users who null-out an inherited var.
            if trimmed.is_empty() {
                return Ok(3000);
            }
            trimmed
                .parse::<u16>()
                .map_err(|e| anyhow!("PORT env var must be a u16 (0..=65535), got {:?}: {}", s, e))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{collect_required_env, disabled_capabilities_from, parse_port_env, Config};
    use std::collections::HashMap;

    #[test]
    fn capabilities_all_enabled_is_empty() {
        assert!(disabled_capabilities_from(true, true, true).is_empty());
    }

    #[test]
    fn capabilities_ai_off_reported() {
        let off = disabled_capabilities_from(false, true, true);
        assert_eq!(off.len(), 1);
        assert!(off[0].contains("AI"));
    }

    #[test]
    fn capabilities_s3_off_reported() {
        let off = disabled_capabilities_from(true, false, true);
        assert_eq!(off.len(), 1);
        assert!(off[0].contains("S3"));
    }

    #[test]
    fn capabilities_line_off_reported() {
        let off = disabled_capabilities_from(true, true, false);
        assert_eq!(off.len(), 1);
        assert!(off[0].contains("LINE"));
    }

    #[test]
    fn capabilities_all_off_reported() {
        assert_eq!(disabled_capabilities_from(false, false, false).len(), 3);
    }

    // ── collect_required_env (cycle #122) ───────────────────────────

    /// Build a closure that reads from a HashMap — lets each test pin
    /// the env state exactly without touching the process env.
    fn map_reader(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> + use<> {
        let map: HashMap<String, String> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        move |name: &str| map.get(name).cloned()
    }

    #[test]
    fn collect_required_env_all_present_returns_values_in_order() {
        let reader = map_reader(&[("BOT_TOKEN", "abc"), ("DATABASE_URL", "postgres://x")]);
        let v = collect_required_env(&["BOT_TOKEN", "DATABASE_URL"], reader).unwrap();
        assert_eq!(v, vec!["abc", "postgres://x"]);
    }

    #[test]
    fn collect_required_env_missing_one_is_listed() {
        let reader = map_reader(&[("BOT_TOKEN", "abc")]);
        let err = collect_required_env(&["BOT_TOKEN", "DATABASE_URL"], reader).unwrap_err();
        assert_eq!(err, vec!["DATABASE_URL"]);
    }

    #[test]
    fn collect_required_env_missing_multiple_listed_together() {
        // This is the cycle-#122 motivator: previously you'd fix one,
        // restart, fix the next. Now both are reported once.
        let reader = map_reader(&[]);
        let err = collect_required_env(&["BOT_TOKEN", "DATABASE_URL"], reader).unwrap_err();
        assert_eq!(err, vec!["BOT_TOKEN", "DATABASE_URL"]);
    }

    #[test]
    fn collect_required_env_empty_string_counts_as_missing() {
        let reader = map_reader(&[("BOT_TOKEN", ""), ("DATABASE_URL", "ok")]);
        let err = collect_required_env(&["BOT_TOKEN", "DATABASE_URL"], reader).unwrap_err();
        assert_eq!(err, vec!["BOT_TOKEN"]);
    }

    #[test]
    fn collect_required_env_whitespace_only_counts_as_missing() {
        let reader = map_reader(&[("BOT_TOKEN", "   "), ("DATABASE_URL", "ok")]);
        let err = collect_required_env(&["BOT_TOKEN", "DATABASE_URL"], reader).unwrap_err();
        assert_eq!(err, vec!["BOT_TOKEN"]);
    }

    #[test]
    fn collect_required_env_trims_returned_values() {
        let reader = map_reader(&[("BOT_TOKEN", "  abc  "), ("DATABASE_URL", "postgres://x")]);
        let v = collect_required_env(&["BOT_TOKEN", "DATABASE_URL"], reader).unwrap();
        assert_eq!(v, vec!["abc", "postgres://x"]);
    }

    // ── parse_port_env (cycle #118) ─────────────────────────────────

    #[test]
    fn port_env_absent_defaults_to_3000() {
        assert_eq!(parse_port_env(None).unwrap(), 3000);
    }

    #[test]
    fn port_env_empty_defaults_to_3000() {
        assert_eq!(parse_port_env(Some("".into())).unwrap(), 3000);
        assert_eq!(parse_port_env(Some("   ".into())).unwrap(), 3000);
    }

    #[test]
    fn port_env_valid_parses() {
        assert_eq!(parse_port_env(Some("8443".into())).unwrap(), 8443);
        assert_eq!(parse_port_env(Some(" 8443 ".into())).unwrap(), 8443);
    }

    // ── parse_bool_env (cycle #133-B) ────────────────────────────────

    #[test]
    fn bool_env_unset_is_false() {
        assert!(!super::parse_bool_env(None));
    }

    #[test]
    fn bool_env_truthy_values() {
        for v in ["1", "true", "TRUE", "True", "yes", "YES", "on", "ON"] {
            assert!(super::parse_bool_env(Some(v)), "expected true for {:?}", v);
        }
    }

    #[test]
    fn bool_env_truthy_trims_whitespace() {
        assert!(super::parse_bool_env(Some("  1  ")));
        assert!(super::parse_bool_env(Some("\ttrue\n")));
    }

    #[test]
    fn bool_env_falsy_values() {
        // Empty, "0", "false", "no", "off", garbage — all false.
        for v in ["", "  ", "0", "false", "FALSE", "no", "off", "maybe", "🤔"] {
            assert!(
                !super::parse_bool_env(Some(v)),
                "expected false for {:?}",
                v
            );
        }
    }

    #[test]
    fn port_env_malformed_errors() {
        // The original bug — silent fallback to 3000 hid these.
        assert!(parse_port_env(Some("eight-thousand".into())).is_err());
        assert!(parse_port_env(Some("3000abc".into())).is_err());
        // Out of u16 range
        assert!(parse_port_env(Some("99999".into())).is_err());
        // Negative
        assert!(parse_port_env(Some("-1".into())).is_err());
    }

    #[test]
    fn test_s3_enabled_both_set() {
        let cfg = Config {
            bot_token: String::new(),
            web_app_url: String::new(),
            port: 3000,
            webhook_path: String::new(),
            app_url: String::new(),
            is_production: false,
            admin_ids: vec![],
            bot_username: String::new(),
            database_url: String::new(),
            grok_api_key: String::new(),
            glm_api_key: String::new(),
            s3_bucket: Some("bucket".into()),
            s3_endpoint: Some("http://s3".into()),
            s3_internal_endpoint: None,
            s3_public_url: None,
            s3_region: None,
            s3_access_key: None,
            s3_secret_key: None,
            backup_assets: false,
            admin_password: None,
            hide_marketing_badges: false,
            delivery_zones: crate::delivery::DeliveryZones::default(),
            promptpay: crate::promptpay::QrConfig::default(),
            line_channel_access_token: None,
        };
        assert!(cfg.s3_enabled());
    }

    #[test]
    fn test_s3_enabled_missing_endpoint() {
        let cfg = Config {
            s3_bucket: Some("bucket".into()),
            s3_endpoint: None,
            ..test_config()
        };
        assert!(!cfg.s3_enabled());
    }

    #[test]
    fn test_s3_enabled_missing_bucket() {
        let cfg = Config {
            s3_bucket: None,
            s3_endpoint: Some("http://s3".into()),
            ..test_config()
        };
        assert!(!cfg.s3_enabled());
    }

    #[test]
    fn test_s3_enabled_both_missing() {
        let cfg = Config {
            s3_bucket: None,
            s3_endpoint: None,
            ..test_config()
        };
        assert!(!cfg.s3_enabled());
    }

    fn test_config() -> Config {
        Config {
            bot_token: String::new(),
            web_app_url: String::new(),
            port: 3000,
            webhook_path: String::new(),
            app_url: String::new(),
            is_production: false,
            admin_ids: vec![],
            bot_username: String::new(),
            database_url: String::new(),
            grok_api_key: String::new(),
            glm_api_key: String::new(),
            s3_bucket: None,
            s3_endpoint: None,
            s3_internal_endpoint: None,
            s3_public_url: None,
            s3_region: None,
            s3_access_key: None,
            s3_secret_key: None,
            backup_assets: false,
            admin_password: None,
            hide_marketing_badges: false,
            delivery_zones: crate::delivery::DeliveryZones::default(),
            promptpay: crate::promptpay::QrConfig::default(),
            line_channel_access_token: None,
        }
    }
}
