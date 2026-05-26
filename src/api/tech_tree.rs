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
         FROM tech_nodes ORDER BY priority ASC, xp_required ASC",
        &[],
    ).await.map_err(|e| { tracing::error!("DB error: {:?}", e); StatusCode::INTERNAL_SERVER_ERROR })?;

    let nodes: Vec<Value> = rows.iter().map(|r| json!({
        "id":              r.get::<_, String>("id"),
        "name":            r.get::<_, String>("name"),
        "description":     r.get::<_, String>("description"),
        "category":        r.get::<_, String>("category"),
        "icon":            r.get::<_, String>("icon"),
        "status":          r.get::<_, String>("status"),
        "xp_required":     r.get::<_, i32>("xp_required"),
        "xp_reward":       r.get::<_, i32>("xp_reward"),
        "dependencies":    r.get::<_, Vec<String>>("dependencies"),
        "unlocks":         r.get::<_, Vec<String>>("unlocks"),
        "features":        r.get::<_, Vec<String>>("features"),
        "estimated_hours": r.get::<_, i32>("estimated_hours"),
        "priority":        r.get::<_, i32>("priority"),
    })).collect();

    Ok(Json(json!({ "nodes": nodes, "total": nodes.len() })))
}

async fn get_tech_node(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, StatusCode> {
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
                "id":              r.get::<_, String>("id"),
                "name":            r.get::<_, String>("name"),
                "description":     r.get::<_, String>("description"),
                "category":        r.get::<_, String>("category"),
                "icon":            r.get::<_, String>("icon"),
                "status":          r.get::<_, String>("status"),
                "xp_required":     r.get::<_, i32>("xp_required"),
                "xp_reward":       r.get::<_, i32>("xp_reward"),
                "dependencies":    r.get::<_, Vec<String>>("dependencies"),
                "unlocks":         r.get::<_, Vec<String>>("unlocks"),
                "features":        r.get::<_, Vec<String>>("features"),
                "estimated_hours": r.get::<_, i32>("estimated_hours"),
                "priority":        r.get::<_, i32>("priority"),
            }
        }))),
        None => Err(StatusCode::NOT_FOUND),
    }
}

async fn get_achievements(State(state): State<AppState>) -> Result<Json<Value>, StatusCode> {
    let client = state.db.pool.get().await.map_err(|e| { tracing::error!("DB error: {:?}", e); StatusCode::INTERNAL_SERVER_ERROR })?;
    let rows = client.query(
        "SELECT id, name, description, icon, xp_reward, requirement, category \
         FROM achievements ORDER BY xp_reward ASC",
        &[],
    ).await.map_err(|e| { tracing::error!("DB error: {:?}", e); StatusCode::INTERNAL_SERVER_ERROR })?;

    let achievements: Vec<Value> = rows.iter().map(|r| json!({
        "id":          r.get::<_, String>("id"),
        "name":        r.get::<_, String>("name"),
        "description": r.get::<_, String>("description"),
        "icon":        r.get::<_, String>("icon"),
        "xp_reward":   r.get::<_, i32>("xp_reward"),
        "requirement": r.get::<_, String>("requirement"),
        "category":    r.get::<_, String>("category"),
    })).collect();

    Ok(Json(json!({ "achievements": achievements, "total": achievements.len() })))
}
