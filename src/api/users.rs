use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    routing::get,
    Json, Router,
};
use serde_json::{json, Value};

use crate::api::auth::{check_owner, validate_telegram_id_param};
use crate::AppState;

pub(crate) fn routes() -> Router<AppState> {
    Router::new().route("/users/me/:telegram_id", get(get_my_profile))
}

/// GET /api/users/me/:telegram_id
/// Owner-only: returns the authenticated user's id.
async fn get_my_profile(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(telegram_id): Path<i64>,
) -> Result<Json<Value>, StatusCode> {
    validate_telegram_id_param(telegram_id)?;
    check_owner(&headers, &state, telegram_id)?;

    Ok(Json(json!({
        "telegram_id": telegram_id,
    })))
}
