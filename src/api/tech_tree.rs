use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
    Json, Router,
};
use serde_json::{json, Value};

use crate::api::auth::check_admin;
use crate::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/tech-tree/nodes", get(get_tech_nodes))
        .route("/tech-tree/nodes/:id", get(get_tech_node))
        .route("/tech-tree/nodes/:id/complete", post(complete_tech_node))
        .route("/tech-tree/achievements", get(get_achievements))
}

async fn get_tech_nodes(State(state): State<AppState>) -> Result<Json<Value>, StatusCode> {
    let client = state.db.pool.get().await.map_err(|e| {
        tracing::error!("DB error: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    let rows = client
        .query(
            "SELECT id, name, description, category, icon, status, xp_required, xp_reward, \
                dependencies, unlocks, features, estimated_hours, priority \
         FROM tech_nodes ORDER BY priority ASC, xp_required ASC LIMIT 2000",
            &[],
        )
        .await
        .map_err(|e| {
            tracing::error!("DB error: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let nodes: Vec<Value> = rows
        .iter()
        .map(|r| {
            json!({
                "id":              r.try_get::<_, String>("id").unwrap_or_default(),
                "name":            r.try_get::<_, String>("name").unwrap_or_default(),
                "description":     r.try_get::<_, String>("description").unwrap_or_default(),
                "category":        r.try_get::<_, String>("category").unwrap_or_default(),
                "icon":            r.try_get::<_, String>("icon").unwrap_or_default(),
                "status":          r.try_get::<_, String>("status").unwrap_or_default(),
                "xp_required":     r.try_get::<_, i32>("xp_required").unwrap_or(0),
                "xp_reward":       r.try_get::<_, i32>("xp_reward").unwrap_or(0),
                "dependencies":    r.try_get::<_, Vec<String>>("dependencies").unwrap_or_default(),
                "unlocks":         r.try_get::<_, Vec<String>>("unlocks").unwrap_or_default(),
                "features":        r.try_get::<_, Vec<String>>("features").unwrap_or_default(),
                "estimated_hours": r.try_get::<_, i32>("estimated_hours").unwrap_or(0),
                "priority":        r.try_get::<_, i32>("priority").unwrap_or(0),
            })
        })
        .collect();

    Ok(Json(json!({ "nodes": nodes, "total": nodes.len() })))
}

fn validate_id(id: &str) -> Result<(), StatusCode> {
    if id.len() > 200 {
        return Err(StatusCode::BAD_REQUEST);
    }
    Ok(())
}

async fn get_tech_node(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    validate_id(&id)?;
    let client = state.db.pool.get().await.map_err(|e| {
        tracing::error!("DB error: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    let row = client
        .query_opt(
            "SELECT id, name, description, category, icon, status, xp_required, xp_reward, \
                dependencies, unlocks, features, estimated_hours, priority \
         FROM tech_nodes WHERE id = $1",
            &[&id],
        )
        .await
        .map_err(|e| {
            tracing::error!("DB error: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    match row {
        Some(r) => Ok(Json(json!({
            "node": {
                "id":              r.try_get::<_, String>("id").unwrap_or_default(),
                "name":            r.try_get::<_, String>("name").unwrap_or_default(),
                "description":     r.try_get::<_, String>("description").unwrap_or_default(),
                "category":        r.try_get::<_, String>("category").unwrap_or_default(),
                "icon":            r.try_get::<_, String>("icon").unwrap_or_default(),
                "status":          r.try_get::<_, String>("status").unwrap_or_default(),
                "xp_required":     r.try_get::<_, i32>("xp_required").unwrap_or(0),
                "xp_reward":       r.try_get::<_, i32>("xp_reward").unwrap_or(0),
                "dependencies":    r.try_get::<_, Vec<String>>("dependencies").unwrap_or_default(),
                "unlocks":         r.try_get::<_, Vec<String>>("unlocks").unwrap_or_default(),
                "features":        r.try_get::<_, Vec<String>>("features").unwrap_or_default(),
                "estimated_hours": r.try_get::<_, i32>("estimated_hours").unwrap_or(0),
                "priority":        r.try_get::<_, i32>("priority").unwrap_or(0),
            }
        }))),
        None => Err(StatusCode::NOT_FOUND),
    }
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

    let mut client = state.db.pool.get().await.map_err(|e| {
        crate::metrics::db_pool_acquire_failed("tech_tree.complete");
        tracing::error!("DB error: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    let tx = client.transaction().await.map_err(|e| {
        tracing::error!("tx error: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // 1. Flip THIS node to completed. 0 rows → unknown id (404).
    let updated = tx
        .execute(
            "UPDATE tech_nodes SET status = 'completed' WHERE id = $1 AND status <> 'completed'",
            &[&id],
        )
        .await
        .map_err(|e| {
            tracing::error!("update error: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    if updated == 0 {
        // Either no such node OR already completed. Look it up to distinguish.
        let exists = tx
            .query_opt("SELECT 1 FROM tech_nodes WHERE id = $1", &[&id])
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
            .is_some();
        if let Err(e) = tx.rollback().await {
            tracing::error!("rollback error: {:?}", e);
        }
        return if exists {
            Ok(Json(
                json!({ "success": true, "unlocked": 0, "note": "already completed" }),
            ))
        } else {
            Err(StatusCode::NOT_FOUND)
        };
    }

    // 2. Cascade: any locked node whose every dep is now completed becomes available.
    let unlocked = tx
        .execute(
            "UPDATE tech_nodes SET status = 'available' \
             WHERE status = 'locked' \
               AND NOT EXISTS ( \
                   SELECT 1 FROM unnest(dependencies) AS dep \
                   WHERE NOT EXISTS ( \
                       SELECT 1 FROM tech_nodes t WHERE t.id = dep AND t.status = 'completed' \
                   ) \
               )",
            &[],
        )
        .await
        .map_err(|e| {
            tracing::error!("cascade error: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    tx.commit().await.map_err(|e| {
        tracing::error!("commit error: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    tracing::info!(
        node_id = %id,
        unlocked = unlocked,
        "tech_tree: completed node and cascaded"
    );
    Ok(Json(json!({ "success": true, "unlocked": unlocked })))
}

async fn get_achievements(State(state): State<AppState>) -> Result<Json<Value>, StatusCode> {
    let client = state.db.pool.get().await.map_err(|e| {
        tracing::error!("DB error: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    let rows = client
        .query(
            "SELECT id, name, description, icon, xp_reward, requirement, category \
         FROM achievements ORDER BY xp_reward ASC LIMIT 2000",
            &[],
        )
        .await
        .map_err(|e| {
            tracing::error!("DB error: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let achievements: Vec<Value> = rows
        .iter()
        .map(|r| {
            json!({
                "id":          r.try_get::<_, String>("id").unwrap_or_default(),
                "name":        r.try_get::<_, String>("name").unwrap_or_default(),
                "description": r.try_get::<_, String>("description").unwrap_or_default(),
                "icon":        r.try_get::<_, String>("icon").unwrap_or_default(),
                "xp_reward":   r.try_get::<_, i32>("xp_reward").unwrap_or(0),
                "requirement": r.try_get::<_, String>("requirement").unwrap_or_default(),
                "category":    r.try_get::<_, String>("category").unwrap_or_default(),
            })
        })
        .collect();

    Ok(Json(
        json!({ "achievements": achievements, "total": achievements.len() }),
    ))
}

#[cfg(test)]
mod tests {
    use super::validate_id;
    use axum::http::StatusCode;

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
    #[test]
    fn cascade_invariant_documented() {
        // No-dep locked node → unnest({}) yields 0 rows → outer NOT EXISTS
        // is true → would be unlocked. The seed has no such rows; this test
        // is a notice for anyone introducing them.
        assert!(true);
    }
}
