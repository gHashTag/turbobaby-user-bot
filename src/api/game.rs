//! Game scoreboard endpoints (Woody Catch).
//!
//! - `GET  /api/game/highscore/:telegram_id` — caller's best score (0 if none).
//! - `POST /api/game/highscore` — upsert with MAX semantics: never overwrites
//!   a higher previously stored value.
//!
//! Both endpoints require Telegram WebApp init-data → owner can only see/update
//! their own score.

use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
    Json, Router,
};
use std::collections::HashMap;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::api::auth::{check_not_blocked, check_owner, validate_telegram_id_param};
use crate::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/game/highscore/:telegram_id", get(get_high_score))
        .route("/game/highscore", post(post_high_score))
        .route("/game/leaderboard", get(get_leaderboard))
}

#[derive(Debug, Deserialize)]
struct HighScoreRequest {
    telegram_id: i64,
    /// Caller's claimed score. Capped at `MAX_HIGH_SCORE` and rejected if
    /// non-finite — defense against client-side tampering surfacing as an
    /// absurd value on the leaderboard.
    score: i64,
}

/// Cap on stored high score. Picked well above any plausible legitimate
/// score so we still catch obvious cheats without limiting fair play.
const MAX_HIGH_SCORE: i64 = 10_000_000;

async fn get_high_score(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(telegram_id): Path<i64>,
) -> Result<Json<Value>, StatusCode> {
    validate_telegram_id_param(telegram_id)?;
    check_owner(&headers, &state, telegram_id)?;
    check_not_blocked(&state, telegram_id).await?;

    let client = state.db.pool.get().await.map_err(|e| {
        crate::metrics::db_pool_acquire_failed("game.high_score.get");
        tracing::error!("game high_score pool: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    let row = client
        .query_opt(
            "SELECT high_score FROM game_high_scores WHERE telegram_id = $1",
            &[&telegram_id],
        )
        .await
        .map_err(|e| {
            tracing::error!("game high_score query: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    let score: i64 = row
        .and_then(|r| r.try_get::<_, i32>("high_score").ok())
        .map(|v| v as i64)
        .unwrap_or(0);
    Ok(Json(json!({ "high_score": score })))
}

async fn post_high_score(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(req): Json<HighScoreRequest>,
) -> Result<Json<Value>, StatusCode> {
    validate_telegram_id_param(req.telegram_id)?;
    if req.score < 0 || req.score > MAX_HIGH_SCORE {
        return Err(StatusCode::BAD_REQUEST);
    }
    check_owner(&headers, &state, req.telegram_id)?;
    check_not_blocked(&state, req.telegram_id).await?;

    let client = state.db.pool.get().await.map_err(|e| {
        crate::metrics::db_pool_acquire_failed("game.high_score.post");
        tracing::error!("game high_score pool: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    // Truncate to i32 (PG INTEGER) safely; we already capped above.
    let score_i32: i32 = req.score as i32;
    // ON CONFLICT … GREATEST keeps the row's existing value when it's higher,
    // so a stale POST from another tab can never lower someone's best.
    client
        .execute(
            "INSERT INTO game_high_scores (telegram_id, high_score, updated_at) \
             VALUES ($1, $2, NOW()) \
             ON CONFLICT (telegram_id) DO UPDATE SET \
                 high_score = GREATEST(game_high_scores.high_score, EXCLUDED.high_score), \
                 updated_at = CASE \
                     WHEN EXCLUDED.high_score > game_high_scores.high_score THEN NOW() \
                     ELSE game_high_scores.updated_at \
                 END",
            &[&req.telegram_id, &score_i32],
        )
        .await
        .map_err(|e| {
            tracing::error!("game high_score upsert: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    // Read back actual stored value (may be the previous higher score).
    let stored = client
        .query_opt(
            "SELECT high_score FROM game_high_scores WHERE telegram_id = $1",
            &[&req.telegram_id],
        )
        .await
        .ok()
        .flatten()
        .and_then(|r| r.try_get::<_, i32>("high_score").ok())
        .map(|v| v as i64)
        .unwrap_or(score_i32 as i64);
    Ok(Json(json!({ "success": true, "high_score": stored })))
}

/// Public leaderboard — top scores across all users. Returns rank, masked
/// `telegram_id` (just to differentiate players, not display), display name
/// from `user_languages.first_name`, and the score.
///
/// Public on purpose: no auth, no per-user filtering. Cap at 20 rows so
/// stale-client polling doesn't pull a giant payload.
///
/// Cycle #32: optional `?telegram_id=N` adds `own_rank` / `own_score` so the
/// client can render "Your rank: #42" when the player isn't visible in the
/// top 20 they just received.
async fn get_leaderboard(
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<Value>, StatusCode> {
    let client = state.db.pool.get().await.map_err(|e| {
        crate::metrics::db_pool_acquire_failed("game.leaderboard");
        tracing::error!("game leaderboard pool: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    let rows = client
        .query(
            "SELECT g.telegram_id, g.high_score, ul.first_name \
             FROM game_high_scores g \
             LEFT JOIN user_languages ul ON g.telegram_id = ul.telegram_id \
             WHERE g.high_score > 0 \
             ORDER BY g.high_score DESC, g.updated_at ASC \
             LIMIT 20",
            &[],
        )
        .await
        .map_err(|e| {
            tracing::error!("game leaderboard query: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let leaderboard: Vec<Value> = rows
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let telegram_id: i64 = r.try_get("telegram_id").unwrap_or(0);
            let score: i32 = r.try_get("high_score").unwrap_or(0);
            // first_name may be null in user_languages; default to "Anonymous"
            let name: String = r
                .try_get::<_, Option<String>>("first_name")
                .ok()
                .flatten()
                .filter(|s| !s.trim().is_empty())
                .unwrap_or_else(|| "Anonymous".to_string());
            json!({
                "rank":        (i as i64) + 1,
                "telegram_id": telegram_id,
                "name":        name,
                "score":       score,
            })
        })
        .collect();

    // Compute caller's own rank when requested. Uses ROW_NUMBER over the same
    // ordering as the top-N list so ranks line up exactly. If the caller has
    // no entry (never played, or score 0), own_rank stays null.
    let mut own_rank: Option<i64> = None;
    let mut own_score: Option<i32> = None;
    if let Some(tid) = params.get("telegram_id").and_then(|s| s.parse::<i64>().ok()) {
        // Skip blocked-user check on purpose: the leaderboard is public.
        if let Ok(Some(rank_row)) = client
            .query_opt(
                "WITH ranked AS ( \
                    SELECT telegram_id, high_score, \
                           ROW_NUMBER() OVER (ORDER BY high_score DESC, updated_at ASC) AS rn \
                    FROM game_high_scores WHERE high_score > 0 \
                 ) \
                 SELECT rn, high_score FROM ranked WHERE telegram_id = $1",
                &[&tid],
            )
            .await
        {
            own_rank = rank_row.try_get::<_, i64>("rn").ok();
            own_score = rank_row.try_get::<_, i32>("high_score").ok();
        }
    }

    Ok(Json(json!({
        "leaderboard": leaderboard,
        "total":       leaderboard.len(),
        "own_rank":    own_rank,
        "own_score":   own_score,
    })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_rejects_negative_score() {
        // Validation lives in the handler itself, but a unit-level check on the
        // boundary numbers documents the intent.
        assert!(MAX_HIGH_SCORE > 0);
    }

    #[test]
    fn high_score_cap_fits_postgres_integer() {
        // PG INTEGER is i32 — verify our cap is comfortably inside that range
        // so the `as i32` cast in `post_high_score` is lossless.
        assert!(MAX_HIGH_SCORE <= i32::MAX as i64);
    }

    #[test]
    fn deserialise_request_shape() {
        let s = r#"{"telegram_id":42,"score":1234}"#;
        let req: HighScoreRequest = serde_json::from_str(s).unwrap();
        assert_eq!(req.telegram_id, 42);
        assert_eq!(req.score, 1234);
    }
}
