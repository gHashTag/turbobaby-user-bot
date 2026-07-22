// Admin auth helper.
//
// Validates Telegram WebApp initData HMAC signature using BOT_TOKEN.
// The frontend sends the raw `Telegram.WebApp.initData` string in the
// `X-Telegram-Init-Data` header. The server verifies the HMAC, extracts
// the user id, and checks it against admin_ids.
//
// Fallback: X-Admin-Token password login (no localhost bypass).

use axum::http::{HeaderMap, StatusCode};
use hmac::{Hmac, Mac};
use sea_orm::EntityTrait;
use sha2::Sha256;

use crate::db::entities::loyalty_profile;
use crate::AppState;

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug)]
pub(crate) struct TelegramUser {
    pub id: i64,
    pub first_name: Option<String>,
    pub username: Option<String>,
}

/// Validate Telegram WebApp initData HMAC signature.
///
/// Algorithm (per Telegram docs):
/// 1. Parse URL-encoded key=value pairs.
/// 2. Remove `hash` AND `signature` keys. `signature` is Telegram's Ed25519
///    third-party-validation field; it is NOT part of the HMAC data-check-string
///    and is now present in every Mini App's initData. Leaving it in makes the
///    HMAC never match → permanent 401 (this was the garden-401 root cause).
/// 3. Sort remaining keys alphabetically.
/// 4. Build data_check_string = "key1=value1\nkey2=value2\n..."
/// 5. secret_key = HMAC_SHA256(key="WebAppData", msg=BOT_TOKEN)
/// 6. expected_hash = HMAC_SHA256(key=secret_key, msg=data_check_string) in hex
/// 7. Compare expected_hash with received `hash` (constant-time)
pub(crate) fn validate_init_data(init_data: &str, bot_token: &str) -> Option<TelegramUser> {
    if init_data.len() > 4096 {
        tracing::warn!("init_data too long ({} bytes)", init_data.len());
        return None;
    }
    tracing::debug!(
        "validate_init_data: len={}, hash_present={}",
        init_data.len(),
        init_data.contains("hash=")
    );
    let mut pairs: Vec<(String, String)> = Vec::new();
    for pair in init_data.split('&') {
        let mut parts = pair.splitn(2, '=');
        let key = parts.next()?;
        let value = parts.next().unwrap_or("");
        pairs.push((key.to_string(), value.to_string()));
    }

    let hash = pairs
        .iter()
        .find(|(k, _)| k == "hash")
        .map(|(_, v)| v.clone())?;

    let mut data_pairs: Vec<_> = pairs
        .into_iter()
        .filter(|(k, _)| k != "hash" && k != "signature")
        .collect();
    data_pairs.sort_by(|a, b| a.0.cmp(&b.0));

    // Build data_check_string from URL-decoded values (real Telegram behavior)
    let data_check_string_decoded = data_pairs
        .iter()
        .map(|(k, v)| {
            let kd = urlencoding::decode(k).unwrap_or(std::borrow::Cow::Borrowed(k));
            let vd = urlencoding::decode(v).unwrap_or(std::borrow::Cow::Borrowed(v));
            format!("{}={}", kd, vd)
        })
        .collect::<Vec<_>>()
        .join("\n");

    // Build data_check_string from raw values (fallback for some generators)
    let data_check_string_raw = data_pairs
        .iter()
        .map(|(k, v)| format!("{}={}", k, v))
        .collect::<Vec<_>>()
        .join("\n");

    // secret_key = HMAC_SHA256("WebAppData", bot_token)
    let mut secret_mac = HmacSha256::new_from_slice(b"WebAppData").ok()?;
    secret_mac.update(bot_token.as_bytes());
    let secret_key = secret_mac.finalize().into_bytes();

    // expected_hash (decoded)
    let mut mac = HmacSha256::new_from_slice(&secret_key).ok()?;
    mac.update(data_check_string_decoded.as_bytes());
    let expected_hash = hex::encode(mac.finalize().into_bytes());

    // expected_hash (raw fallback)
    let mut mac_raw = HmacSha256::new_from_slice(&secret_key).ok()?;
    mac_raw.update(data_check_string_raw.as_bytes());
    let expected_hash_raw = hex::encode(mac_raw.finalize().into_bytes());

    // Try both constant-time comparisons
    let ok_decoded = constant_time_eq::constant_time_eq(expected_hash.as_bytes(), hash.as_bytes());
    let ok_raw = constant_time_eq::constant_time_eq(expected_hash_raw.as_bytes(), hash.as_bytes());

    if !ok_decoded && !ok_raw {
        // Safe diagnostics: log hash lengths and first/last bytes, plus key list,
        // but never the full initData or user PII. Helps catch token/format drift.
        let hash_prefix = hash.chars().take(8).collect::<String>();
        let hash_suffix = hash.chars().rev().take(4).collect::<String>();
        let exp_dec_prefix = expected_hash.chars().take(8).collect::<String>();
        let exp_dec_suffix = expected_hash.chars().rev().take(4).collect::<String>();
        let exp_raw_prefix = expected_hash_raw.chars().take(8).collect::<String>();
        let exp_raw_suffix = expected_hash_raw.chars().rev().take(4).collect::<String>();
        let key_list: Vec<&str> = data_pairs.iter().map(|(k, _)| k.as_str()).collect();
        tracing::warn!(
            "initData HMAC mismatch: hash_len={} keys={:?} hash=[{}..{}] expected_decoded=[{}..{}] expected_raw=[{}..{}]",
            hash.len(),
            key_list,
            hash_prefix,
            hash_suffix,
            exp_dec_prefix,
            exp_dec_suffix,
            exp_raw_prefix,
            exp_raw_suffix
        );
        return None;
    }

    // Validate auth_date freshness (initData valid for 24h per Telegram docs)
    let auth_date = data_pairs
        .iter()
        .find(|(k, _)| k == "auth_date")
        .and_then(|(_, v)| v.parse::<i64>().ok());
    if let Some(ad) = auth_date {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .ok()?
            .as_secs() as i64;
        if ad > now || now - ad > 86400 {
            tracing::warn!(
                "initData expired or future-dated: auth_date={} now={}",
                ad,
                now
            );
            return None;
        }
    } else {
        tracing::warn!("initData missing auth_date");
        return None;
    }

    // Extract user JSON
    let user_json = data_pairs
        .iter()
        .find(|(k, _)| k == "user")
        .map(|(_, v)| v.as_str())?;

    let user_decoded = urlencoding::decode(user_json).ok()?;
    let user: serde_json::Value = serde_json::from_str(&user_decoded).ok()?;

    let id = user.get("id")?.as_i64()?;
    let first_name = user
        .get("first_name")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let username = user
        .get("username")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    Some(TelegramUser {
        id,
        first_name,
        username,
    })
}

