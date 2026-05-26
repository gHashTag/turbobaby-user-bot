use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::AppState;
use crate::db::referrals::{
    find_referrer_by_code, get_or_create_referral_code, get_referrer_stats, get_top_referrers,
    record_referral,
};

// ──────────────────────────────────────────────────────────────────
// Route registration
// ──────────────────────────────────────────────────────────────────

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/referrals/me/:telegram_id", get(get_my_referrals))
        .route("/referrals/leaderboard", get(get_leaderboard))
        .route("/referrals/track", post(track_referral))
}

// ──────────────────────────────────────────────────────────────────
// Request / Response types
// ──────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct LeaderboardQuery {
    pub period: Option<String>,
    pub limit: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct TrackReferralRequest {
    pub code: String,
    pub referred_id: i64,
    pub source: Option<String>,
}

// ──────────────────────────────────────────────────────────────────
// Handlers
// ──────────────────────────────────────────────────────────────────

/// GET /api/referrals/me/:telegram_id
///
/// Returns the user's referral code, invite link, and statistics.
async fn get_my_referrals(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(telegram_id): Path<i64>,
) -> Result<Json<Value>, StatusCode> {
    crate::api::auth::check_owner(&headers, &state, telegram_id)?;
    let code = get_or_create_referral_code(&state.db.pool, telegram_id)
        .await
        .map_err(|e| { tracing::error!("DB error: {:?}", e); StatusCode::INTERNAL_SERVER_ERROR })?;

    let stats = get_referrer_stats(&state.db.pool, telegram_id)
        .await
        .map_err(|e| { tracing::error!("DB error: {:?}", e); StatusCode::INTERNAL_SERVER_ERROR })?;

    let bot_username = &state.config.bot_username;
    let invite_link = format!("https://t.me/{}?start=ref_{}", bot_username, code);

    Ok(Json(json!({
        "code": code,
        "invite_link": invite_link,
        "stats": {
            "total_invited": stats.total_invited,
            "confirmed": stats.confirmed,
            "pending": stats.pending,
            "total_bonus_earned": stats.total_bonus_earned,
        }
    })))
}

/// GET /api/referrals/leaderboard?period=weekly|monthly|all&limit=10
async fn get_leaderboard(
    State(state): State<AppState>,
    Query(params): Query<LeaderboardQuery>,
) -> Result<Json<Value>, StatusCode> {
    let period = params.period.as_deref().unwrap_or("all");
    let limit = params.limit.unwrap_or(10).min(50);

    let top = get_top_referrers(&state.db.pool, period, limit)
        .await
        .map_err(|e| { tracing::error!("DB error: {:?}", e); StatusCode::INTERNAL_SERVER_ERROR })?;

    Ok(Json(json!({
        "period": period,
        "leaderboard": top,
    })))
}

/// POST /api/referrals/track
///
/// Internal endpoint called from the bot /start handler when a user arrives
/// via a referral link. Idempotent — silently returns ok if already tracked.
async fn track_referral(
    State(state): State<AppState>,
    Json(req): Json<TrackReferralRequest>,
) -> Result<Json<Value>, StatusCode> {
    // Look up referrer
    let referrer_id = find_referrer_by_code(&state.db.pool, &req.code)
        .await
        .map_err(|e| { tracing::error!("DB error: {:?}", e); StatusCode::INTERNAL_SERVER_ERROR })?;

    let referrer_id = match referrer_id {
        Some(id) => id,
        None => {
            return Ok(Json(json!({
                "success": false,
                "reason": "unknown_code"
            })))
        }
    };

    // Self-referral guard
    if referrer_id == req.referred_id {
        return Ok(Json(json!({
            "success": false,
            "reason": "self_referral"
        })));
    }

    let event_id = record_referral(
        &state.db.pool,
        referrer_id,
        req.referred_id,
        &req.code,
        req.source.as_deref(),
    )
    .await
    .map_err(|e| { tracing::error!("DB error: {:?}", e); StatusCode::INTERNAL_SERVER_ERROR })?;

    Ok(Json(json!({
        "success": true,
        "event_id": event_id,
        "referrer_id": referrer_id,
    })))
}
