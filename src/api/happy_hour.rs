use crate::AppState;
use axum::{extract::State, http::StatusCode, routing::get, Json, Router};
use chrono::Timelike;
use serde_json::{json, Value};

pub(crate) fn routes() -> Router<AppState> {
    Router::new().route("/happy-hour", get(get_happy_hour))
}

fn compute_happy_hour(config: &Value, current_hour: i64) -> (bool, bool, f64, i64, i64) {
    let happy_hour = &config["happy_hour"];
    let enabled = happy_hour["enabled"].as_bool().unwrap_or(false);
    let start = happy_hour["start"].as_i64().unwrap_or(18);
    let end = happy_hour["end"].as_i64().unwrap_or(21);
    let active = enabled && current_hour >= start && current_hour < end;
    let discount_raw = happy_hour["discount"].as_f64().unwrap_or(0.0);
    let discount = if discount_raw.is_finite() {
        discount_raw.clamp(0.0, 100.0)
    } else {
        0.0
    };
    (enabled, active, discount, start, end)
}

async fn get_happy_hour(State(state): State<AppState>) -> Result<Json<Value>, StatusCode> {
    // Cycle #82: SeaORM. The `loyalty_config` table holds a singleton row
    // (id=1) with a JSONB blob; `find_by_id(1)` is enough — no LIMIT
    // needed.
    use crate::db::entities::loyalty_config::Entity as LoyaltyConfigEntity;
    use sea_orm::EntityTrait;
    let model = LoyaltyConfigEntity::find_by_id(1)
        .one(&state.db.orm)
        .await
        .map_err(|e| {
            tracing::error!("get_happy_hour SeaORM error: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    match model {
        Some(m) => {
            let current_hour = chrono::Local::now().hour() as i64;
            let (enabled, active, discount, start, end) =
                compute_happy_hour(&m.config, current_hour);
            Ok(Json(json!({
                "enabled": enabled,
                "active": active,
                "discount": discount,
                "start": start,
                "end": end,
            })))
        }
        None => Ok(Json(json!({
            "enabled": false,
            "active": false,
            "discount": 0,
            "start": 18,
            "end": 21,
        }))),
    }
}

#[cfg(test)]
mod tests {
    use super::compute_happy_hour;
    use serde_json::json;

    #[test]
    fn test_compute_happy_hour_active() {
        let config =
            json!({"happy_hour": {"enabled": true, "start": 18, "end": 21, "discount": 10.0}});
        let (enabled, active, discount, start, end) = compute_happy_hour(&config, 19);
        assert!(enabled);
        assert!(active);
        assert_eq!(discount, 10.0);
        assert_eq!(start, 18);
        assert_eq!(end, 21);
    }

    #[test]
    fn test_compute_happy_hour_inactive_before() {
        let config =
            json!({"happy_hour": {"enabled": true, "start": 18, "end": 21, "discount": 10.0}});
        let (_, active, _, _, _) = compute_happy_hour(&config, 17);
        assert!(!active);
    }

    #[test]
    fn test_compute_happy_hour_inactive_after() {
        let config =
            json!({"happy_hour": {"enabled": true, "start": 18, "end": 21, "discount": 10.0}});
        let (_, active, _, _, _) = compute_happy_hour(&config, 21);
        assert!(!active);
    }

    #[test]
    fn test_compute_happy_hour_disabled() {
        let config =
            json!({"happy_hour": {"enabled": false, "start": 18, "end": 21, "discount": 10.0}});
        let (enabled, active, _, _, _) = compute_happy_hour(&config, 19);
        assert!(!enabled);
        assert!(!active);
    }

    #[test]
    fn test_compute_happy_hour_defaults() {
        let config = json!({});
        let (enabled, active, discount, start, end) = compute_happy_hour(&config, 19);
        assert!(!enabled);
        assert!(!active);
        assert_eq!(discount, 0.0);
        assert_eq!(start, 18);
        assert_eq!(end, 21);
    }

    #[test]
    fn test_compute_happy_hour_discount_clamp_high() {
        let config = json!({"happy_hour": {"enabled": true, "discount": 150.0}});
        let (_, _, discount, _, _) = compute_happy_hour(&config, 19);
        assert_eq!(discount, 100.0);
    }

    #[test]
    fn test_compute_happy_hour_discount_clamp_low() {
        let config = json!({"happy_hour": {"enabled": true, "discount": -10.0}});
        let (_, _, discount, _, _) = compute_happy_hour(&config, 19);
        assert_eq!(discount, 0.0);
    }

    #[test]
    fn test_compute_happy_hour_discount_nan() {
        let config = json!({"happy_hour": {"enabled": true, "discount": f64::NAN}});
        let (_, _, discount, _, _) = compute_happy_hour(&config, 19);
        assert_eq!(discount, 0.0);
    }

    #[test]
    fn test_compute_happy_hour_discount_inf() {
        let config = json!({"happy_hour": {"enabled": true, "discount": f64::INFINITY}});
        let (_, _, discount, _, _) = compute_happy_hour(&config, 19);
        assert_eq!(discount, 0.0);
    }

    #[test]
    fn test_compute_happy_hour_start_greater_than_end() {
        let config = json!({"happy_hour": {"enabled": true, "start": 21, "end": 18}});
        let (_, active, _, _, _) = compute_happy_hour(&config, 19);
        assert!(!active);
    }
}
