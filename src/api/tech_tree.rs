use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use serde_json::{json, Value};

use crate::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/tech-tree/nodes", get(get_tech_nodes))
        .route("/tech-tree/nodes/:id", get(get_tech_node))
        .route("/tech-tree/achievements", get(get_achievements))
}

async fn get_tech_nodes(State(state): State<AppState>) -> Result<Json<Value>, StatusCode> {
    let client = state.db.pool.get().await.map_err(|e| { tracing::error!("DB error: {:?}", e); StatusCode::INTERNAL_SERVER_ERROR })?;
    let rows = client.query(
        "SELECT id, name, description, category, icon, status, xp_required, xp_reward, \
                dependencies, unlocks, features, estimated_hours, priority \
         FROM tech_nodes ORDER BY priority ASC, xp_required ASC LIMIT 2000",
        &[],
    ).await.map_err(|e| { tracing::error!("DB error: {:?}", e); StatusCode::INTERNAL_SERVER_ERROR })?;

    let nodes: Vec<Value> = rows.iter().map(|r| json!({
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
    })).collect();

    Ok(Json(json!({ "nodes": nodes, "total": nodes.len() })))
}

fn validate_id(id: &str) -> Result<(), StatusCode> {
    if id.len() > 200 { return Err(StatusCode::BAD_REQUEST); }
    Ok(())
}

async fn get_tech_node(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    validate_id(&id)?;
    let client = state.db.pool.get().await.map_err(|e| { tracing::error!("DB error: {:?}", e); StatusCode::INTERNAL_SERVER_ERROR })?;
    let row = client.query_opt(
        "SELECT id, name, description, category, icon, status, xp_required, xp_reward, \
                dependencies, unlocks, features, estimated_hours, priority \
         FROM tech_nodes WHERE id = $1",
        &[&id],
    ).await.map_err(|e| { tracing::error!("DB error: {:?}", e); StatusCode::INTERNAL_SERVER_ERROR })?;

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

async fn get_achievements(State(state): State<AppState>) -> Result<Json<Value>, StatusCode> {
    let client = state.db.pool.get().await.map_err(|e| { tracing::error!("DB error: {:?}", e); StatusCode::INTERNAL_SERVER_ERROR })?;
    let rows = client.query(
        "SELECT id, name, description, icon, xp_reward, requirement, category \
         FROM achievements ORDER BY xp_reward ASC LIMIT 2000",
        &[],
    ).await.map_err(|e| { tracing::error!("DB error: {:?}", e); StatusCode::INTERNAL_SERVER_ERROR })?;

    let achievements: Vec<Value> = rows.iter().map(|r| json!({
        "id":          r.try_get::<_, String>("id").unwrap_or_default(),
        "name":        r.try_get::<_, String>("name").unwrap_or_default(),
        "description": r.try_get::<_, String>("description").unwrap_or_default(),
        "icon":        r.try_get::<_, String>("icon").unwrap_or_default(),
        "xp_reward":   r.try_get::<_, i32>("xp_reward").unwrap_or(0),
        "requirement": r.try_get::<_, String>("requirement").unwrap_or_default(),
        "category":    r.try_get::<_, String>("category").unwrap_or_default(),
    })).collect();

    Ok(Json(json!({ "achievements": achievements, "total": achievements.len() })))
}

#[cfg(test)]
mod tests {
    use super::{validate_id};
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
}
