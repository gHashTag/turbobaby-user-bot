use axum::{
    extract::{DefaultBodyLimit, Multipart, State},
    http::{HeaderMap, StatusCode},
    routing::post,
    Json, Router,
};
use serde_json::{json, Value};

use crate::api::auth::check_admin;
use crate::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/upload", post(upload_file))
        .layer(DefaultBodyLimit::max(110 * 1024 * 1024))
}

const MAX_UPLOAD_SIZE: usize = 100 * 1024 * 1024; // 100 MB
const ALLOWED_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "gif", "webp", "mp4", "mov", "webm"];

fn validate_upload(filename: &str, data: &[ u8]) -> Result<(), StatusCode> {
    if filename.len() > 500 { return Err(StatusCode::BAD_REQUEST); }
    if data.is_empty() { return Err(StatusCode::BAD_REQUEST); }
    if data.len() > MAX_UPLOAD_SIZE {
        tracing::warn!("upload file too large: {} bytes", data.len());
        return Err(StatusCode::PAYLOAD_TOO_LARGE);
    }
    let ext = filename.rsplit('.').next().unwrap_or("").to_lowercase();
    if ext.len() > 50 {
        return Err(StatusCode::BAD_REQUEST);
    }
    if !ALLOWED_EXTENSIONS.contains(&ext.as_str()) {
        tracing::warn!("upload disallowed extension: {}", ext);
        return Err(StatusCode::UNSUPPORTED_MEDIA_TYPE);
    }
    Ok(())
}

async fn upload_file(
    headers: HeaderMap,
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<Json<Value>, StatusCode> {
    check_admin(&headers, &state)?;

    let field = match multipart.next_field().await {
        Ok(Some(f)) => f,
        Ok(None) => {
            tracing::warn!("upload: no fields in multipart");
            return Err(StatusCode::BAD_REQUEST);
        }
        Err(e) => {
            tracing::error!("upload next_field error: {:?}", e);
            return Err(StatusCode::BAD_REQUEST);
        }
    };

    let filename = field.file_name().unwrap_or("upload").to_string();
    let data = field.bytes().await.map_err(|e| {
        tracing::error!("upload field.bytes() error: {:?}", e);
        StatusCode::BAD_REQUEST
    })?;
    validate_upload(&filename, &data)?;
    let ext = filename.rsplit('.').next().unwrap_or("").to_lowercase();

    let short_id = uuid::Uuid::new_v4().to_string();
    let safe_name = format!("{}.{}", short_id.get(0..8).unwrap_or(&short_id), ext);

    // Prefer S3 when configured — local /data/uploads is ephemeral on Railway
    // (no persistent volume) and vanishes on every redeploy.
    if state.config.s3_enabled() {
        // Wrap the S3 upload in a hard timeout so a stuck PutObject can never
        // hang the worker long enough for Railway's edge to return 502 or for
        // the healthcheck to mark the container unhealthy.
        let s3_fut = crate::s3::upload_to_s3(&state.config, &safe_name, data.clone());
        match tokio::time::timeout(std::time::Duration::from_secs(90), s3_fut).await {
            Ok(Ok(url)) => {
                tracing::info!("upload success (s3): url={}", url);
                return Ok(Json(json!({
                    "url": url,
                    "filename": safe_name
                })));
            }
            Ok(Err(e)) => {
                tracing::error!("upload s3 error: {:?}", e);
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            }
            Err(_elapsed) => {
                tracing::error!(
                    "upload s3 timeout after 90s: filename={} size={}",
                    safe_name,
                    data.len()
                );
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            }
        }
    }

    let path = format!("/data/uploads/{}", safe_name);

    if let Err(e) = tokio::fs::create_dir_all("/data/uploads").await {
        tracing::error!("upload create_dir_all error: {:?}", e);
    }

    match tokio::fs::write(&path, &data).await {
        Ok(_) => {
            let url = format!("/uploads/{}", safe_name);
            tracing::info!("upload success (local): url={}", url);
            Ok(Json(json!({
                "url": url,
                "filename": safe_name
            })))
        }
        Err(e) => {
            tracing::error!("upload file write error: {:?}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{validate_upload, ALLOWED_EXTENSIONS, MAX_UPLOAD_SIZE};
    use axum::http::StatusCode;

    #[test]
    fn test_validate_upload_ok() {
        assert!(validate_upload("photo.jpg", &[0u8; 100]).is_ok());
    }

    #[test]
    fn test_validate_upload_empty_file() {
        assert_eq!(validate_upload("photo.jpg", &[]).unwrap_err(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn test_validate_upload_name_too_long() {
        let name = "a".repeat(501) + ".jpg";
        assert_eq!(validate_upload(&name, &[0u8; 100]).unwrap_err(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn test_validate_upload_too_large() {
        assert_eq!(validate_upload("photo.jpg", &vec![0u8; MAX_UPLOAD_SIZE + 1]).unwrap_err(), StatusCode::PAYLOAD_TOO_LARGE);
    }

    #[test]
    fn test_validate_upload_disallowed_ext() {
        assert_eq!(validate_upload("photo.exe", &[0u8; 100]).unwrap_err(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
    }

    #[test]
    fn test_validate_upload_no_ext() {
        assert_eq!(validate_upload("photo", &[0u8; 100]).unwrap_err(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
    }

    #[test]
    fn test_validate_upload_ext_too_long() {
        let name = format!("photo.{}", "a".repeat(51));
        assert_eq!(validate_upload(&name, &[0u8; 100]).unwrap_err(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn test_allowed_extensions_coverage() {
        for ext in ALLOWED_EXTENSIONS {
            assert!(validate_upload(&format!("file.{}", ext), &[0u8; 10]).is_ok(), "ext {} should be allowed", ext);
        }
    }
}
