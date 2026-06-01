pub mod admin;
pub mod auth;
pub mod cache;
pub mod cart;
pub mod catalog;
pub mod garden;
pub mod happy_hour;
pub mod loyalty;
#[cfg(feature = "utoipa")]
pub mod openapi;
pub mod orders;
pub mod quest;
pub mod rate_limit;
pub mod referrals;
pub mod strains;
pub mod tech_tree;
pub mod upload;

use crate::AppState;
use axum::http::StatusCode;
use axum::{extract::DefaultBodyLimit, response::Json, routing::get, Router};
use serde_json::{json, Value};

/// Extract a required boolean field from a JSON body.
pub(crate) fn extract_bool(body: &Value, key: &str) -> Result<bool, StatusCode> {
    body.get(key)
        .and_then(|v| v.as_bool())
        .ok_or(StatusCode::BAD_REQUEST)
}

/// Extract a required discount field (f64, 0..=100, finite) from a JSON body.
pub(crate) fn extract_discount(body: &Value) -> Result<f64, StatusCode> {
    let discount = body
        .get("discount")
        .and_then(|v| v.as_f64())
        .ok_or(StatusCode::BAD_REQUEST)?;
    if !discount.is_finite() || !(0.0..=100.0).contains(&discount) {
        return Err(StatusCode::BAD_REQUEST);
    }
    Ok(discount)
}

/// Validates that a URL is either empty/None or starts with an allowed scheme.
/// Allowed: http://, https://, /, data:image/, data:video/
pub fn validate_url(url: &Option<String>) -> Result<(), StatusCode> {
    if let Some(ref u) = url {
        if u.is_empty() {
            return Ok(());
        }
        if u.len() > 2048 {
            return Err(StatusCode::BAD_REQUEST);
        }
        // Block protocol-relative URLs (//evil.com/...)
        if u.starts_with("//") {
            return Err(StatusCode::BAD_REQUEST);
        }
        let allowed = u.starts_with("http://")
            || u.starts_with("https://")
            || u.starts_with('/')
            || u.starts_with("data:image/")
            || u.starts_with("data:video/");
        if !allowed {
            return Err(StatusCode::BAD_REQUEST);
        }
    }
    Ok(())
}

pub fn router(state: crate::AppState) -> Router {
    Router::new()
        .route("/health", get(health_handler))
        .nest("/api", api_routes(state.clone()))
}

fn api_routes(state: AppState) -> Router {
    let mut router = Router::new()
        .route("/ping", get(ping_handler))
        .merge(orders::routes())
        .merge(strains::routes())
        .merge(loyalty::routes())
        .merge(admin::routes())
        .merge(catalog::routes())
        .merge(quest::routes())
        .merge(happy_hour::routes())
        .merge(garden::routes())
        .merge(referrals::routes())
        .merge(tech_tree::routes())
        .merge(cart::routes());

    // Apply 2MB body limit to all non-upload routes.
    // Upload routes are merged AFTER this layer so their own 110MB limit remains effective.
    router = router.layer(DefaultBodyLimit::max(2 * 1024 * 1024));
    router = router.merge(upload::routes());

    router.with_state(state)
}

async fn ping_handler() -> Json<Value> {
    Json(json!({"status": "ok", "service": "woody-weed-bot"}))
}

async fn health_handler() -> Json<Value> {
    Json(json!({ "status": "ok", "service": "woody-weed-bot" }))
}

#[cfg(test)]
mod tests {
    use super::{extract_bool, extract_discount, validate_url};
    use axum::http::StatusCode;
    use serde_json::json;

    #[test]
    fn validate_url_accepts_none() {
        assert!(validate_url(&None).is_ok());
    }

    #[test]
    fn validate_url_accepts_empty() {
        assert!(validate_url(&Some("".to_string())).is_ok());
    }

    #[test]
    fn validate_url_accepts_http() {
        assert!(validate_url(&Some("http://example.com/img.jpg".to_string())).is_ok());
    }

    #[test]
    fn validate_url_accepts_https() {
        assert!(validate_url(&Some("https://example.com/img.jpg".to_string())).is_ok());
    }

    #[test]
    fn validate_url_accepts_relative() {
        assert!(validate_url(&Some("/uploads/abc.jpg".to_string())).is_ok());
    }

    #[test]
    fn validate_url_accepts_data_image() {
        assert!(validate_url(&Some("data:image/png;base64,iVBORw0KGgo=".to_string())).is_ok());
    }

    #[test]
    fn validate_url_accepts_data_video() {
        assert!(validate_url(&Some("data:video/mp4;base64,AAAA".to_string())).is_ok());
    }

    #[test]
    fn validate_url_rejects_javascript() {
        assert_eq!(
            validate_url(&Some("javascript:alert(1)".to_string())).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn validate_url_rejects_data_text_html() {
        assert_eq!(
            validate_url(&Some(
                "data:text/html,<script>alert(1)</script>".to_string()
            ))
            .unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn validate_url_rejects_ftp() {
        assert_eq!(
            validate_url(&Some("ftp://evil.com/file.jpg".to_string())).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn validate_url_rejects_protocol_relative() {
        assert_eq!(
            validate_url(&Some("//evil.com/img.jpg".to_string())).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn validate_url_rejects_too_long() {
        let long = "https://x.com/".to_string() + &"a".repeat(3000);
        assert_eq!(
            validate_url(&Some(long)).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn test_extract_bool_true() {
        assert!(extract_bool(&json!({"k": true}), "k").unwrap());
    }

    #[test]
    fn test_extract_bool_false() {
        assert!(!extract_bool(&json!({"k": false}), "k").unwrap());
    }

    #[test]
    fn test_extract_bool_missing() {
        assert_eq!(
            extract_bool(&json!({}), "k").unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn test_extract_bool_not_bool() {
        assert_eq!(
            extract_bool(&json!({"k": "true"}), "k").unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn test_extract_discount_ok() {
        assert_eq!(extract_discount(&json!({"discount": 15.0})).unwrap(), 15.0);
    }

    #[test]
    fn test_extract_discount_missing() {
        assert_eq!(
            extract_discount(&json!({})).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn test_extract_discount_not_number() {
        assert_eq!(
            extract_discount(&json!({"discount": "ten"})).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn test_extract_discount_negative() {
        assert_eq!(
            extract_discount(&json!({"discount": -1.0})).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn test_extract_discount_too_high() {
        assert_eq!(
            extract_discount(&json!({"discount": 101.0})).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn test_extract_discount_nan() {
        assert_eq!(
            extract_discount(&json!({"discount": f64::NAN})).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }
}
