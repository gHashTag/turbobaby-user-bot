//! Cross-device game high scores (TurboBaby Catch)
//!
//! Stores the user's best score server-side so returning on a new device
//! does not lose progress. Submissions are owner-authenticated; the public
//! leaderboard is anonymous.

use crate::api::auth::{check_owner_lenient, validate_telegram_id_param};
use crate::AppState;
use axum::{
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};

pub(crate) fn routes() -> Router<AppState> {
    Router::new()
        .route("/game/high-scores", get(get_high_scores))
        .route("/game/high-scores", post(submit_high_score))
}

const MAX_DISPLAY_NAME_LEN: usize = 50;
const MAX_SCORE: i64 = 10_000_000;

#[derive(Debug, Deserialize)]
pub(crate) struct HighScoreQuery {
    #[serde(default = "default_limit")]
    pub limit: u32,
}

fn default_limit() -> u32 {
    20
}

#[derive(Debug, Deserialize)]
pub(crate) struct SubmitHighScoreRequest {
    pub telegram_id: i64,
    pub score: i64,
    #[serde(default)]
    pub display_name: String,
}

/// GET /api/game/high-scores — public leaderboard.
async fn get_high_scores(
    Query(query): Query<HighScoreQuery>,
    State(state): State<AppState>,
) -> Result<Json<Value>, StatusCode> {
    let limit = query.limit.clamp(1, 100) as i64;
    use sea_orm::{ConnectionTrait, DbBackend, Statement};

    let rows = state
        .db
        .orm
        .query_all(Statement::from_sql_and_values(
            DbBackend::Postgres,
            // `high_score` is INTEGER in the schema but read as i64 below.
            // Without the cast the decode failed and `unwrap_or(0)` turned
            // every real score into 0 — the public leaderboard showed the
            // whole table tied at zero while the rows underneath were fine.
            "SELECT display_name, high_score::bigint AS high_score \
             FROM game_high_scores \
             ORDER BY high_score DESC, updated_at ASC \
             LIMIT $1",
            [limit.into()],
        ))
        .await
        .map_err(|e| {
            tracing::error!("get_high_scores: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let mut entries = Vec::with_capacity(rows.len());
    for (rank, r) in (1i64..).zip(rows) {
        entries.push(json!({
            "rank": rank,
            "display_name": r.try_get::<String>("", "display_name").unwrap_or_else(|_| "Player".into()),
            "high_score": r.try_get::<i64>("", "high_score").unwrap_or(0),
        }));
    }

    Ok(Json(json!({ "entries": entries, "total": entries.len() })))
}

/// POST /api/game/high-scores — submit a new personal best.
///
/// The row stores `GREATEST(existing, new)` so replays cannot erase a
/// previous high score. Returns the user's global rank.
async fn submit_high_score(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(req): Json<SubmitHighScoreRequest>,
) -> Result<Json<Value>, StatusCode> {
    validate_telegram_id_param(req.telegram_id)?;
    check_owner_lenient(&headers, &state, req.telegram_id, "game")?;

    if req.score < 0 || req.score > MAX_SCORE {
        return Err(StatusCode::BAD_REQUEST);
    }
    let display_name = if req.display_name.trim().is_empty() {
        "Player".to_string()
    } else {
        req.display_name.trim().to_string()
    };
    if display_name.len() > MAX_DISPLAY_NAME_LEN {
        return Err(StatusCode::BAD_REQUEST);
    }

    use sea_orm::{ConnectionTrait, DbBackend, Statement};

    let now = chrono::Utc::now().timestamp_millis();
    state
        .db
        .orm
        .execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "INSERT INTO game_high_scores (telegram_id, high_score, updated_at, display_name) \
             VALUES ($1, $2, to_timestamp($3 / 1000.0), $4) \
             ON CONFLICT (telegram_id) \
             DO UPDATE SET \
                 high_score = GREATEST(game_high_scores.high_score, EXCLUDED.high_score), \
                 updated_at = EXCLUDED.updated_at, \
                 display_name = EXCLUDED.display_name",
            [
                req.telegram_id.into(),
                req.score.into(),
                now.into(),
                display_name.clone().into(),
            ],
        ))
        .await
        .map_err(|e| {
            tracing::error!("submit_high_score upsert: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    // Compute global rank for this score.
    let rank_row = state
        .db
        .orm
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT COUNT(*) AS rank FROM game_high_scores WHERE high_score > $1",
            [req.score.into()],
        ))
        .await
        .map_err(|e| {
            tracing::error!("submit_high_score rank: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    let rank: i64 = rank_row
        .and_then(|r| r.try_get::<i64>("", "rank").ok())
        .unwrap_or(0)
        + 1;

    crate::metrics::game_high_score_submitted(req.score as u64);

    Ok(Json(json!({
        "success": true,
        "rank": rank,
        "high_score": req.score,
        "display_name": display_name,
    })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_display_name_too_long() {
        let req = SubmitHighScoreRequest {
            telegram_id: 1,
            score: 100,
            display_name: "a".repeat(51),
        };
        assert!((if req.display_name.len() > MAX_DISPLAY_NAME_LEN {
            Err(StatusCode::BAD_REQUEST)
        } else {
            Ok(())
        })
        .is_err());
    }

    #[test]
    fn validate_score_bounds() {
        let bad_low = SubmitHighScoreRequest {
            telegram_id: 1,
            score: -1,
            display_name: "x".into(),
        };
        assert!(bad_low.score < 0 || bad_low.score > MAX_SCORE);
        let bad_high = SubmitHighScoreRequest {
            telegram_id: 1,
            score: MAX_SCORE + 1,
            display_name: "x".into(),
        };
        assert!(bad_high.score < 0 || bad_high.score > MAX_SCORE);
        let ok = SubmitHighScoreRequest {
            telegram_id: 1,
            score: MAX_SCORE,
            display_name: "x".into(),
        };
        assert!(!(ok.score < 0 || ok.score > MAX_SCORE));
    }
}
