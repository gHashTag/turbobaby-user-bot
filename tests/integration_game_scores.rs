//! Integration tests for the game leaderboard.
//!
//! `src/api/game.rs` sat at 21% coverage. The property that matters is the
//! `GREATEST(existing, new)` upsert: a later, worse run must never erase a
//! player's personal best. Everything else here is input validation on a
//! public-facing, user-submitted score.
//!
//! Marked `#[ignore]` — runs with:
//!
//! ```sh
//! DATABASE_URL=postgres://... cargo test --features backend -- --ignored
//! ```

#![cfg(feature = "backend")]

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;

const BOT_TOKEN: &str = "dummy_test_token";

struct Resp {
    status: StatusCode,
    body: Value,
}

async fn send(app: axum::Router, request: Request<Body>) -> Resp {
    let response = app.oneshot(request).await.expect("router.oneshot");
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or(Value::Null)
    };
    Resp { status, body }
}

fn fresh_telegram_id() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    800_000_000 + (nanos % 90_000_000) as i64
}

async fn submit(app: axum::Router, tid: i64, score: i64, name: &str) -> Resp {
    let request = Request::builder()
        .method("POST")
        .uri("/api/game/high-scores")
        .header("content-type", "application/json")
        .header(
            "X-Telegram-Init-Data",
            common::make_init_data(tid, BOT_TOKEN),
        )
        .body(Body::from(
            serde_json::to_vec(&json!({
                "telegram_id": tid,
                "score": score,
                "display_name": name,
            }))
            .unwrap(),
        ))
        .unwrap();
    send(app, request).await
}

async fn leaderboard(app: axum::Router, limit: &str) -> Resp {
    send(
        app,
        Request::builder()
            .uri(format!("/api/game/high-scores?limit={limit}"))
            .body(Body::empty())
            .unwrap(),
    )
    .await
}

/// What is actually stored for this player.
///
/// The leaderboard endpoint returns only the top 100, so once the table has
/// accumulated rows a modest test score stops being visible there. Storage
/// semantics — personal best kept, one row per player, blank name replaced —
/// have to be asserted against the row itself, or the test quietly turns into
/// a test of how full the board is.
async fn stored_score(db: &woody_weed_bot::db::Database, tid: i64) -> Option<i64> {
    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    db.orm
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT high_score::bigint AS high_score FROM game_high_scores WHERE telegram_id = $1",
            [tid.into()],
        ))
        .await
        .expect("stored score query")
        .and_then(|r| r.try_get::<i64>("", "high_score").ok())
}

async fn stored_rows(db: &woody_weed_bot::db::Database, tid: i64) -> i64 {
    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    db.orm
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT COUNT(*)::bigint AS n FROM game_high_scores WHERE telegram_id = $1",
            [tid.into()],
        ))
        .await
        .expect("row count query")
        .and_then(|r| r.try_get::<i64>("", "n").ok())
        .unwrap_or(0)
}

async fn stored_name(db: &woody_weed_bot::db::Database, tid: i64) -> Option<String> {
    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    db.orm
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT display_name FROM game_high_scores WHERE telegram_id = $1",
            [tid.into()],
        ))
        .await
        .expect("display name query")
        .and_then(|r| r.try_get::<String>("", "display_name").ok())
}

