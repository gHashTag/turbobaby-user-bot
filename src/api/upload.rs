use axum::{
    extract::{Multipart, State},
    http::StatusCode,
    routing::post,
    Json, Router,
};
use serde_json::{json, Value};
use tracing::error;

use crate::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/upload", post(upload_file))
}

async fn upload_file(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<Json<Value>, StatusCode> {
    while let Some(field) = multipart.next_field().await.map_err(|_| StatusCode::BAD_REQUEST)? {
        let name = field.name().unwrap_or("").to_string();
        let filename = field.file_name().unwrap_or("upload").to_string();
        let data = field.bytes().await.map_err(|_| StatusCode::BAD_REQUEST)?;

        // If S3 enabled, upload there
        if state.config.s3_enabled() {
            match crate::s3::upload_to_s3(&state.config, &filename, &data).await {
                Ok(url) => return Ok(Json(json!({ "url": url, "filename": filename }))),
                Err(e) => {
                    error!("S3 upload failed: {}", e);
                    return Err(StatusCode::INTERNAL_SERVER_ERROR);
                }
            }
        } else {
            // Save locally
            let path = format!("/data/uploads/{}", filename);
            tokio::fs::create_dir_all("/data/uploads").await.ok();
            tokio::fs::write(&path, &data).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            return Ok(Json(json!({ "url": format!("/uploads/{}", filename), "filename": filename })));
        }
    }
    Err(StatusCode::BAD_REQUEST)
}
