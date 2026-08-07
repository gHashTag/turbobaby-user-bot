//! Frontend error telemetry sink.
//!
//! `POST /api/client-errors` accepts sanitized JS/WASM runtime errors from
//! the Mini App, writes them to `client_error_logs`, and pages admins when
//! the error looks like a panic or a critical WASM load failure.
//!
//! Privacy guardrails:
//!   - The endpoint is deliberately unauthenticated so errors that happen
//!     before login can still be reported.
//!   - `telegram_id` is stored only if the client explicitly sends a numeric
//!     id in the JSON body; it is never derived from headers.
//!   - `message`/`stack`/`url_path` are truncated and stripped of Telegram
//!     initData-shaped tokens (`hash=...`, `user=...`).
//!   - Per-IP sliding-window rate-limit prevents a single client from
//!     flooding the table.

use axum::{
    extract::{Json, State},
    http::{HeaderMap, StatusCode},
    routing::post,
    Router,
};
use serde::Deserialize;
// Payload is deserialized by `Json<ClientErrorRequest>`; no manual Value needed.
// use serde_json::Value;
use std::time::{Duration, Instant};

use crate::api::rate_limit::{
    check_and_record, client_ip_from_headers, new_store, SlidingWindowStore,
};
use crate::AppState;

/// Per-IP rate limit: 10 error reports per minute is generous for legitimate
/// bursts (a page load throwing several errors) but stops a runaway loop
/// from writing millions of rows. `max_keys` bounds memory under a spray of
/// IPs to ~6 MB.
static CLIENT_ERROR_RATE_LIMIT: std::sync::LazyLock<SlidingWindowStore> =
    std::sync::LazyLock::new(new_store);
const CLIENT_ERROR_RL_WINDOW: Duration = Duration::from_secs(60);
const CLIENT_ERROR_RL_MAX_ATTEMPTS: usize = 10;
const CLIENT_ERROR_RL_MAX_IPS: usize = 10_000;

/// Global throttle for admin alerts: one panic-style alert per 5 minutes so
/// a widespread client bug does not spam every admin.
static LAST_CLIENT_ERROR_ALERT: std::sync::LazyLock<tokio::sync::Mutex<Option<Instant>>> =
    std::sync::LazyLock::new(|| tokio::sync::Mutex::new(None));
const ALERT_COOLDOWN: Duration = Duration::from_secs(300);

const MAX_MESSAGE_LEN: usize = 2000;
const MAX_STACK_LEN: usize = 1000;
const MAX_URL_PATH_LEN: usize = 500;
const MAX_USER_AGENT_LEN: usize = 500;

pub(crate) fn routes() -> Router<AppState> {
    Router::new()
        .route("/client-errors", post(log_client_error))
        .route("/client-events", post(log_client_event))
}

#[derive(Debug, Deserialize)]
pub(crate) struct ClientErrorRequest {
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub stack: Option<String>,
    #[serde(default)]
    pub url_path: Option<String>,
    #[serde(default)]
    pub user_agent: Option<String>,
    #[serde(default)]
    pub telegram_id: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ClientEventRequest {
    pub event: String,
    #[serde(default)]
    pub detail: Option<String>,
    #[serde(default)]
    pub url_path: Option<String>,
    #[serde(default)]
    pub telegram_id: Option<i64>,
}

/// Normalize source to one of the allowed enum values; unknown values
/// become `"unknown"` so a malicious client can't force arbitrary labels.
fn normalize_source(raw: Option<String>) -> String {
    match raw.as_deref() {
        Some("wasm") => "wasm".to_string(),
        Some("android") => "android".to_string(),
        Some("ios") => "ios".to_string(),
        _ => "unknown".to_string(),
    }
}

/// Remove Telegram initData-shaped tokens from free-text fields.
///
/// We don't use regex (not worth a dependency). The scan removes:
///   - `hash=<64 hex chars>` (with or without trailing `&`)
///   - `user=<url-encoded json blob>` up to the next `&` or end of string
///
/// This is defense-in-depth: the Mini App should never put initData into
/// error messages, but if it leaks through a URL or a logged object we
/// don't want it on disk.
pub(crate) fn sanitize_telemetry_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        // Try to strip `hash=` token.
        if let Some(strip_len) = strip_hash_token(bytes, i) {
            out.push_str("[hash_redacted]");
            i += strip_len;
            continue;
        }
        // Try to strip `user=` token.
        if let Some(strip_len) = strip_user_token(bytes, i) {
            out.push_str("[user_redacted]");
            i += strip_len;
            continue;
        }
        // Copy one UTF-8 character. ASCII fast path.
        if bytes[i].is_ascii() {
            out.push(bytes[i] as char);
            i += 1;
        } else {
            let start = i;
            i += 1;
            while i < bytes.len() && (bytes[i] & 0b1100_0000) == 0b1000_0000 {
                i += 1;
            }
            if let Ok(chunk) = std::str::from_utf8(&bytes[start..i]) {
                out.push_str(chunk);
            } else {
                // Invalid UTF-8: replace with lossy marker and keep moving.
                out.push('\u{FFFD}');
            }
        }
    }
    out
}

