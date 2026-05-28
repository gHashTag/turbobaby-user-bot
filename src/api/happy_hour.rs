use axum::{
    extract::State,
    http::StatusCode,
    routing::get,
    Json, Router,
};
use chrono::Timelike;
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
            let config: serde_json::Value = r.try_get(0).unwrap_or(Value::Null);
            let happy_hour = &config["happy_hour"];
            let enabled = happy_hour["enabled"].as_bool().unwrap_or(false);
            let start = happy_hour["start"].as_i64().unwrap_or(18);
            let end = happy_hour["end"].as_i64().unwrap_or(21);
            let current_hour = chrono::Local::now().hour() as i64;
            let active = enabled && current_hour >= start && current_hour < end;
            let discount = happy_hour["discount"].as_f64().unwrap_or(0.0).clamp(0.0, 100.0);
            Ok(Json(json!({
                "enabled": enabled,
                "active": active,
                "discount": discount,
                "start": start,
                "end": end,
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
