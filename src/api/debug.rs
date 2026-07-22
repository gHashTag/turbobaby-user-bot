//! Debug helpers for Telegram WebApp initData validation.
//!
//! These endpoints exist ONLY to diagnose Mini App auth problems in
//! production without exposing secrets. They accept the same
//! `X-Telegram-Init-Data` header the rest of the API uses and return
//! structured validation diagnostics.

use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::Json,
    routing::get,
    Router,
};
use serde_json::{json, Value};

use crate::api::auth::validate_init_data_debug;
use crate::AppState;

pub(crate) fn routes() -> Router<AppState> {
    Router::new().route("/debug/validate-init-data", get(validate_init_data_handler))
}

/// Return detailed initData validation info for the header the client sent.
/// Does NOT echo the full initData back (it contains user info); only
/// returns length, whether a hash is present, user_id if validation succeeds,
/// and a machine-readable `reason` when validation fails.
async fn validate_init_data_handler(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<Value>, StatusCode> {
    let init_data = headers
        .get("X-Telegram-Init-Data")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();

    let len = init_data.len();
    let has_hash = init_data.contains("hash=");
    let has_user = init_data.contains("user=");
    let has_auth_date = init_data.contains("auth_date=");

    let info = validate_init_data_debug(&init_data, &state.config.bot_token);

    Ok(Json(json!({
        "valid": info.ok,
        "init_data_len": len,
        "has_hash": has_hash,
        "has_user": has_user,
        "has_auth_date": has_auth_date,
        "user_id": info.user.as_ref().map(|u| u.id),
        "keys": info.keys,
        "reason": info.error,
        "hash": info.hash,
        "expected_hash_decoded": info.expected_hash_decoded,
        "expected_hash_raw": info.expected_hash_raw,
        "expected_hash_with_signature": info.expected_hash_with_signature,
        "data_check_string_decoded": info.data_check_string_decoded,
        "data_check_string_raw": info.data_check_string_raw,
        "data_check_string_with_signature": info.data_check_string_with_signature,
        "ok_with_signature": constant_time_eq::constant_time_eq(
            info.expected_hash_with_signature.as_bytes(),
            info.hash.as_bytes()
        ),
    })))
}