/// If `bytes[i..]` starts `hash=<64 hex>` (and optional `&`), return the
/// total byte length to skip. Telegram's WebApp hash is a 64-char hex HMAC.
fn strip_hash_token(bytes: &[u8], i: usize) -> Option<usize> {
    const PREFIX: &[u8] = b"hash=";
    if bytes.len() < i + PREFIX.len() + 64 {
        return None;
    }
    if &bytes[i..i + PREFIX.len()] != PREFIX {
        return None;
    }
    let start = i + PREFIX.len();
    let mut end = start;
    while end < bytes.len() && bytes[end].is_ascii_hexdigit() {
        end += 1;
    }
    if end - start != 64 {
        return None;
    }
    // Swallow a trailing `&` if present.
    if end < bytes.len() && bytes[end] == b'&' {
        end += 1;
    }
    Some(end - i)
}

/// If `bytes[i..]` starts `user=` followed by a value, return the length
/// to skip, including a trailing `&` if present.
fn strip_user_token(bytes: &[u8], i: usize) -> Option<usize> {
    const PREFIX: &[u8] = b"user=";
    if bytes.len() < i + PREFIX.len() + 1 {
        return None;
    }
    if &bytes[i..i + PREFIX.len()] != PREFIX {
        return None;
    }
    let start = i + PREFIX.len();
    let mut end = start;
    while end < bytes.len() && bytes[end] != b'&' {
        end += 1;
    }
    // Require the value to look like JSON-ish Telegram user data:
    // either it starts with `%7B` (encoded `{`) or a raw `{`.
    let value = &bytes[start..end];
    if !value.starts_with(b"%7B") && !value.starts_with(b"{") {
        return None;
    }
    if end < bytes.len() && bytes[end] == b'&' {
        end += 1;
    }
    Some(end - i)
}

/// Truncate to at most `max` Unicode scalar values.
fn truncate_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        s.chars().take(max).collect()
    }
}

/// Validate/normalize the incoming payload.
fn sanitize_request(req: ClientErrorRequest) -> Result<SanitizedError, StatusCode> {
    let source = normalize_source(req.source);
    let message = req
        .message
        .as_deref()
        .map(|s| sanitize_telemetry_text(s))
        .map(|s| truncate_chars(&s, MAX_MESSAGE_LEN))
        .unwrap_or_default();
    if message.trim().is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }
    let stack = req
        .stack
        .as_deref()
        .map(|s| sanitize_telemetry_text(s))
        .map(|s| truncate_chars(&s, MAX_STACK_LEN));
    let url_path = req
        .url_path
        .as_deref()
        .map(|s| sanitize_telemetry_text(s))
        .map(|s| truncate_chars(&s, MAX_URL_PATH_LEN));
    if let Some(ref p) = url_path {
        if p.len() > MAX_URL_PATH_LEN {
            return Err(StatusCode::BAD_REQUEST);
        }
    }
    let user_agent = req
        .user_agent
        .as_deref()
        .map(|s| sanitize_telemetry_text(s))
        .map(|s| truncate_chars(&s, MAX_USER_AGENT_LEN));
    let telegram_id = req.telegram_id.filter(|&id| id > 0);

    Ok(SanitizedError {
        source,
        message,
        stack,
        url_path,
        user_agent,
        telegram_id,
    })
}

#[derive(Debug)]
struct SanitizedError {
    source: String,
    message: String,
    stack: Option<String>,
    url_path: Option<String>,
    user_agent: Option<String>,
    telegram_id: Option<i64>,
}

/// Stable hash for grouping identical errors. SHA-256 of the sanitized
/// message; case-sensitive so "AbortError" and "abortError" don't collapse.
fn message_hash(message: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(message.as_bytes());
    hex::encode(hasher.finalize())
}

/// True when the error looks operationally important enough to page admins.
fn should_alert(source: &str, message: &str) -> bool {
    // Rust panic hook output starts with "PANIC:" (see lib.rs wasm start).
    // WASM `RuntimeError` / "unreachable" usually mean a Rust panic or a
    // wasm-bindgen mismatch. "Importing binding name" is the classic
    // stale-dist error after a trunk/wasm-bindgen version drift.
    let lower = message.to_ascii_lowercase();
    source == "window.onerror"
        && (message.starts_with("PANIC:")
            || lower.contains("runtimeerror")
            || lower.contains("unreachable")
            || lower.contains("importing binding name"))
}

