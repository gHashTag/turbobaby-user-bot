use axum::{
    extract::State,
    http::StatusCode,
    routing::get,
    Json, Router,
};
use serde_json::{json, Value};
use crate::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/happy-hour", get(get_happy_hour))
}

async fn get_happy_hour(State(state): State<AppState>) -> Result<Json<Value>, StatusCode> {
    let client = state.db.pool.get().await.map_err(|e| { tracing::error!("DB error: {:?}", e); StatusCode::INTERNAL_SERVER_ERROR })?;
    let row = client.query_opt(
        "SELECT config FROM loyalty_config LIMIT 1",
        &[],
    ).await.map_err(|e| { tracing::error!("DB error: {:?}", e); StatusCode::INTERNAL_SERVER_ERROR })?;

    match row {
        Some(r) => {
            let config: serde_json::Value = r.get(0);
            let happy_hour = &config["happy_hour"];
            Ok(Json(json!({
                "enabled": happy_hour["enabled"].as_bool().unwrap_or(false),
                "active": happy_hour["active"].as_bool().unwrap_or(false),
                "discount": happy_hour["discount"].as_f64().unwrap_or(0.0),
                "start": happy_hour["start"].as_i64().unwrap_or(18),
                "end": happy_hour["end"].as_i64().unwrap_or(21),
            })))
        },
        None => Ok(Json(json!({
            "enabled": false,
            "active": false,
            "discount": 0,
            "start": 18,
            "end": 21,
        })))
    }
}
