pub mod auth;
pub mod orders;
pub mod strains;
pub mod loyalty;
pub mod admin;
pub mod upload;
pub mod catalog;
pub mod quest;
pub mod happy_hour;
pub mod garden;
pub mod referrals;
pub mod tech_tree;
pub mod cart;
pub mod cache;
#[cfg(feature = "utoipa")]
pub mod openapi;

use axum::{
    Router,
    routing::get,
    response::Json,
    extract::DefaultBodyLimit,
};
use axum::http::StatusCode;
use serde_json::{json, Value};
use crate::AppState;

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
    use super::validate_url;
    use axum::http::StatusCode;

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
        assert_eq!(validate_url(&Some("javascript:alert(1)".to_string())).unwrap_err(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn validate_url_rejects_data_text_html() {
        assert_eq!(validate_url(&Some("data:text/html,<script>alert(1)</script>".to_string())).unwrap_err(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn validate_url_rejects_ftp() {
        assert_eq!(validate_url(&Some("ftp://evil.com/file.jpg".to_string())).unwrap_err(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn validate_url_rejects_protocol_relative() {
        assert_eq!(validate_url(&Some("//evil.com/img.jpg".to_string())).unwrap_err(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn validate_url_rejects_too_long() {
        let long = "https://x.com/".to_string() + &"a".repeat(3000);
        assert_eq!(validate_url(&Some(long)).unwrap_err(), StatusCode::BAD_REQUEST);
    }
}
