use axum::{
    extract::{Multipart, State},
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
}

const MAX_UPLOAD_SIZE: usize = 10 * 1024 * 1024; // 10 MB
const ALLOWED_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "gif", "webp", "mp4", "mov", "webm"];

async fn upload_file(
    headers: HeaderMap,
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<Json<Value>, StatusCode> {
    check_admin(&headers, &state)?;
    while let Some(field) = multipart.next_field().await.map_err(|_| StatusCode::BAD_REQUEST)? {
        let filename = field.file_name().unwrap_or("upload").to_string();
        let data = field.bytes().await.map_err(|_| StatusCode::BAD_REQUEST)?;

        // Size limit
        if data.len() > MAX_UPLOAD_SIZE {
            return Err(StatusCode::PAYLOAD_TOO_LARGE);
        }

        // Extension check
        let ext = filename.rsplit('.').next().unwrap_or("").to_lowercase();
        if !ALLOWED_EXTENSIONS.contains(&ext.as_str()) {
            return Err(StatusCode::UNSUPPORTED_MEDIA_TYPE);
        }

        // Path traversal protection
        let safe_name = filename.replace('/', "_").replace("\\", "_").replace("..", "_");
        let short_id = uuid::Uuid::new_v4().to_string();
        let unique_name = format!("{}-{}", &short_id[..8], safe_name);

        // Save locally (S3 disabled for now)
        let path = format!("/data/uploads/{}", unique_name);
        tokio::fs::create_dir_all("/data/uploads").await.ok();
        tokio::fs::write(&path, &data).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        return Ok(Json(json!({
            "url": format!("/uploads/{}", unique_name),
            "filename": unique_name
        })));
    }
    Err(StatusCode::BAD_REQUEST)
}
