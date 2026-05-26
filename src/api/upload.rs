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

const MAX_UPLOAD_SIZE: usize = 100 * 1024 * 1024; // 100 MB
const ALLOWED_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "gif", "webp", "mp4", "mov", "webm"];

async fn upload_file(
    headers: HeaderMap,
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<Json<Value>, StatusCode> {
    tracing::info!("=== UPLOAD START ===");
    tracing::info!("Headers: {:?}", headers);
    
    tracing::info!("Step 1: Checking admin auth...");
    match check_admin(&headers, &state) {
        Ok(admin_id) => tracing::info!("Step 1: Auth OK, admin_id={}", admin_id),
        Err(e) => {
            tracing::error!("Step 1: Auth FAILED — {:?}", e);
            return Err(e);
        }
    }
    
    tracing::info!("Step 2: Reading multipart fields...");
    let mut field_count = 0;
    while let Some(field) = multipart.next_field().await.map_err(|e| {
        tracing::error!("Step 2: Multipart next_field error: {:?}", e);
        StatusCode::BAD_REQUEST
    })? {
        field_count += 1;
        tracing::info!("Step 2: Found field #{}", field_count);
        
        let filename = field.file_name().unwrap_or("upload").to_string();
        tracing::info!("Step 3: filename='{}'", filename);
        
        tracing::info!("Step 4: Reading field bytes...");
        let data = field.bytes().await.map_err(|e| {
            tracing::error!("Step 4: field.bytes() error: {:?}", e);
            StatusCode::BAD_REQUEST
        })?;
        tracing::info!("Step 4: Read {} bytes", data.len());

        // Size limit
        tracing::info!("Step 5: Checking size limit ({} > {} ?)", data.len(), MAX_UPLOAD_SIZE);
        if data.len() > MAX_UPLOAD_SIZE {
            tracing::error!("Step 5: File too large! {} bytes", data.len());
            return Err(StatusCode::PAYLOAD_TOO_LARGE);
        }
        tracing::info!("Step 5: Size OK");

        // Extension check
        let ext = filename.rsplit('.').next().unwrap_or("").to_lowercase();
        tracing::info!("Step 6: Extension='{}' allowed={:?}", ext, ALLOWED_EXTENSIONS);
        if !ALLOWED_EXTENSIONS.contains(&ext.as_str()) {
            tracing::error!("Step 6: Extension '{}' NOT ALLOWED", ext);
            return Err(StatusCode::UNSUPPORTED_MEDIA_TYPE);
        }
        tracing::info!("Step 6: Extension OK");

        // Generate safe filename
        let short_id = uuid::Uuid::new_v4().to_string();
        let safe_name = format!("{}.{}", &short_id[..8], ext);
        tracing::info!("Step 7: Safe filename='{}'", safe_name);

        // Save locally
        let path = format!("/data/uploads/{}", safe_name);
        tracing::info!("Step 8: Creating dir /data/uploads...");
        match tokio::fs::create_dir_all("/data/uploads").await {
            Ok(_) => tracing::info!("Step 8: Dir created/exists"),
            Err(e) => tracing::error!("Step 8: create_dir_all error: {:?}", e),
        }
        
        tracing::info!("Step 9: Writing file to '{}' ({} bytes)...", path, data.len());
        match tokio::fs::write(&path, &data).await {
            Ok(_) => {
                tracing::info!("Step 9: File written OK");
                let url = format!("/uploads/{}", safe_name);
                tracing::info!("=== UPLOAD SUCCESS === url={}", url);
                return Ok(Json(json!({
                    "url": url,
                    "filename": safe_name
                })));
            }
            Err(e) => {
                tracing::error!("Step 9: File write FAILED: {:?}", e);
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            }
        }
    }
    
    tracing::error!("=== UPLOAD FAILED: no fields in multipart ===");
    Err(StatusCode::BAD_REQUEST)
}
