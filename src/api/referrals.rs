use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::get,
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::api::auth::{check_not_blocked, validate_telegram_id_param};
use crate::db::referrals::{
    get_invitees, get_or_create_referral_code, get_referral_milestones, get_referrer_stats,
    get_top_referrers,
};
use crate::trios::referrals::assign_share_source;
use crate::AppState;

// ──────────────────────────────────────────────────────────────────
// Route registration
// ──────────────────────────────────────────────────────────────────

pub(crate) fn routes() -> Router<AppState> {
    Router::new()
        .route("/referrals/me/:telegram_id", get(get_my_referrals))
        .route(
            "/referrals/me/:telegram_id/share-source",
            get(get_share_source),
        )
        .route("/referrals/me/:telegram_id/invitees", get(get_my_invitees))
        .route(
            "/referrals/me/:telegram_id/milestones",
            get(get_my_milestones),
        )
        // `POST /referrals/me/:id/garden-invite` stood here. It took a raw
        // `referrer_id` from the request body and recorded a referral against
        // it — a second invite path that only the garden screen ever called
        // (D5). The canonical one is `/start ref_<code>`, handled by the bot
        // against an opaque code that identifies nobody. Removing this closes
        // the only route where an inviter was named by a number a caller could
        // type; nothing is lost, because the link a customer shares has always
        // carried the code, not the id.
        .route("/referrals/leaderboard", get(get_leaderboard))
}

// ──────────────────────────────────────────────────────────────────
// Request / Response types
// ──────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub(crate) struct LeaderboardQuery {
    pub period: Option<String>,
    pub limit: Option<i64>,
}

// ──────────────────────────────────────────────────────────────────
// Handlers
// ──────────────────────────────────────────────────────────────────

/// GET /api/referrals/me/:telegram_id/share-source
///
/// Loop #20: returns the deterministic A/B share source assigned to this user.
async fn get_share_source(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(telegram_id): Path<i64>,
) -> Result<Json<Value>, StatusCode> {
    validate_telegram_id_param(telegram_id)?;
    crate::api::auth::check_owner(&headers, &state, telegram_id)?;
    check_not_blocked(&state, telegram_id).await?;

    let source = assign_share_source(telegram_id, &state.config.referral_share_sources);
    crate::metrics::share_source_assigned(&source);
    Ok(Json(json!({"source": source})))
}

/// GET /api/referrals/me/:telegram_id/invitees
///
/// Loop #20: list the people this user referred, with their order status so
/// the inviter sees social proof. The invites were sent from the garden screen
/// once; the events they wrote are ordinary `referral_events` rows and outlive
/// it (D5).
async fn get_my_invitees(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(telegram_id): Path<i64>,
) -> Result<Json<Value>, StatusCode> {
    validate_telegram_id_param(telegram_id)?;
    crate::api::auth::check_owner(&headers, &state, telegram_id)?;
    check_not_blocked(&state, telegram_id).await?;

    let invitees = get_invitees(&state.db.orm, telegram_id)
        .await
        .map_err(|e| {
            tracing::error!("DB error get_invitees: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    crate::metrics::referral_invitees_viewed();
    Ok(Json(json!({
        "count": invitees.len(),
        "invitees": invitees,
    })))
}

/// GET /api/referrals/me/:telegram_id/milestones
///
/// Loop #21: returns the referral milestone bonuses already awarded to this
/// user plus the count of confirmed referrals, so the UI can show progress
/// toward the next threshold.
///
/// The response carries two kinds of money and they are not the same number.
/// `awards` is what each reached milestone actually paid, read from its row.
/// `bonuses` is what an unreached rung pays today, read from `loyalty_config`.
/// The shop may edit the config at any time, so a rung reached in June and the
/// same rung offered now can differ — the client must print the server's
/// numbers rather than a table of its own, which is exactly the mistake the
/// removed garden screen made.
async fn get_my_milestones(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(telegram_id): Path<i64>,
) -> Result<Json<Value>, StatusCode> {
    validate_telegram_id_param(telegram_id)?;
    crate::api::auth::check_owner(&headers, &state, telegram_id)?;
    check_not_blocked(&state, telegram_id).await?;

    let (achieved, confirmed) = get_referral_milestones(&state.db.orm, telegram_id)
        .await
        .map_err(|e| {
            tracing::error!("DB error get_referral_milestones: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let bonuses = crate::db::referrals::milestone_bonus_amounts(&state.db.orm).await;

    Ok(Json(json!({
        "confirmed": confirmed,
        // Kept as a bare list of numbers: shipped clients read it.
        "achieved": achieved.iter().map(|(m, _)| *m).collect::<Vec<i32>>(),
        "awards": achieved
            .iter()
            .map(|(m, amount)| json!({ "milestone": m, "bonus_amount": amount }))
            .collect::<Vec<Value>>(),
        "thresholds": crate::db::referrals::MILESTONE_THRESHOLDS,
        "bonuses": crate::db::referrals::MILESTONE_THRESHOLDS
            .iter()
            .map(|m| json!({
                "milestone": m,
                "bonus_amount": bonuses.get(m).copied().unwrap_or(0.0),
            }))
            .collect::<Vec<Value>>(),
    })))
}

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
    // Cycle #156: `limit` was previously `.min(50)` only — capped
    // from above but not from below. A negative value (e.g. `?limit=-1`)
    // passed through to PostgreSQL's `LIMIT $1`, which rejects with
    // `ERROR: LIMIT must not be negative` → 500 instead of a clean
    // 400 at the boundary. Mirror the existing silent-cap convention
    // for the lower bound: clamp into `[1, 50]`. 0 is also silently
    // clamped to 1 — returning zero rows for a leaderboard request
    // is more confusing than returning at least one entry.
    let limit = params.limit.unwrap_or(10).clamp(1, 50);
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

    // ── cycle #156 lower-bound clamp ─────────────────────────────────

    #[test]
    fn test_validate_leaderboard_negative_limit_clamped_to_one() {
        // Pre-cycle #156 this would pass through to `LIMIT -1`, which
        // PostgreSQL rejects with a 500 — the validator now clamps.
        let q = LeaderboardQuery {
            period: None,
            limit: Some(-1),
        };
        assert_eq!(validate_leaderboard_query(&q).unwrap(), ("all", 1));
    }

    #[test]
    fn test_validate_leaderboard_zero_limit_clamped_to_one() {
        let q = LeaderboardQuery {
            period: None,
            limit: Some(0),
        };
        assert_eq!(validate_leaderboard_query(&q).unwrap(), ("all", 1));
    }

    #[test]
    fn test_validate_leaderboard_i64_min_does_not_panic() {
        // `.clamp(1, 50)` on `i64::MIN` must not panic — domain-bound
        // input from an adversarial client shouldn't crash the handler.
        let q = LeaderboardQuery {
            period: None,
            limit: Some(i64::MIN),
        };
        assert_eq!(validate_leaderboard_query(&q).unwrap(), ("all", 1));
    }
}