/// Detailed diagnostics returned by `validate_init_data_debug`.
#[derive(Debug)]
pub(crate) struct InitDataDebugInfo {
    pub ok: bool,
    pub data_check_string_decoded: String,
    pub data_check_string_raw: String,
    pub data_check_string_with_signature: String,
    pub hash: String,
    pub expected_hash_decoded: String,
    pub expected_hash_raw: String,
    pub expected_hash_with_signature: String,
    pub expected_hash_decoded_alt_secret: String,
    pub expected_hash_raw_alt_secret: String,
    pub expected_hash_with_signature_alt_secret: String,
    pub keys: Vec<String>,
    pub user: Option<TelegramUser>,
    pub error: Option<String>,
}

/// Debug version that returns detailed validation info instead of just Option.
pub(crate) fn validate_init_data_debug(init_data: &str, bot_token: &str) -> InitDataDebugInfo {
    if init_data.len() > 4096 {
        return InitDataDebugInfo {
            ok: false,
            data_check_string_decoded: String::new(),
            data_check_string_raw: String::new(),
            data_check_string_with_signature: String::new(),
            hash: String::new(),
            expected_hash_decoded: String::new(),
            expected_hash_raw: String::new(),
            expected_hash_with_signature: String::new(),
            expected_hash_decoded_alt_secret: String::new(),
            expected_hash_raw_alt_secret: String::new(),
            expected_hash_with_signature_alt_secret: String::new(),
            keys: Vec::new(),
            user: None,
            error: Some("init_data too long".to_string()),
        };
    }
    let mut pairs: Vec<(String, String)> = Vec::new();
    for pair in init_data.split('&') {
        let mut parts = pair.splitn(2, '=');
        let key = match parts.next() {
            Some(k) => k,
            None => {
                return InitDataDebugInfo {
                    ok: false,
                    data_check_string_decoded: String::new(),
                    data_check_string_raw: String::new(),
                    data_check_string_with_signature: String::new(),
                    hash: String::new(),
                    expected_hash_decoded: String::new(),
                    expected_hash_raw: String::new(),
                    expected_hash_with_signature: String::new(),
                    expected_hash_decoded_alt_secret: String::new(),
                    expected_hash_raw_alt_secret: String::new(),
                    expected_hash_with_signature_alt_secret: String::new(),
                    keys: Vec::new(),
                    user: None,
                    error: Some("empty pair".to_string()),
                }
            }
        };
        let value = parts.next().unwrap_or("");
        pairs.push((key.to_string(), value.to_string()));
    }

    let keys: Vec<String> = pairs.iter().map(|(k, _)| k.clone()).collect();

    let hash = match pairs
        .iter()
        .find(|(k, _)| k == "hash")
        .map(|(_, v)| v.clone())
    {
        Some(h) => h,
        None => {
            return InitDataDebugInfo {
                ok: false,
                data_check_string_decoded: String::new(),
                data_check_string_raw: String::new(),
                data_check_string_with_signature: String::new(),
                hash: String::new(),
                expected_hash_decoded: String::new(),
                expected_hash_raw: String::new(),
                expected_hash_with_signature: String::new(),
                expected_hash_decoded_alt_secret: String::new(),
                expected_hash_raw_alt_secret: String::new(),
                expected_hash_with_signature_alt_secret: String::new(),
                keys,
                user: None,
                error: Some("missing hash".to_string()),
            }
        }
    };

    let pairs_clone = pairs.clone();

    let mut data_pairs: Vec<_> = pairs
        .into_iter()
        .filter(|(k, _)| k != "hash" && k != "signature")
        .collect();
    data_pairs.sort_by(|a, b| a.0.cmp(&b.0));

    let data_check_string_decoded = data_pairs
        .iter()
        .map(|(k, v)| {
            let kd = urlencoding::decode(k).unwrap_or(std::borrow::Cow::Borrowed(k));
            let vd = urlencoding::decode(v).unwrap_or(std::borrow::Cow::Borrowed(v));
            format!("{}={}", kd, vd)
        })
        .collect::<Vec<_>>()
        .join("\n");

    let data_check_string_raw = data_pairs
        .iter()
        .map(|(k, v)| format!("{}={}", k, v))
        .collect::<Vec<_>>()
        .join("\n");

    // Variant where signature is NOT excluded (some Telegram clients reportedly
    // include it in the HMAC data-check-string). Kept for diagnostics only.
    let mut data_pairs_with_sig: Vec<_> = pairs_clone
        .into_iter()
        .filter(|(k, _)| k != "hash")
        .collect();
    data_pairs_with_sig.sort_by(|a, b| a.0.cmp(&b.0));
    let data_check_string_with_signature = data_pairs_with_sig
        .iter()
        .map(|(k, v)| {
            let kd = urlencoding::decode(k).unwrap_or(std::borrow::Cow::Borrowed(k));
            let vd = urlencoding::decode(v).unwrap_or(std::borrow::Cow::Borrowed(v));
            format!("{}={}", kd, vd)
        })
        .collect::<Vec<_>>()
        .join("\n");

    let mut secret_mac = match HmacSha256::new_from_slice(b"WebAppData") {
        Ok(m) => m,
        Err(_) => {
            return InitDataDebugInfo {
                ok: false,
                data_check_string_decoded,
                data_check_string_raw,
                data_check_string_with_signature: String::new(),
                hash,
                expected_hash_decoded: String::new(),
                expected_hash_raw: String::new(),
                expected_hash_with_signature: String::new(),
                expected_hash_decoded_alt_secret: String::new(),
                expected_hash_raw_alt_secret: String::new(),
                expected_hash_with_signature_alt_secret: String::new(),
                keys,
                user: None,
                error: Some("HMAC init failed".to_string()),
            }
        }
    };
    secret_mac.update(bot_token.as_bytes());
    let secret_key = secret_mac.finalize().into_bytes();

    // Diagnostic: Telegram docs say the secret is HMAC("WebAppData", bot_token).
    // Some reports claim only the token payload (after "bot_id:") is used.
    let alt_secret_key = bot_token.find(':').and_then(|idx| {
        HmacSha256::new_from_slice(b"WebAppData").ok().map(|mut alt_mac| {
            alt_mac.update(bot_token[idx + 1..].as_bytes());
            alt_mac.finalize().into_bytes()
        })
    });

    let mut mac = match HmacSha256::new_from_slice(&secret_key) {
        Ok(m) => m,
        Err(_) => {
            return InitDataDebugInfo {
                ok: false,
                data_check_string_decoded,
                data_check_string_raw,
                data_check_string_with_signature: String::new(),
                hash,
                expected_hash_decoded: String::new(),
                expected_hash_raw: String::new(),
                expected_hash_with_signature: String::new(),
                expected_hash_decoded_alt_secret: String::new(),
                expected_hash_raw_alt_secret: String::new(),
                expected_hash_with_signature_alt_secret: String::new(),
                keys,
                user: None,
                error: Some("HMAC init failed".to_string()),
            }
        }
    };
    mac.update(data_check_string_decoded.as_bytes());
    let expected_hash_decoded = hex::encode(mac.finalize().into_bytes());

    let mut mac_raw = match HmacSha256::new_from_slice(&secret_key) {
        Ok(m) => m,
        Err(_) => {
            return InitDataDebugInfo {
                ok: false,
                data_check_string_decoded,
                data_check_string_raw,
                data_check_string_with_signature: String::new(),
                hash,
                expected_hash_decoded,
                expected_hash_raw: String::new(),
                expected_hash_with_signature: String::new(),
                expected_hash_decoded_alt_secret: String::new(),
                expected_hash_raw_alt_secret: String::new(),
                expected_hash_with_signature_alt_secret: String::new(),
                keys,
                user: None,
                error: Some("HMAC init failed".to_string()),
            }
        }
    };
    mac_raw.update(data_check_string_raw.as_bytes());
    let expected_hash_raw = hex::encode(mac_raw.finalize().into_bytes());

    let mut mac_sig = match HmacSha256::new_from_slice(&secret_key) {
        Ok(m) => m,
        Err(_) => {
            return InitDataDebugInfo {
                ok: false,
                data_check_string_decoded,
                data_check_string_raw,
                data_check_string_with_signature,
                hash,
                expected_hash_decoded,
                expected_hash_raw,
                expected_hash_with_signature: String::new(),
                expected_hash_decoded_alt_secret: String::new(),
                expected_hash_raw_alt_secret: String::new(),
                expected_hash_with_signature_alt_secret: String::new(),
                keys,
                user: None,
                error: Some("HMAC init failed".to_string()),
            }
        }
    };
    mac_sig.update(data_check_string_with_signature.as_bytes());
    let expected_hash_with_signature = hex::encode(mac_sig.finalize().into_bytes());

    let (expected_hash_decoded_alt_secret, expected_hash_raw_alt_secret, expected_hash_with_signature_alt_secret) =
        if let Some(ref alt_key) = alt_secret_key {
            let dec = HmacSha256::new_from_slice(alt_key)
                .map(|mut m| { m.update(data_check_string_decoded.as_bytes()); hex::encode(m.finalize().into_bytes()) })
                .unwrap_or_default();
            let raw = HmacSha256::new_from_slice(alt_key)
                .map(|mut m| { m.update(data_check_string_raw.as_bytes()); hex::encode(m.finalize().into_bytes()) })
                .unwrap_or_default();
            let sig = HmacSha256::new_from_slice(alt_key)
                .map(|mut m| { m.update(data_check_string_with_signature.as_bytes()); hex::encode(m.finalize().into_bytes()) })
                .unwrap_or_default();
            (dec, raw, sig)
        } else {
            (String::new(), String::new(), String::new())
        };

    let user = data_pairs
        .iter()
        .find(|(k, _)| k == "user")
        .and_then(|(_, v)| {
            let decoded = urlencoding::decode(v).ok()?;
            let user: serde_json::Value = serde_json::from_str(&decoded).ok()?;
            let id = user.get("id")?.as_i64()?;
            let first_name = user
                .get("first_name")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let username = user
                .get("username")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            Some(TelegramUser {
                id,
                first_name,
                username,
            })
        });

    let ok_decoded =
        constant_time_eq::constant_time_eq(expected_hash_decoded.as_bytes(), hash.as_bytes());
    let ok_raw =
        constant_time_eq::constant_time_eq(expected_hash_raw.as_bytes(), hash.as_bytes());
    let ok_with_sig =
        constant_time_eq::constant_time_eq(expected_hash_with_signature.as_bytes(), hash.as_bytes());
    let mut ok = ok_decoded || ok_raw;
    let mut error = None;

    if !ok {
        error = Some(format!(
            "HMAC mismatch: decoded={} raw={} with_sig={} hash_len={}",
            ok_decoded,
            ok_raw,
            ok_with_sig,
            hash.len()
        ));
    } else {
        let auth_date = data_pairs
            .iter()
            .find(|(k, _)| k == "auth_date")
            .and_then(|(_, v)| v.parse::<i64>().ok());
        if let Some(ad) = auth_date {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .ok()
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);
            if ad > now || now.saturating_sub(ad) > 86400 {
                ok = false;
                error = Some(format!(
                    "initData expired or future-dated: auth_date={} now={}",
                    ad, now
                ));
            }
        } else {
            ok = false;
            error = Some("initData missing auth_date".to_string());
        }
    }

    InitDataDebugInfo {
        ok,
        data_check_string_decoded,
        data_check_string_raw,
        data_check_string_with_signature,
        hash,
        expected_hash_decoded,
        expected_hash_raw,
        expected_hash_with_signature,
        expected_hash_decoded_alt_secret,
        expected_hash_raw_alt_secret,
        expected_hash_with_signature_alt_secret,
        keys,
        user,
        error,
    }
}

