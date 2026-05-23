// Admin auth helper.
//
// Validates Telegram WebApp initData HMAC signature using BOT_TOKEN.
// The frontend sends the raw `Telegram.WebApp.initData` string in the
// `X-Telegram-Init-Data` header. The server verifies the HMAC, extracts
// the user id, and checks it against admin_ids.
//
// Fallback: if initData is missing/invalid but `X-Admin-Telegram-Id` is
// present and the request is from localhost (debug), we accept it. This
// allows local development without Telegram.

use axum::http::{HeaderMap, StatusCode};
use hmac::{Hmac, Mac};
use sha2::Sha256;

use crate::AppState;

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug)]
pub struct TelegramUser {
    pub id: i64,
    pub first_name: Option<String>,
    pub username: Option<String>,
}

/// Validate Telegram WebApp initData HMAC signature.
///
/// Algorithm (per Telegram docs):
/// 1. Parse URL-encoded key=value pairs.
/// 2. Remove `hash` key.
/// 3. Sort remaining keys alphabetically.
/// 4. Build data_check_string = "key1=value1\nkey2=value2\n..."
/// 5. secret_key = HMAC_SHA256(key="WebAppData", msg=BOT_TOKEN)
/// 6. expected_hash = HMAC_SHA256(key=secret_key, msg=data_check_string) in hex
/// 7. Compare expected_hash with received `hash` (constant-time)
pub fn validate_init_data(init_data: &str, bot_token: &str) -> Option<TelegramUser> {
    tracing::info!("validate_init_data: len={}, hash_present={}, preview={}", init_data.len(), init_data.contains("hash="), &init_data[..init_data.len().min(100)]);
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
        .filter(|(k, _)| k != "hash")
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
        tracing::warn!("initData HMAC mismatch (decoded={}, raw={})", ok_decoded, ok_raw);
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
    let first_name = user.get("first_name").and_then(|v| v.as_str()).map(|s| s.to_string());
    let username = user.get("username").and_then(|v| v.as_str()).map(|s| s.to_string());

    Some(TelegramUser { id, first_name, username })
}

