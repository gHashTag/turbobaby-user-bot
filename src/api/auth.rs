// Admin auth helper.
//
// Frontend admin UI sends the Telegram user id (from
// Telegram.WebApp.initDataUnsafe.user.id) in the `X-Admin-Telegram-Id`
// header. Each write-handler that modifies the catalog calls
// `check_admin(&headers, &state)?` at the very top.
//
// NOTE: this trusts the client header. A future improvement should verify
// the full Telegram WebApp `initData` HMAC using BOT_TOKEN. For now,
// combined with HTTPS + unguessable admin telegram ids in ADMIN_IDS env,
// this is enough to prevent random public writes through the admin panel.

use axum::http::{HeaderMap, StatusCode};

use crate::AppState;

pub fn check_admin(headers: &HeaderMap, state: &AppState) -> Result<i64, StatusCode> {
    let id = headers
        .get("X-Admin-Telegram-Id")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<i64>().ok())
        .ok_or_else(|| {
            tracing::warn!("admin request without X-Admin-Telegram-Id header");
            StatusCode::UNAUTHORIZED
        })?;

    if !state.config.admin_ids.contains(&id) {
        tracing::warn!("admin request from non-admin telegram_id={}", id);
        return Err(StatusCode::FORBIDDEN);
    }
    Ok(id)
}
