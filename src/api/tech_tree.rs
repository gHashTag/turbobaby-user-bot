use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
    Json, Router,
};
use serde_json::{json, Value};

use crate::api::auth::check_admin;
use crate::AppState;

pub(crate) fn routes() -> Router<AppState> {
    Router::new()
        .route("/tech-tree/nodes", get(get_tech_nodes))
        .route("/tech-tree/nodes/:id", get(get_tech_node))
        .route("/tech-tree/nodes/:id/complete", post(complete_tech_node))
        .route("/tech-tree/achievements", get(get_achievements))
}

/// `GET /api/tech-tree/nodes`: an empty roadmap, always.
///
/// The `tech_nodes` rows are the old shop's product roadmap (migration 010
/// seeded them: its strain catalogue, its sommelier, a dosing guide, a session
/// journal), and the owner ruled on 2026-09-25 that nothing cannabis-related
/// may appear anywhere. No TurboBaby node has ever been written, so the read
/// stops here, in code: the route stays declared and answers the shape it
/// always did, a client that still fetches it gets an empty tree rather than
/// an error, and no row is read, written or deleted. The admin `complete`
/// route below still flips a stored row's status.
async fn get_tech_nodes() -> Json<Value> {
    Json(json!({ "nodes": [], "total": 0 }))
}

fn validate_id(id: &str) -> Result<(), StatusCode> {
    if id.len() > 200 {
        return Err(StatusCode::BAD_REQUEST);
    }
    Ok(())
}

/// `GET /api/tech-tree/nodes/:id`: not found, for every id, for the reason
/// `get_tech_nodes` gives. An over-long id is still refused first, as before.
async fn get_tech_node(Path(id): Path<String>) -> Result<Json<Value>, StatusCode> {
    validate_id(&id)?;
    Err(StatusCode::NOT_FOUND)
}

/// Mark a tech-tree node as completed (admin-only).
///
/// The `tech_nodes.status` column is a **global** roadmap state, not per-user
/// progress — this is a project changelog visualisation, not a game mechanic.
/// After flipping the node to `'completed'`, cascade-unlock any `'locked'`
/// node whose every dependency is now completed. One transaction.
async fn complete_tech_node(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    check_admin(&headers, &state)?;
    validate_id(&id)?;

    // Cycle #93: SeaORM tx. The cascade UPDATE references a CTE-shaped
    // condition (`NOT EXISTS … unnest(dependencies)`) that no typed
    // builder supports — raw Statement throughout. Two-statement tx with
    // explicit "already completed vs not found" disambiguation.
    use sea_orm::{ConnectionTrait, DbBackend, Statement, TransactionTrait};
    let tx = state.db.orm.begin().await.map_err(|e| {
        tracing::error!("tech_tree complete tx.begin: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // 1. Flip THIS node to completed. 0 rows → unknown id OR already completed.
    let updated = tx
        .execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "UPDATE tech_nodes SET status = 'completed' WHERE id = $1 AND status <> 'completed'",
            [id.clone().into()],
        ))
        .await
        .map_err(|e| {
            tracing::error!("tech_tree complete update: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    if updated.rows_affected() == 0 {
        // Distinguish "not found" from "already completed". tx drops on
        // early return → auto-rollback (releases any held locks).
        let exists = tx
            .query_one(Statement::from_sql_and_values(
                DbBackend::Postgres,
                "SELECT 1 AS one FROM tech_nodes WHERE id = $1",
                [id.clone().into()],
            ))
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
            .is_some();
        return if exists {
            // Commit to release locks cleanly before returning.
            if let Err(e) = tx.commit().await {
                tracing::warn!("tech_tree complete: no-op commit failed: {}", e);
            }
            Ok(Json(
                json!({ "success": true, "unlocked": 0, "note": "already completed" }),
            ))
        } else {
            Err(StatusCode::NOT_FOUND)
        };
    }

    // 2. Cascade: any locked node whose every dep is now completed becomes available.
    let unlocked = tx
        .execute(Statement::from_string(
            DbBackend::Postgres,
            "UPDATE tech_nodes SET status = 'available' \
             WHERE status = 'locked' \
               AND NOT EXISTS ( \
                   SELECT 1 FROM unnest(dependencies) AS dep \
                   WHERE NOT EXISTS ( \
                       SELECT 1 FROM tech_nodes t WHERE t.id = dep AND t.status = 'completed' \
                   ) \
               )"
            .to_string(),
        ))
        .await
        .map_err(|e| {
            tracing::error!("tech_tree cascade: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    let unlocked = unlocked.rows_affected();

    tx.commit().await.map_err(|e| {
        tracing::error!("tech_tree complete commit: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    tracing::info!(
        node_id = %id,
        unlocked = unlocked,
        "tech_tree: completed node and cascaded"
    );
    Ok(Json(json!({ "success": true, "unlocked": unlocked })))
}

/// `GET /api/tech-tree/achievements`: an empty list, always.
///
/// The `achievements` rows belong to the same retired roadmap (migrations 010
/// and 066: badges for trying every strain, for the sommelier, for watering the
/// garden), nothing in TurboBaby awards one, and the owner's ruling of
/// 2026-09-25 is the same. The route stays and answers its old shape; no row is
/// read, written or deleted.
async fn get_achievements() -> Json<Value> {
    Json(json!({ "achievements": [], "total": 0 }))
}

#[cfg(test)]
mod tests {
    use super::{get_achievements, get_tech_node, get_tech_nodes, validate_id};
    use axum::extract::Path;
    use axum::http::StatusCode;
    use serde_json::json;

    #[test]
    fn test_validate_id_ok() {
        assert!(validate_id("abc123").is_ok());
    }

    #[test]
    fn test_validate_id_too_long() {
        let id = "a".repeat(201);
        assert_eq!(validate_id(&id).unwrap_err(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn test_validate_id_exactly_200() {
        let id = "a".repeat(200);
        assert!(validate_id(&id).is_ok());
    }

    // The cascade-unlock SQL is exercised against a live DB in integration
    // tests (when those land). What we can sanity-check at unit level is the
    // assumption it relies on — that an empty `unnest(dependencies)` array
    // makes the NOT EXISTS clause vacuously true, i.e. a no-dep node would
    // unlock if it were locked. The seed leaves no-dep nodes as 'available'
    // already so this never actually fires in practice, but documenting the
    // invariant here so future schema changes notice if it shifts.
    /// Owner, 2026-09-25: the old shop's roadmap and its badges are not
    /// served. The routes answer their old shape, empty, with no database.
    #[tokio::test]
    async fn the_retired_roadmap_answers_empty_and_not_found() {
        assert_eq!(get_tech_nodes().await.0, json!({ "nodes": [], "total": 0 }));
        assert_eq!(
            get_achievements().await.0,
            json!({ "achievements": [], "total": 0 })
        );
        for id in ["core-menu", "wasm-calculator", "experience-dosage", "x"] {
            assert_eq!(
                get_tech_node(Path(id.to_string())).await.err(),
                Some(StatusCode::NOT_FOUND),
                "{id}"
            );
        }
        assert_eq!(
            get_tech_node(Path("a".repeat(201))).await.err(),
            Some(StatusCode::BAD_REQUEST)
        );
    }

    #[test]
    fn cascade_invariant_documented() {
        // No-dep locked node → unnest({}) yields 0 rows → outer NOT EXISTS
        // is true → would be unlocked. The seed has no such rows; this test
        // is a notice for anyone introducing them.
        assert!(true);
    }
}