/// Debug version that returns detailed validation info instead of just Option.
pub fn validate_init_data_debug(init_data: &str, bot_token: &str) -> (bool, String, String, String, Option<TelegramUser>, Option<String>) {
    let mut pairs: Vec<(String, String)> = Vec::new();
    for pair in init_data.split('&') {
        let mut parts = pair.splitn(2, '=');
        let key = match parts.next() {
            Some(k) => k,
            None => return (false, String::new(), String::new(), String::new(), None, Some("empty pair".to_string())),
        };
        let value = parts.next().unwrap_or("");
        pairs.push((key.to_string(), value.to_string()));
    }

    let hash = match pairs.iter().find(|(k, _)| k == "hash").map(|(_, v)| v.clone()) {
        Some(h) => h,
        None => return (false, String::new(), String::new(), String::new(), None, Some("missing hash".to_string())),
    };

    let mut data_pairs: Vec<_> = pairs.into_iter().filter(|(k, _)| k != "hash").collect();
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

    let mut secret_mac = match HmacSha256::new_from_slice(b"WebAppData") {
        Ok(m) => m,
        Err(_) => return (false, data_check_string_decoded.clone(), hash.clone(), String::new(), None, Some("HMAC init failed".to_string())),
    };
    secret_mac.update(bot_token.as_bytes());
    let secret_key = secret_mac.finalize().into_bytes();

    let mut mac = match HmacSha256::new_from_slice(&secret_key) {
        Ok(m) => m,
        Err(_) => return (false, data_check_string_decoded.clone(), hash.clone(), String::new(), None, Some("HMAC init failed".to_string())),
    };
    mac.update(data_check_string_decoded.as_bytes());
    let expected_hash = hex::encode(mac.finalize().into_bytes());

    let mut mac_raw = match HmacSha256::new_from_slice(&secret_key) {
        Ok(m) => m,
        Err(_) => return (false, data_check_string_raw.clone(), hash.clone(), String::new(), None, Some("HMAC init failed".to_string())),
    };
    mac_raw.update(data_check_string_raw.as_bytes());
    let expected_hash_raw = hex::encode(mac_raw.finalize().into_bytes());

    let user = data_pairs.iter().find(|(k, _)| k == "user").and_then(|(_, v)| {
        let decoded = urlencoding::decode(v).ok()?;
        let user: serde_json::Value = serde_json::from_str(&decoded).ok()?;
        let id = user.get("id")?.as_i64()?;
        let first_name = user.get("first_name").and_then(|v| v.as_str()).map(|s| s.to_string());
        let username = user.get("username").and_then(|v| v.as_str()).map(|s| s.to_string());
        Some(TelegramUser { id, first_name, username })
    });

    let ok_decoded = constant_time_eq::constant_time_eq(expected_hash.as_bytes(), hash.as_bytes());
    let ok_raw = constant_time_eq::constant_time_eq(expected_hash_raw.as_bytes(), hash.as_bytes());
    let ok = ok_decoded || ok_raw;
    (ok, data_check_string_decoded, hash, expected_hash, user, None)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn generate_init_data(bot_token: &str, user_id: i64, first_name: &str) -> String {
        let user_json = format!(
            "{{\"id\":{},\"first_name\":\"{}\"}}",
            user_id, first_name
        );
        let user_json_for_encode = user_json.clone();
        let user_encoded = urlencoding::encode(&user_json_for_encode);
        // Compute hash over DECODED values (matches real Telegram behavior)
        let mut pairs = vec![
            ("auth_date".to_string(), "1234567890".to_string()),
            ("user".to_string(), user_json),
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

        // Emit the URL-encoded user value in the query string
        format!(
            "auth_date=1234567890&hash={}&user={}",
            hash, user_encoded
        )
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
}

pub fn generate_admin_token(password: &str, bot_token: &str) -> String {
    let mut mac = HmacSha256::new_from_slice(b"WoodyWeedBotAdmin").unwrap();
    mac.update(bot_token.as_bytes());
    mac.update(b":");
    mac.update(password.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

pub fn verify_admin_token(token: &str, bot_token: &str, expected_password: &str) -> bool {
    let expected = generate_admin_token(expected_password, bot_token);
    constant_time_eq::constant_time_eq(token.as_bytes(), expected.as_bytes())
}

pub fn check_admin(headers: &HeaderMap, state: &AppState) -> Result<i64, StatusCode> {
    // 1. Try Telegram initData validation (production path)
    if let Some(init_data) = headers
        .get("X-Telegram-Init-Data")
        .and_then(|v| v.to_str().ok())
    {
        if !init_data.is_empty() {
            if let Some(user) = validate_init_data(init_data, &state.config.bot_token) {
                if state.config.admin_ids.contains(&user.id) {
                    tracing::info!(
                        "admin authenticated via initData telegram_id={} username={:?}",
                        user.id,
                        user.username
                    );
                    return Ok(user.id);
                } else {
                    tracing::warn!(
                        "initData valid but user not admin telegram_id={}",
                        user.id
                    );
                    return Err(StatusCode::FORBIDDEN);
                }
            } else {
                tracing::warn!("invalid initData signature");
                return Err(StatusCode::UNAUTHORIZED);
            }
        }
    }

    // 2. Password login via X-Admin-Token (works in all builds if ADMIN_PASSWORD is set)
    if let Some(token) = headers.get("X-Admin-Token").and_then(|v| v.to_str().ok()) {
        if let Some(ref password) = state.config.admin_password {
            if verify_admin_token(token, &state.config.bot_token, password) {
                tracing::info!("admin authenticated via password token");
                return Ok(0);
            }
        }
    }

    // 3. Fallback for local development (debug builds only): trust X-Admin-Telegram-Id header
    //    NEVER allow this in production — it is a full auth bypass vector.
    #[cfg(debug_assertions)]
    {
        let id = headers
            .get("X-Admin-Telegram-Id")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<i64>().ok())
            .ok_or_else(|| {
                tracing::warn!("admin request without valid auth (initData, token, or fallback)");
                StatusCode::UNAUTHORIZED
            })?;

        if !state.config.admin_ids.contains(&id) {
            tracing::warn!("admin request from non-admin telegram_id={}", id);
            return Err(StatusCode::FORBIDDEN);
        }
        tracing::info!("admin authenticated via fallback header telegram_id={}", id);
        Ok(id)
    }
    #[cfg(not(debug_assertions))]
    {
        tracing::warn!("admin request without valid auth (initData or token)");
        Err(StatusCode::UNAUTHORIZED)
    }
}