#[allow(unreachable_pub)] // Used by tests/integration_*.rs to mint admin tokens.
pub fn generate_admin_token(password: &str, bot_token: &str) -> String {
    let mut mac = match HmacSha256::new_from_slice(b"WoodyWeedBotAdmin") {
        Ok(m) => m,
        Err(_) => return String::new(),
    };
    mac.update(bot_token.as_bytes());
    mac.update(b":");
    mac.update(password.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

pub(crate) fn verify_admin_token(token: &str, bot_token: &str, expected_password: &str) -> bool {
    let expected = generate_admin_token(expected_password, bot_token);
    constant_time_eq::constant_time_eq(token.as_bytes(), expected.as_bytes())
}

/// Cycle #106: per-IP sliding-window-log rate-limit for failed admin
/// auth attempts. Both auth paths (Telegram initData and X-Admin-Token
/// password) were brute-forceable without bound — initData is HMAC so
/// the keyspace makes brute-force infeasible, but a weak admin password
/// could be cracked at HTTP speed. 10 failed attempts per 5 minutes per
/// IP gives generous slack for typos while bounding brute-force to
/// ~120 attempts/hour (a 35-bit password still takes 32+ years).
///
/// Uses the sync variant (`std::sync::Mutex`) so this stays in the
/// existing sync `check_admin` signature — wrapping the 57 call-sites
/// in `.await` adds churn without algorithmic value (the critical
/// section is microseconds, no await inside the lock).
static ADMIN_AUTH_RATE_LIMIT: std::sync::LazyLock<crate::api::rate_limit::SyncSlidingWindowStore> =
    std::sync::LazyLock::new(crate::api::rate_limit::new_sync_store);
const ADMIN_AUTH_RL_WINDOW: std::time::Duration = std::time::Duration::from_secs(5 * 60);
const ADMIN_AUTH_RL_MAX_ATTEMPTS: usize = 10;
const ADMIN_AUTH_RL_MAX_IPS: usize = 10_000;

pub(crate) fn check_admin(headers: &HeaderMap, state: &AppState) -> Result<i64, StatusCode> {
    // Debug-level diagnostics only; admin_ids values are sensitive and never logged.
    tracing::debug!(
        "CHECK_ADMIN: bot_token_len={} admin_count={} admin_password_set={} init_data_present={} token_present={}",
        state.config.bot_token.len(),
        state.config.admin_ids.len(),
        state.config.admin_password.is_some(),
        headers.get("X-Telegram-Init-Data").is_some(),
        headers.get("X-Admin-Token").is_some(),
    );

    // 1. Try Telegram initData validation (production path)
    if let Some(init_data) = headers
        .get("X-Telegram-Init-Data")
        .and_then(|v| v.to_str().ok())
    {
        if !init_data.is_empty() {
            if let Some(user) = validate_init_data(init_data, &state.config.bot_token) {
                if state.config.admin_ids.contains(&user.id) {
                    tracing::debug!(
                        "admin authenticated via initData telegram_id={} username={:?}",
                        user.id,
                        user.username
                    );
                    return Ok(user.id);
                } else {
                    tracing::warn!(
                        "initData valid but user not admin telegram_id={} — trying X-Admin-Token fallback",
                        user.id,
                    );
                    // Don't return here — allow password fallback below
                }
            } else {
                tracing::warn!("invalid initData signature — falling back to X-Admin-Token");
                // Don't return here — allow password fallback below
            }
        }
    }

    // 2. Password login via X-Admin-Token (works in all builds if ADMIN_PASSWORD is set)
    if let Some(token) = headers.get("X-Admin-Token").and_then(|v| v.to_str().ok()) {
        if let Some(ref password) = state.config.admin_password {
            if verify_admin_token(token, &state.config.bot_token, password) {
                tracing::info!("admin authenticated via password token");
                return Ok(0);
            } else {
                tracing::warn!("CHECK_ADMIN: token verification FAILED");
            }
        } else {
            tracing::warn!("CHECK_ADMIN: ADMIN_PASSWORD not set");
        }
    }

    tracing::warn!("admin request without valid auth (initData or token)");
    crate::metrics::auth_failure("admin_no_valid_auth");
    record_failed_admin_attempt(headers)?;
    Err(StatusCode::UNAUTHORIZED)
}

/// Cycle #126: shared rate-limit hook for admin-auth failure paths.
/// Originally inlined in `check_admin` (cycle #106). Extracted so
/// `admin_login` can call the same `ADMIN_AUTH_RATE_LIMIT` store —
/// brute-force across either path now counts against the same per-IP
/// bucket. Returns `Ok(())` if the caller may proceed (with whatever
/// 401/403 status fits its semantics) or `Err(429)` if the IP has
/// crossed the threshold.
pub(crate) fn record_failed_admin_attempt(headers: &HeaderMap) -> Result<(), StatusCode> {
    let client_ip = crate::api::rate_limit::client_ip_from_headers(headers);
    let allowed = crate::api::rate_limit::check_and_record_sync(
        &ADMIN_AUTH_RATE_LIMIT,
        &client_ip,
        ADMIN_AUTH_RL_WINDOW,
        ADMIN_AUTH_RL_MAX_ATTEMPTS,
        ADMIN_AUTH_RL_MAX_IPS,
    );
    if !allowed {
        crate::metrics::rate_limit_blocked("admin_auth");
        tracing::warn!("admin auth: rate-limit exceeded for ip={}", client_ip);
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }
    Ok(())
}

/// Verify that the Telegram user in `X-Telegram-Init-Data` owns `expected_telegram_id`.
/// Returns the authenticated telegram_id on success.
pub(crate) fn check_owner(
    headers: &HeaderMap,
    state: &AppState,
    expected_telegram_id: i64,
) -> Result<i64, StatusCode> {
    if let Some(init_data) = headers
        .get("X-Telegram-Init-Data")
        .and_then(|v| v.to_str().ok())
    {
        if !init_data.is_empty() {
            if let Some(user) = validate_init_data(init_data, &state.config.bot_token) {
                if user.id == expected_telegram_id {
                    return Ok(user.id);
                } else {
                    tracing::warn!(
                        "owner mismatch: initData user={} expected={}",
                        user.id,
                        expected_telegram_id
                    );
                    crate::metrics::auth_failure("owner_mismatch");
                    return Err(StatusCode::FORBIDDEN);
                }
            } else {
                tracing::warn!("owner check failed: invalid initData");
                crate::metrics::auth_failure("invalid_init_data");
                return Err(StatusCode::UNAUTHORIZED);
            }
        }
    }
    tracing::warn!("owner check failed: missing initData");
    crate::metrics::auth_failure("missing_init_data");
    Err(StatusCode::UNAUTHORIZED)
}

/// Returns `Ok(())` if the user is not blocked.
/// Returns `Err(StatusCode::FORBIDDEN)` if the user is blocked or DB lookup fails.
pub(crate) async fn check_not_blocked(
    state: &AppState,
    telegram_id: i64,
) -> Result<(), StatusCode> {
    let result = loyalty_profile::Entity::find_by_id(telegram_id)
        .one(&state.db.orm)
        .await;

    match result {
        Ok(Some(profile)) if profile.is_blocked => {
            tracing::warn!("blocked user attempted action telegram_id={}", telegram_id);
            crate::metrics::auth_failure("user_blocked");
            Err(StatusCode::FORBIDDEN)
        }
        Ok(_) => Ok(()),
        Err(e) => {
            tracing::error!(
                "check_not_blocked DB error telegram_id={} err={}",
                telegram_id,
                e
            );
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Validates that a Telegram ID extracted from a path/query parameter is positive
/// and within the safe integer range (i64 ≤ 2^53-1).
pub(crate) fn validate_telegram_id_param(id: i64) -> Result<(), StatusCode> {
    if id <= 0 {
        return Err(StatusCode::BAD_REQUEST);
    }
    if id > 9_007_199_254_740_991 {
        return Err(StatusCode::BAD_REQUEST);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn generate_init_data(bot_token: &str, user_id: i64, first_name: &str) -> String {
        let user_json = format!("{{\"id\":{},\"first_name\":\"{}\"}}", user_id, first_name);
        let user_json_for_encode = user_json.clone();
        let user_encoded = urlencoding::encode(&user_json_for_encode);
        // Use a fresh auth_date so freshness check passes (within last hour)
        let auth_date = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64
            - 3600;
        generate_init_data_with_date(
            bot_token,
            user_id,
            first_name,
            auth_date,
            &user_json,
            &user_encoded,
        )
    }

    fn generate_init_data_with_date(
        bot_token: &str,
        _user_id: i64,
        _first_name: &str,
        auth_date: i64,
        user_json: &str,
        user_encoded: &str,
    ) -> String {
        let auth_date_str = auth_date.to_string();
        let mut pairs = [
            ("auth_date".to_string(), auth_date_str.clone()),
            ("user".to_string(), user_json.to_string()),
        ];
        pairs.sort_by(|a, b| a.0.cmp(&b.0));
        let data_check_string = pairs
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>()
            .join("\n");

        let mut secret_mac = HmacSha256::new_from_slice(b"WebAppData").unwrap();
        secret_mac.update(bot_token.as_bytes());
        let secret_key = secret_mac.finalize().into_bytes();

        let mut mac = HmacSha256::new_from_slice(&secret_key).unwrap();
        mac.update(data_check_string.as_bytes());
        let hash = hex::encode(mac.finalize().into_bytes());

        format!(
            "auth_date={}&hash={}&user={}",
            auth_date_str, hash, user_encoded
        )
    }

    /// Build initData that also carries the Ed25519 `signature` field Telegram
    /// now attaches to every Mini App launch. The `hash` is computed over the
    /// data-check-string that EXCLUDES both `hash` and `signature` (per spec),
    /// while the emitted string still contains an arbitrary `signature=` value —
    /// exactly what a real client sends. Validation must ignore `signature`.
    fn generate_init_data_with_signature(
        bot_token: &str,
        user_id: i64,
        first_name: &str,
        signature: &str,
    ) -> String {
        let user_json = format!("{{\"id\":{},\"first_name\":\"{}\"}}", user_id, first_name);
        let user_encoded = urlencoding::encode(&user_json);
        let auth_date = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64
            - 3600;
        let auth_date_str = auth_date.to_string();

        // data-check-string excludes hash AND signature.
        let mut pairs = [
            ("auth_date".to_string(), auth_date_str.clone()),
            ("user".to_string(), user_json.clone()),
        ];
        pairs.sort_by(|a, b| a.0.cmp(&b.0));
        let data_check_string = pairs
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>()
            .join("\n");

        let mut secret_mac = HmacSha256::new_from_slice(b"WebAppData").unwrap();
        secret_mac.update(bot_token.as_bytes());
        let secret_key = secret_mac.finalize().into_bytes();

        let mut mac = HmacSha256::new_from_slice(&secret_key).unwrap();
        mac.update(data_check_string.as_bytes());
        let hash = hex::encode(mac.finalize().into_bytes());

        // The emitted query string carries signature; order is irrelevant.
        format!(
            "auth_date={}&signature={}&hash={}&user={}",
            auth_date_str, signature, hash, user_encoded
        )
    }

    #[test]
    fn test_validate_init_data_with_signature_field() {
        // Regression: Telegram adds an Ed25519 `signature` field to initData.
        // It must be excluded from the HMAC data-check-string alongside `hash`;
        // otherwise HMAC never matches and every request 401s (garden-401 bug).
        let token = "test_bot_token_12345";
        let init_data = generate_init_data_with_signature(
            token,
            8420420131,
            "ShopOwner",
            "abcDEF123_signature-value",
        );
        assert!(
            init_data.contains("signature="),
            "test fixture must include a signature field"
        );
        let user = validate_init_data(&init_data, token)
            .expect("initData with a signature field must still validate");
        assert_eq!(user.id, 8420420131);
        assert_eq!(user.first_name, Some("ShopOwner".to_string()));
    }

    #[test]
    fn test_validate_init_data_debug_with_signature_field() {
        let token = "test_bot_token_12345";
        let init_data =
            generate_init_data_with_signature(token, 8420420131, "ShopOwner", "sig_xyz");
        let info = validate_init_data_debug(&init_data, token);
        assert!(
            info.ok,
            "debug validator must accept signature-bearing initData; err={:?}",
            info.error
        );
        assert_eq!(info.user.map(|u| u.id), Some(8420420131));
    }

    #[test]
    fn test_validate_init_data_with_query_id_and_full_user_json() {
        // Regression for real Telegram initData: it carries query_id, and
        // user JSON includes is_premium, photo_url, last_name, etc. All of
        // these must participate in the HMAC data-check-string exactly as
        // received (URL-decoded), and signature must still be excluded.
        let token = "test_bot_token_12345";
        let user_json = r#"{"id":144022504,"first_name":"Dmitrii","last_name":"T27 DEV","username":"t27_dev","language_code":"ru","is_premium":true,"allows_write_to_pm":true,"photo_url":"https:\/\/t.me\/i\/userpic\/320\/abc.jpg"}"#;
        let user_encoded = urlencoding::encode(user_json);
        let auth_date = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64
            - 3600;
        let auth_date_str = auth_date.to_string();
        let query_id = "AAHom5UIAAAAAOiblQhHtdzh";

        let mut pairs = [
            ("auth_date".to_string(), auth_date_str.clone()),
            ("query_id".to_string(), query_id.to_string()),
            ("user".to_string(), user_json.to_string()),
        ];
        pairs.sort_by(|a, b| a.0.cmp(&b.0));
        let data_check_string = pairs
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>()
            .join("\n");

        let mut secret_mac = HmacSha256::new_from_slice(b"WebAppData").unwrap();
        secret_mac.update(token.as_bytes());
        let secret_key = secret_mac.finalize().into_bytes();

        let mut mac = HmacSha256::new_from_slice(&secret_key).unwrap();
        mac.update(data_check_string.as_bytes());
        let hash = hex::encode(mac.finalize().into_bytes());

        let signature = "dummy_sig_value";
        let init_data = format!(
            "auth_date={}&query_id={}&signature={}&hash={}&user={}",
            auth_date_str, query_id, signature, hash, user_encoded
        );

        let user = validate_init_data(&init_data, token)
            .expect("realistic initData with query_id + full user JSON must validate");
        assert_eq!(user.id, 144022504);
        assert_eq!(user.first_name, Some("Dmitrii".to_string()));
    }

    #[test]
    fn test_validate_init_data_success() {
        let token = "test_bot_token_12345";
        let init_data = generate_init_data(token, 8420420131, "ShopOwner");
        let user = validate_init_data(&init_data, token).expect("should validate");
        assert_eq!(user.id, 8420420131);
        assert_eq!(user.first_name, Some("ShopOwner".to_string()));
    }

    #[test]
    fn test_validate_init_data_bad_hash() {
        let token = "test_bot_token_12345";
        let mut init_data = generate_init_data(token, 8420420131, "ShopOwner");
        // Corrupt the hash
        init_data = init_data.replace("hash=", "hash=bad");
        assert!(validate_init_data(&init_data, token).is_none());
    }

    #[test]
    fn test_validate_init_data_wrong_token() {
        let token = "test_bot_token_12345";
        let init_data = generate_init_data(token, 8420420131, "ShopOwner");
        assert!(validate_init_data(&init_data, "wrong_token").is_none());
    }

    #[test]
    fn test_validate_init_data_missing_user() {
        let token = "test_bot_token_12345";
        let init_data = "auth_date=1234567890&hash=abcd1234";
        assert!(validate_init_data(init_data, token).is_none());
    }

    #[test]
    fn test_validate_init_data_expired() {
        let token = "test_bot_token_12345";
        let user_id: i64 = 8420420131;
        let user_json = format!("{{\"id\":{},\"first_name\":\"{}\"}}", user_id, "ShopOwner");
        let user_encoded = urlencoding::encode(&user_json);
        // auth_date 25 hours in the past
        let auth_date = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64
            - 90000;
        let init_data = generate_init_data_with_date(
            token,
            user_id,
            "ShopOwner",
            auth_date,
            &user_json,
            &user_encoded,
        );
        assert!(validate_init_data(&init_data, token).is_none());
    }

    #[test]
    fn test_validate_init_data_future_dated() {
        let token = "test_bot_token_12345";
        let user_id: i64 = 8420420131;
        let user_json = format!("{{\"id\":{},\"first_name\":\"{}\"}}", user_id, "ShopOwner");
        let user_encoded = urlencoding::encode(&user_json);
        // auth_date 1 hour in the future
        let auth_date = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64
            + 3600;
        let init_data = generate_init_data_with_date(
            token,
            user_id,
            "ShopOwner",
            auth_date,
            &user_json,
            &user_encoded,
        );
        assert!(validate_init_data(&init_data, token).is_none());
    }

    #[test]
    fn test_generate_admin_token_deterministic() {
        let t1 = generate_admin_token("password123", "bot_token");
        let t2 = generate_admin_token("password123", "bot_token");
        assert_eq!(t1, t2);
        assert_eq!(t1.len(), 64); // hex-encoded SHA-256
    }

    #[test]
    fn test_verify_admin_token_success() {
        let token = generate_admin_token("secret", "bot");
        assert!(verify_admin_token(&token, "bot", "secret"));
    }

    #[test]
    fn test_verify_admin_token_wrong_password() {
        let token = generate_admin_token("secret", "bot");
        assert!(!verify_admin_token(&token, "bot", "wrong"));
    }

    #[test]
    fn test_verify_admin_token_wrong_bot_token() {
        let token = generate_admin_token("secret", "bot");
        assert!(!verify_admin_token(&token, "other_bot", "secret"));
    }

    #[test]
    fn test_validate_telegram_id_param_ok() {
        assert!(validate_telegram_id_param(123456789).is_ok());
        assert!(validate_telegram_id_param(1).is_ok());
    }

    #[test]
    fn test_validate_telegram_id_param_zero() {
        assert_eq!(
            validate_telegram_id_param(0).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn test_validate_telegram_id_param_negative() {
        assert_eq!(
            validate_telegram_id_param(-1).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn test_validate_telegram_id_param_too_large() {
        assert_eq!(
            validate_telegram_id_param(9_007_199_254_740_992).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    // ── record_failed_admin_attempt (cycle #127) ─────────────────────
    //
    // The helper uses a global static (`ADMIN_AUTH_RATE_LIMIT`), which
    // tests *normally* would have to reset between runs. Instead each
    // test uses a UUID-derived "virtual IP" string in `x-forwarded-for`
    // so its rate-limit bucket is isolated by construction — no shared
    // state, no flakiness from test ordering.

    /// Build a HeaderMap whose client-IP-from-headers result is `ip`.
    fn headers_with_ip(ip: &str) -> HeaderMap {
        let mut h = HeaderMap::new();
        h.insert("x-forwarded-for", ip.parse().expect("valid header value"));
        h
    }

    #[test]
    fn record_failed_admin_attempt_allows_under_cap_then_blocks() {
        let ip = format!("test-127-{}", uuid::Uuid::new_v4());
        let headers = headers_with_ip(&ip);

        // First ADMIN_AUTH_RL_MAX_ATTEMPTS (10) calls must succeed.
        for i in 0..super::ADMIN_AUTH_RL_MAX_ATTEMPTS {
            assert!(
                super::record_failed_admin_attempt(&headers).is_ok(),
                "attempt {} should pass under the cap",
                i + 1
            );
        }

        // The (N+1)th must return 429.
        let result = super::record_failed_admin_attempt(&headers);
        assert_eq!(
            result.unwrap_err(),
            StatusCode::TOO_MANY_REQUESTS,
            "attempt past the cap must return 429"
        );
    }

    #[test]
    fn record_failed_admin_attempt_isolates_per_ip() {
        let ip_a = format!("test-127-{}", uuid::Uuid::new_v4());
        let ip_b = format!("test-127-{}", uuid::Uuid::new_v4());
        let headers_a = headers_with_ip(&ip_a);
        let headers_b = headers_with_ip(&ip_b);

        // Exhaust IP A.
        for _ in 0..super::ADMIN_AUTH_RL_MAX_ATTEMPTS {
            assert!(super::record_failed_admin_attempt(&headers_a).is_ok());
        }
        assert_eq!(
            super::record_failed_admin_attempt(&headers_a).unwrap_err(),
            StatusCode::TOO_MANY_REQUESTS
        );

        // IP B is independent — its first call must succeed.
        assert!(
            super::record_failed_admin_attempt(&headers_b).is_ok(),
            "IP B should not be affected by IP A's exhaustion"
        );
    }
}
