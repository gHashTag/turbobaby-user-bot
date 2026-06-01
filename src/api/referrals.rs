use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::get,
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::api::auth::{check_not_blocked, validate_telegram_id_param};
use crate::db::referrals::{get_or_create_referral_code, get_referrer_stats, get_top_referrers};
use crate::AppState;

// ──────────────────────────────────────────────────────────────────
// Route registration
// ──────────────────────────────────────────────────────────────────

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/referrals/me/:telegram_id", get(get_my_referrals))
        .route("/referrals/leaderboard", get(get_leaderboard))
}

// ──────────────────────────────────────────────────────────────────
// Request / Response types
// ──────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct LeaderboardQuery {
    pub period: Option<String>,
    pub limit: Option<i64>,
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
    validate_telegram_id_param(telegram_id)?;
    crate::api::auth::check_owner(&headers, &state, telegram_id)?;
    check_not_blocked(&state, telegram_id).await?;
    let code = get_or_create_referral_code(&state.db.orm, telegram_id)
        .await
        .map_err(|e| {
            tracing::error!("DB error: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let stats = get_referrer_stats(&state.db.orm, telegram_id)
        .await
        .map_err(|e| {
            tracing::error!("DB error: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

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

fn validate_leaderboard_query(params: &LeaderboardQuery) -> Result<(&str, i64), StatusCode> {
    let period = params.period.as_deref().unwrap_or("all");
    if !matches!(period, "weekly" | "monthly" | "all") {
        return Err(StatusCode::BAD_REQUEST);
    }
    let limit = params.limit.unwrap_or(10).min(50);
    Ok((period, limit))
}

/// GET /api/referrals/leaderboard?period=weekly|monthly|all&limit=10
async fn get_leaderboard(
    State(state): State<AppState>,
    Query(params): Query<LeaderboardQuery>,
) -> Result<Json<Value>, StatusCode> {
    let (period, limit) = validate_leaderboard_query(&params)?;

    let top = get_top_referrers(&state.db.orm, period, limit)
        .await
        .map_err(|e| {
            tracing::error!("DB error: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(json!({
        "period": period,
        "leaderboard": top,
    })))
}

#[cfg(test)]
mod tests {
    use super::{validate_leaderboard_query, LeaderboardQuery};
    use axum::http::StatusCode;

    #[test]
    fn test_validate_leaderboard_defaults() {
        let q = LeaderboardQuery {
            period: None,
            limit: None,
        };
        assert_eq!(validate_leaderboard_query(&q).unwrap(), ("all", 10));
    }

    #[test]
    fn test_validate_leaderboard_weekly() {
        let q = LeaderboardQuery {
            period: Some("weekly".into()),
            limit: Some(20),
        };
        assert_eq!(validate_leaderboard_query(&q).unwrap(), ("weekly", 20));
    }

    #[test]
    fn test_validate_leaderboard_limit_capped() {
        let q = LeaderboardQuery {
            period: None,
            limit: Some(100),
        };
        assert_eq!(validate_leaderboard_query(&q).unwrap(), ("all", 50));
    }

    #[test]
    fn test_validate_leaderboard_invalid_period() {
        let q = LeaderboardQuery {
            period: Some("daily".into()),
            limit: None,
        };
        assert_eq!(
            validate_leaderboard_query(&q).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }
}
