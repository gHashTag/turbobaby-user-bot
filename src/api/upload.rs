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
    if filename.len() > 500 { return Err(StatusCode::BAD_REQUEST); }

    let data = field.bytes().await.map_err(|e| {
        tracing::error!("upload field.bytes() error: {:?}", e);
        StatusCode::BAD_REQUEST
    })?;

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

    let short_id = uuid::Uuid::new_v4().to_string();
    let safe_name = format!("{}.{}", short_id.get(0..8).unwrap_or(&short_id), ext);
    let path = format!("/data/uploads/{}", safe_name);

    if let Err(e) = tokio::fs::create_dir_all("/data/uploads").await {
        tracing::error!("upload create_dir_all error: {:?}", e);
    }

    match tokio::fs::write(&path, &data).await {
        Ok(_) => {
            let url = format!("/uploads/{}", safe_name);
            tracing::info!("upload success: url={}", url);
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