/// Find a player's score on the board, if present.
#[allow(dead_code)]
fn score_of(board: &Value, name: &str) -> Option<i64> {
    board["entries"]
        .as_array()?
        .iter()
        .find(|e| e["display_name"] == name)
        .and_then(|e| e["high_score"].as_i64())
}

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn a_worse_later_run_never_lowers_a_personal_best() {
    // The whole point of the GREATEST upsert. Without it, one bad run after a
    // great one would silently destroy the player's record.
    let Some((app, db)) = common::make_app_with_db().await else {
        eprintln!("DATABASE_URL not set — skipping");
        return;
    };
    let tid = fresh_telegram_id();
    let name = format!("best-{tid}");

    assert_eq!(
        submit(app.clone(), tid, 5_000, &name).await.status,
        StatusCode::OK
    );
    assert_eq!(submit(app, tid, 10, &name).await.status, StatusCode::OK);

    assert_eq!(
        stored_score(&db, tid).await,
        Some(5_000),
        "the personal best must survive a worse run"
    );
}

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn a_better_run_replaces_the_previous_best() {
    let Some((app, db)) = common::make_app_with_db().await else {
        eprintln!("DATABASE_URL not set — skipping");
        return;
    };
    let tid = fresh_telegram_id();
    let name = format!("improve-{tid}");

    submit(app.clone(), tid, 100, &name).await;
    submit(app, tid, 900, &name).await;

    assert_eq!(stored_score(&db, tid).await, Some(900));
}

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn a_player_occupies_exactly_one_leaderboard_row() {
    // Repeated submissions must upsert, not append — otherwise one player
    // would fill the whole board.
    let Some((app, db)) = common::make_app_with_db().await else {
        eprintln!("DATABASE_URL not set — skipping");
        return;
    };
    let tid = fresh_telegram_id();
    let name = format!("single-{tid}");

    for score in [10, 20, 30] {
        submit(app.clone(), tid, score, &name).await;
    }

    assert_eq!(
        stored_rows(&db, tid).await,
        1,
        "three submissions must leave one row, not three"
    );
}

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn out_of_range_scores_are_refused() {
    let Some(app) = common::make_app().await else {
        eprintln!("DATABASE_URL not set — skipping");
        return;
    };
    let tid = fresh_telegram_id();

    for score in [-1_i64, 10_000_001] {
        assert_eq!(
            submit(app.clone(), tid, score, "cheater").await.status,
            StatusCode::BAD_REQUEST,
            "score {score} must be refused"
        );
    }
    // Zero is a legitimate score — a player who scored nothing still ranks.
    assert_eq!(
        submit(app, tid, 0, "zero").await.status,
        StatusCode::OK,
        "zero is a real score, not an invalid one"
    );
}

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn an_over_long_display_name_is_refused() {
    let Some(app) = common::make_app().await else {
        eprintln!("DATABASE_URL not set — skipping");
        return;
    };
    let tid = fresh_telegram_id();
    let resp = submit(app, tid, 10, &"n".repeat(51)).await;
    assert_eq!(resp.status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn a_blank_display_name_falls_back_to_player() {
    let Some((app, db)) = common::make_app_with_db().await else {
        eprintln!("DATABASE_URL not set — skipping");
        return;
    };
    let tid = fresh_telegram_id();
    assert_eq!(submit(app, tid, 7, "   ").await.status, StatusCode::OK);

    assert_eq!(
        stored_name(&db, tid).await.as_deref(),
        Some("Player"),
        "a blank name must be stored as the Player placeholder, not empty"
    );
}

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn the_board_is_ordered_by_score_descending() {
    let Some(app) = common::make_app().await else {
        eprintln!("DATABASE_URL not set — skipping");
        return;
    };
    let board = leaderboard(app, "100").await;
    assert_eq!(board.status, StatusCode::OK);
    let scores: Vec<i64> = board.body["entries"]
        .as_array()
        .expect("entries array")
        .iter()
        .filter_map(|e| e["high_score"].as_i64())
        .collect();
    assert!(
        scores.windows(2).all(|w| w[0] >= w[1]),
        "leaderboard must be sorted high to low, got {scores:?}"
    );
}

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn ranks_are_consecutive_starting_at_one() {
    let Some(app) = common::make_app().await else {
        eprintln!("DATABASE_URL not set — skipping");
        return;
    };
    let board = leaderboard(app, "100").await;
    let ranks: Vec<i64> = board.body["entries"]
        .as_array()
        .expect("entries array")
        .iter()
        .filter_map(|e| e["rank"].as_i64())
        .collect();
    assert_eq!(
        ranks,
        (1..=ranks.len() as i64).collect::<Vec<_>>(),
        "ranks must run 1..n with no gaps or repeats"
    );
}

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn the_limit_is_clamped_rather_than_trusted() {
    // `limit` is a public query parameter: a huge or zero value must not turn
    // into an unbounded scan or an empty board.
    let Some(app) = common::make_app().await else {
        eprintln!("DATABASE_URL not set — skipping");
        return;
    };
    let tid = fresh_telegram_id();
    submit(app.clone(), tid, 42, &format!("limit-{tid}")).await;

    let huge = leaderboard(app.clone(), "999999").await;
    assert_eq!(huge.status, StatusCode::OK);
    assert!(
        huge.body["entries"].as_array().expect("entries").len() <= 100,
        "limit must be clamped to 100"
    );

    let zero = leaderboard(app, "0").await;
    assert_eq!(zero.status, StatusCode::OK);
    assert_eq!(
        zero.body["entries"].as_array().expect("entries").len(),
        1,
        "limit 0 must clamp up to 1, not return nothing"
    );
}

#[tokio::test]
#[ignore = "needs DATABASE_URL env var; run with --ignored"]
async fn a_score_cannot_be_submitted_for_another_player() {
    let Some(app) = common::make_app().await else {
        eprintln!("DATABASE_URL not set — skipping");
        return;
    };
    let victim = fresh_telegram_id();
    let attacker = victim + 1;

    let resp = send(
        app,
        Request::builder()
            .method("POST")
            .uri("/api/game/high-scores")
            .header("content-type", "application/json")
            .header(
                "X-Telegram-Init-Data",
                common::make_init_data(attacker, BOT_TOKEN),
            )
            .body(Body::from(
                serde_json::to_vec(&json!({
                    "telegram_id": victim,
                    "score": 9_999_999,
                    "display_name": "spoofed",
                }))
                .unwrap(),
            ))
            .unwrap(),
    )
    .await;
    assert_eq!(
        resp.status,
        StatusCode::FORBIDDEN,
        "initData for one user must not write another user's score"
    );
}
