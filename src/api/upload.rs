use axum::{
    extract::{Multipart, State},
    http::StatusCode,
    routing::post,
    Json, Router,
};
use serde_json::{json, Value};

use crate::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/upload", post(upload_file))
}

async fn upload_file(
    State(_): State<AppState>,
    mut multipart: Multipart,
) -> Result<Json<Value>, StatusCode> {
    while let Some(field) = multipart.next_field().await.map_err(|_| StatusCode::BAD_REQUEST)? {
        let filename = field.file_name().unwrap_or("upload").to_string();
        let data = field.bytes().await.map_err(|_| StatusCode::BAD_REQUEST)?;

        // Save locally (S3 disabled for now)
        let path = format!("/data/uploads/{}", filename);
        tokio::fs::create_dir_all("/data/uploads").await.ok();
        tokio::fs::write(&path, &data).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        return Ok(Json(json!({
            "url": format!("/uploads/{}", filename),
            "filename": filename
        })));
    }
    Err(StatusCode::BAD_REQUEST)
}