async fn log_client_error(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(req): Json<ClientErrorRequest>,
) -> Result<StatusCode, StatusCode> {
    let ip = client_ip_from_headers(&headers);
    if !check_and_record(
        &CLIENT_ERROR_RATE_LIMIT,
        &ip,
        CLIENT_ERROR_RL_WINDOW,
        CLIENT_ERROR_RL_MAX_ATTEMPTS,
        CLIENT_ERROR_RL_MAX_IPS,
    )
    .await
    {
        crate::metrics::rate_limit_blocked("client_error");
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }

    let err = sanitize_request(req)?;
    let hash = message_hash(&err.message);

    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    state
        .db
        .orm
        .execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "INSERT INTO client_error_logs \
             (source, message_hash, message, stack, url_path, user_agent, telegram_id) \
             VALUES ($1,$2,$3,$4,$5,$6,$7)",
            [
                err.source.clone().into(),
                hash.clone().into(),
                err.message.clone().into(),
                err.stack.into(),
                err.url_path.clone().into(),
                err.user_agent.clone().into(),
                err.telegram_id.into(),
            ],
        ))
        .await
        .map_err(|e| {
            tracing::error!("client_error insert: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    crate::metrics::client_error_received(&err.source);

    if should_alert(&err.source, &err.message) {
        let should_page = {
            let mut last = LAST_CLIENT_ERROR_ALERT.lock().await;
            let now = Instant::now();
            if let Some(t) = *last {
                if now.saturating_duration_since(t) < ALERT_COOLDOWN {
                    false
                } else {
                    *last = Some(now);
                    true
                }
            } else {
                *last = Some(now);
                true
            }
        };
        if should_page {
            let bot = state.bot.clone();
            let config = state.config.clone();
            let safe_message = crate::util::html_escape(&err.message);
            let safe_path = err
                .url_path
                .as_deref()
                .map(crate::util::html_escape)
                .unwrap_or_else(|| "unknown".to_string());
            let safe_source = crate::util::html_escape(&err.source);
            let snippet: String = safe_message.chars().take(200).collect();
            tokio::spawn(async move {
                let text = format!(
                    "\u{1F6A8} Client panic / WASM crash\n\
                     \u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\n\
                     \u{1F4CD} {}\n\
                     \u{1F4E1} {}\n\
                     \u{1F4A3} {}",
                    safe_path, safe_source, snippet
                );
                crate::notify::notify_admins(&bot, &config, &text).await;
            });
        }
    }

    Ok(StatusCode::ACCEPTED)
}

/// Loop #13: lightweight conversion/event telemetry sink. Unlike
/// `/client-errors`, this endpoint does not require a `message` field; it
/// simply logs named client events (e.g. `cart_deep_link_opened`) for
/// funnel attribution. Rate-limited separately so normal analytics traffic
/// never competes with panic/error reports.
async fn log_client_event(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(req): Json<ClientEventRequest>,
) -> Result<StatusCode, StatusCode> {
    let ip = client_ip_from_headers(&headers);
    if !check_and_record(
        &CLIENT_ERROR_RATE_LIMIT,
        &ip,
        CLIENT_ERROR_RL_WINDOW,
        CLIENT_ERROR_RL_MAX_ATTEMPTS,
        CLIENT_ERROR_RL_MAX_IPS,
    )
    .await
    {
        crate::metrics::rate_limit_blocked("client_event");
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }

    let event = sanitize_event_name(&req.event);
    let detail = req
        .detail
        .as_deref()
        .map(sanitize_telemetry_text)
        .map(|s| truncate_chars(&s, MAX_MESSAGE_LEN))
        .unwrap_or_default();
    let url_path = req
        .url_path
        .as_deref()
        .map(sanitize_telemetry_text)
        .map(|s| truncate_chars(&s, MAX_URL_PATH_LEN));
    let telegram_id = req.telegram_id.filter(|&id| id > 0);

    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    state
        .db
        .orm
        .execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "INSERT INTO client_event_logs \
             (event, detail, url_path, telegram_id) \
             VALUES ($1,$2,$3,$4)",
            [
                event.clone().into(),
                detail.clone().into(),
                url_path.into(),
                telegram_id.into(),
            ],
        ))
        .await
        .map_err(|e| {
            tracing::error!("client_event insert: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    match event.as_str() {
        "cart_deep_link_opened" => crate::metrics::cart_deep_link_opened(&detail),
        "reorder_clicked" => crate::metrics::reorder_clicked(&detail),
        "garden_reminder_clicked" => crate::metrics::garden_reminder_clicked(&detail),
        "checkout_started" => crate::metrics::checkout_started(),
        "checkout_completed" => crate::metrics::checkout_completed(),
        "checkout_error" => crate::metrics::checkout_error(&detail),
        "bonus_applied" => {
            if let Ok(v) = detail.parse::<f64>() {
                crate::metrics::bonus_applied(v);
            }
        }
        "stars_applied" => {
            if let Ok(v) = detail.parse::<i64>() {
                crate::metrics::stars_applied(v);
            }
        }
        "garden_reward_applied" => {
            if let Ok(v) = detail.parse::<f64>() {
                crate::metrics::garden_reward_applied(v);
            }
        }
        _ => {}
    }

    Ok(StatusCode::ACCEPTED)
}

/// Normalize event name to an allow-list so a malicious client can't create
/// arbitrary time-series names.
fn sanitize_event_name(raw: &str) -> String {
    match raw {
        "cart_deep_link_opened" => "cart_deep_link_opened".to_string(),
        "reorder_clicked" => "reorder_clicked".to_string(),
        "garden_reminder_clicked" => "garden_reminder_clicked".to_string(),
        "checkout_retry_clicked" => "checkout_retry_clicked".to_string(),
        "checkout_started" => "checkout_started".to_string(),
        "checkout_completed" => "checkout_completed".to_string(),
        "checkout_error" => "checkout_error".to_string(),
        "bonus_applied" => "bonus_applied".to_string(),
        "stars_applied" => "stars_applied".to_string(),
        "garden_reward_applied" => "garden_reward_applied".to_string(),
        _ => "unknown".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_source_accepts_known_values() {
        assert_eq!(normalize_source(Some("wasm".into())), "wasm");
        assert_eq!(normalize_source(Some("android".into())), "android");
        assert_eq!(normalize_source(Some("ios".into())), "ios");
    }

    #[test]
    fn normalize_source_rejects_unknown_values() {
        assert_eq!(normalize_source(Some("evil".into())), "unknown");
        assert_eq!(normalize_source(None), "unknown");
    }

    #[test]
    fn sanitize_strips_telegram_hash_token() {
        let raw = "error in ?user=foo&hash=aabbccddeeff00112233445566778899aabbccddeeff00112233445566778899&auth_date=123";
        let clean = sanitize_telemetry_text(raw);
        assert!(!clean.contains("aabbccdd"));
        assert!(clean.contains("[hash_redacted]"));
        assert!(clean.contains("auth_date=123"));
    }

    #[test]
    fn sanitize_strips_telegram_user_token() {
        let raw = "user={\"id\":123,\"first_name\":\"A\"}&hash=abc";
        let clean = sanitize_telemetry_text(raw);
        assert!(clean.contains("[user_redacted]"));
        assert!(!clean.contains("\"id\":123"));
    }

    #[test]
    fn sanitize_strips_url_encoded_user_token() {
        let raw = "user=%7B%22id%22%3A123%7D&other=1";
        let clean = sanitize_telemetry_text(raw);
        assert!(clean.contains("[user_redacted]"));
        assert!(clean.contains("other=1"));
    }

    #[test]
    fn sanitize_keeps_ordinary_hash_word() {
        let raw = "hash table lookup failed";
        let clean = sanitize_telemetry_text(raw);
        assert_eq!(clean, raw);
    }

    #[test]
    fn sanitize_keeps_ordinary_user_word() {
        let raw = "user not found";
        let clean = sanitize_telemetry_text(raw);
        assert_eq!(clean, raw);
    }

    #[test]
    fn truncate_chars_respects_unicode_boundaries() {
        let s = "🔥".repeat(10); // 40 bytes
        assert_eq!(truncate_chars(&s, 5).chars().count(), 5);
    }

    #[test]
    fn sanitize_request_rejects_empty_message() {
        let req = ClientErrorRequest {
            source: None,
            message: Some("   ".into()),
            stack: None,
            url_path: None,
            user_agent: None,
            telegram_id: None,
        };
        assert_eq!(sanitize_request(req).unwrap_err(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn should_alert_detects_panic_prefix() {
        assert!(should_alert("window.onerror", "PANIC: oh no"));
        assert!(!should_alert("window.onerror", "network timeout"));
        assert!(!should_alert("unhandledrejection", "PANIC: ignored"));
    }

    #[test]
    fn should_alert_detects_runtime_error() {
        assert!(should_alert("window.onerror", "RuntimeError: unreachable"));
    }

    #[test]
    fn should_alert_detects_wasm_bindgen_mismatch() {
        assert!(should_alert(
            "window.onerror",
            "Importing binding name 'foo' not found"
        ));
    }

    #[test]
    fn message_hash_is_stable() {
        let h1 = message_hash("same");
        let h2 = message_hash("same");
        let h3 = message_hash("different");
        assert_eq!(h1, h2);
        assert_ne!(h1, h3);
    }
}
