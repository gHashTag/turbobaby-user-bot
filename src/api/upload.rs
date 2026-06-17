use axum::{
    extract::{DefaultBodyLimit, Multipart, State},
    http::{HeaderMap, StatusCode},
    routing::post,
    Json, Router,
};
use bytes::{Bytes, BytesMut};
use serde_json::{json, Value};
use std::time::Duration;

use crate::api::auth::check_admin;
use crate::api::rate_limit::{
    check_and_record, client_ip_from_headers, new_store, SlidingWindowStore,
};
use crate::AppState;

pub(crate) fn routes() -> Router<AppState> {
    Router::new()
        .route("/upload", post(upload_file))
        .layer(DefaultBodyLimit::max(110 * 1024 * 1024))
}

const MAX_UPLOAD_SIZE: usize = 100 * 1024 * 1024; // 100 MB
const ALLOWED_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "gif", "webp", "mp4", "mov", "webm"];

/// Per-IP sliding-window log: 30 uploads / hour. Admin-only endpoint but
/// rate-limited as defense-in-depth against leaked-token storage exhaustion.
static UPLOAD_RATE_LIMIT: std::sync::LazyLock<SlidingWindowStore> =
    std::sync::LazyLock::new(new_store);
const UPLOAD_RL_WINDOW: Duration = Duration::from_secs(60 * 60);
const UPLOAD_RL_MAX_ATTEMPTS: usize = 30;
const UPLOAD_RL_MAX_IPS: usize = 1_000;

type UploadError = (StatusCode, Json<Value>);

fn err(status: StatusCode, msg: impl Into<String>) -> UploadError {
    let msg = msg.into();
    (status, Json(json!({ "error": msg })))
}

/// Body-independent checks (filename length + extension allowlist). These
/// depend only on `field.file_name()`, so the handler runs them BEFORE
/// streaming the body — a disallowed/oversized-name upload is then rejected
/// without first buffering up to MAX_UPLOAD_SIZE (100 MB) into memory
/// (fail-fast / resource-exhaustion hardening).
fn validate_extension(filename: &str) -> Result<(), UploadError> {
    if filename.len() > 500 {
        return Err(err(StatusCode::BAD_REQUEST, "filename too long"));
    }
    let ext = filename.rsplit('.').next().unwrap_or("").to_lowercase();
    if ext.len() > 50 {
        return Err(err(StatusCode::BAD_REQUEST, "extension too long"));
    }
    if !ALLOWED_EXTENSIONS.contains(&ext.as_str()) {
        tracing::warn!("upload disallowed extension: {}", ext);
        return Err(err(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            format!("disallowed extension: .{}", ext),
        ));
    }
    Ok(())
}

fn validate_filename(filename: &str, size: usize) -> Result<(), UploadError> {
    validate_extension(filename)?;
    if size == 0 {
        return Err(err(StatusCode::BAD_REQUEST, "empty file"));
    }
    if size > MAX_UPLOAD_SIZE {
        tracing::warn!("upload file too large: {} bytes", size);
        return Err(err(
            StatusCode::PAYLOAD_TOO_LARGE,
            format!("file too large: {} bytes (max {})", size, MAX_UPLOAD_SIZE),
        ));
    }
    Ok(())
}

/// Verify the file's leading bytes (magic number) match the claimed extension.
/// OWASP advises against trusting the extension alone — it's attacker-supplied
/// and an `.png` can carry arbitrary bytes. Covers exactly the
/// `ALLOWED_EXTENSIONS` media types; returns false for too-short or mismatched
/// content (the caller then rejects). The signatures here are mandatory per
/// each format's spec, so a spec-compliant file is never a false negative.
fn content_matches_extension(data: &[u8], ext: &str) -> bool {
    let starts = |sig: &[u8]| data.len() >= sig.len() && &data[..sig.len()] == sig;
    match ext {
        "jpg" | "jpeg" => starts(&[0xFF, 0xD8, 0xFF]),
        "png" => starts(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]),
        "gif" => starts(b"GIF87a") || starts(b"GIF89a"),
        "webp" => data.len() >= 12 && &data[0..4] == b"RIFF" && &data[8..12] == b"WEBP",
        // ISO Base Media File Format (mp4/mov): `ftyp` box at byte offset 4.
        "mp4" | "mov" => data.len() >= 12 && &data[4..8] == b"ftyp",
        // Matroska / WebM: EBML header.
        "webm" => starts(&[0x1A, 0x45, 0xDF, 0xA3]),
        // Unknown ext can't reach here (allow-list gates it); fail closed.
        _ => false,
    }
}

async fn upload_file(
    headers: HeaderMap,
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<Json<Value>, UploadError> {
    tracing::info!("upload: request received at /api/upload");

    let admin_id = check_admin(&headers, &state).map_err(|status| {
        tracing::warn!("upload: admin auth failed status={}", status);
        err(status, "admin auth failed")
    })?;
    tracing::info!("upload: admin auth ok admin_id={}", admin_id);

    // Defense-in-depth rate-limit: even an authenticated admin can't spam.
    let client_ip = client_ip_from_headers(&headers);
    if !check_and_record(
        &UPLOAD_RATE_LIMIT,
        &client_ip,
        UPLOAD_RL_WINDOW,
        UPLOAD_RL_MAX_ATTEMPTS,
        UPLOAD_RL_MAX_IPS,
    )
    .await
    {
        crate::metrics::rate_limit_blocked("upload");
        tracing::warn!(
            "upload: rate-limit exceeded admin_id={} ip={}",
            admin_id,
            client_ip
        );
        return Err(err(
            StatusCode::TOO_MANY_REQUESTS,
            "upload rate limit exceeded",
        ));
    }

    // Walk multipart fields. We accept the first non-empty file-like field; log
    // every field we encounter so we can see in Railway logs what the client
    // actually sent if the wrong shape arrives.
    let mut chosen: Option<(String, BytesMut)> = None;
    loop {
        let next = multipart.next_field().await.map_err(|e| {
            tracing::error!("upload: next_field error: {:?}", e);
            // Generic client message; detail stays in the server log above
            // (don't leak parser internals in the response body).
            err(StatusCode::BAD_REQUEST, "invalid multipart request")
        })?;
        let mut field = match next {
            Some(f) => f,
            None => break,
        };
        let field_name = field.name().unwrap_or("").to_string();
        let filename = field.file_name().unwrap_or("").to_string();
        tracing::info!(
            "upload: multipart field name={:?} filename={:?}",
            field_name,
            filename
        );

        if filename.is_empty() {
            // Non-file field — drain and skip.
            while let Ok(Some(_)) = field.chunk().await {}
            continue;
        }

        // Fail-fast: reject a bad filename/extension BEFORE buffering the body,
        // so a disallowed upload never consumes up to MAX_UPLOAD_SIZE of memory.
        validate_extension(&filename)?;

        // Stream chunks into a single contiguous buffer. We avoid
        // `field.bytes()` because it internally collects the whole body via
        // `http_body_util::BodyExt::collect` which can fragment + double-copy.
        // Reading chunk-by-chunk lets us enforce the size cap incrementally
        // and abort before the buffer grows past MAX_UPLOAD_SIZE — that's the
        // safety net against OOM on Railway's small plan.
        let mut buf = BytesMut::new();
        tracing::info!("upload: begin streaming field filename={}", filename);
        loop {
            match field.chunk().await {
                Ok(Some(chunk)) => {
                    if buf.len().saturating_add(chunk.len()) > MAX_UPLOAD_SIZE {
                        tracing::warn!(
                            "upload: aborting — would exceed MAX_UPLOAD_SIZE (have {} + chunk {} > {})",
                            buf.len(),
                            chunk.len(),
                            MAX_UPLOAD_SIZE
                        );
                        return Err(err(
                            StatusCode::PAYLOAD_TOO_LARGE,
                            format!("file exceeds max size of {} bytes", MAX_UPLOAD_SIZE),
                        ));
                    }
                    buf.extend_from_slice(&chunk);
                }
                Ok(None) => break,
                Err(e) => {
                    tracing::error!(
                        "upload: chunk read error after {} bytes: {:?}",
                        buf.len(),
                        e
                    );
                    return Err(err(StatusCode::BAD_REQUEST, "upload read error"));
                }
            }
        }
        tracing::info!(
            "upload: finished streaming filename={} total_bytes={}",
            filename,
            buf.len()
        );
        chosen = Some((filename, buf));
        break;
    }

    let (filename, buf) = match chosen {
        Some(p) => p,
        None => {
            tracing::warn!("upload: no file field found in multipart");
            return Err(err(
                StatusCode::BAD_REQUEST,
                "no file field in multipart body",
            ));
        }
    };

    validate_filename(&filename, buf.len())?;
    let ext = filename.rsplit('.').next().unwrap_or("").to_lowercase();

    // OWASP: don't trust the extension alone — verify the magic bytes match,
    // so a renamed non-media file (or polyglot) can't be stored as an image.
    if !content_matches_extension(&buf, &ext) {
        tracing::warn!("upload: content does not match .{} signature", ext);
        return Err(err(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            format!("file content does not match .{} type", ext),
        ));
    }

    let short_id = uuid::Uuid::new_v4().to_string();
    let safe_name = format!("{}.{}", short_id.get(0..8).unwrap_or(&short_id), ext);

    // Freeze BytesMut -> Bytes is a zero-copy handoff: the same allocation is
    // moved into ByteStream so we never duplicate the payload.
    let data: Bytes = buf.freeze();
    let size = data.len();

    let s3_on = state.config.s3_enabled();
    tracing::info!(
        "upload: s3_enabled={} branch={} safe_name={} size={}",
        s3_on,
        if s3_on { "s3" } else { "local" },
        safe_name,
        size
    );

    // Prefer S3 when configured — local /data/uploads is ephemeral on Railway
    // (no persistent volume) and vanishes on every redeploy.
    if s3_on {
        let bucket = state.config.s3_bucket.as_deref().unwrap_or("");
        tracing::info!(
            "upload: calling upload_to_s3 bucket={} key=uploads/{} size={}",
            bucket,
            safe_name,
            size
        );
        // Wrap the S3 upload in a hard timeout so a stuck PutObject can never
        // hang the worker long enough for Railway's edge to return 502 or for
        // the healthcheck to mark the container unhealthy.
        let s3_fut = crate::s3::upload_to_s3(&state.config, &safe_name, data);
        match tokio::time::timeout(std::time::Duration::from_secs(90), s3_fut).await {
            Ok(Ok(url)) => {
                tracing::info!("upload: success (s3) url={} size={}", url, size);
                return Ok(Json(json!({
                    "url": url,
                    "filename": safe_name
                })));
            }
            Ok(Err(e)) => {
                tracing::error!("upload s3 error: {:?}", e);
                return Err(err(StatusCode::INTERNAL_SERVER_ERROR, "s3 upload failed"));
            }
            Err(_elapsed) => {
                tracing::error!(
                    "upload s3 timeout after 90s: filename={} size={}",
                    safe_name,
                    size
                );
                return Err(err(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "s3 upload timed out after 90s",
                ));
            }
        }
    }

    let path = format!("/data/uploads/{}", safe_name);
    tracing::info!("upload: writing to local path={} size={}", path, size);

    if let Err(e) = tokio::fs::create_dir_all("/data/uploads").await {
        tracing::error!("upload create_dir_all error: {:?}", e);
    }

    match tokio::fs::write(&path, &data).await {
        Ok(_) => {
            let url = format!("/uploads/{}", safe_name);
            tracing::info!("upload: success (local) url={} size={}", url, size);
            Ok(Json(json!({
                "url": url,
                "filename": safe_name
            })))
        }
        Err(e) => {
            tracing::error!("upload file write error: {:?}", e);
            Err(err(StatusCode::INTERNAL_SERVER_ERROR, "file write failed"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        content_matches_extension, validate_extension, validate_filename, ALLOWED_EXTENSIONS,
        MAX_UPLOAD_SIZE,
    };
    use axum::http::StatusCode;

    #[test]
    fn test_content_matches_extension_valid_signatures() {
        assert!(content_matches_extension(
            &[0xFF, 0xD8, 0xFF, 0xE0, 0x00],
            "jpg"
        ));
        assert!(content_matches_extension(&[0xFF, 0xD8, 0xFF], "jpeg"));
        assert!(content_matches_extension(
            &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00],
            "png"
        ));
        assert!(content_matches_extension(b"GIF89a....", "gif"));
        assert!(content_matches_extension(b"RIFF\0\0\0\0WEBPxx", "webp"));
        assert!(content_matches_extension(b"\0\0\0\x20ftypmp42", "mp4"));
        assert!(content_matches_extension(b"\0\0\0\x14ftypqt  ", "mov"));
        assert!(content_matches_extension(
            &[0x1A, 0x45, 0xDF, 0xA3, 0x00],
            "webm"
        ));
    }

    #[test]
    fn test_content_matches_extension_mismatch_and_short() {
        // A renamed text/exe file with a .png name must be rejected.
        assert!(!content_matches_extension(b"MZ\x90\x00 not a png", "png"));
        assert!(!content_matches_extension(b"<svg>...</svg>", "png"));
        // Wrong-but-real signature for the claimed ext.
        assert!(!content_matches_extension(&[0xFF, 0xD8, 0xFF], "png"));
        // Too short to carry the signature.
        assert!(!content_matches_extension(&[0xFF], "jpg"));
        assert!(!content_matches_extension(b"RIFF", "webp"));
        assert!(!content_matches_extension(b"", "gif"));
    }

    #[test]
    fn test_validate_upload_ok() {
        assert!(validate_filename("photo.jpg", 100).is_ok());
    }

    #[test]
    fn test_validate_extension_fail_fast() {
        // Body-independent gate (no size): allowed passes, disallowed is
        // rejected with the right status (so the handler can bail before
        // buffering the body).
        assert!(validate_extension("photo.png").is_ok());
        assert_eq!(
            validate_extension("malware.exe").unwrap_err().0,
            StatusCode::UNSUPPORTED_MEDIA_TYPE
        );
        assert_eq!(
            validate_extension("noext").unwrap_err().0,
            StatusCode::UNSUPPORTED_MEDIA_TYPE
        );
        assert_eq!(
            validate_extension(&("a".repeat(501) + ".png"))
                .unwrap_err()
                .0,
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn test_validate_upload_empty_file() {
        assert_eq!(
            validate_filename("photo.jpg", 0).unwrap_err().0,
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn test_validate_upload_name_too_long() {
        let name = "a".repeat(501) + ".jpg";
        assert_eq!(
            validate_filename(&name, 100).unwrap_err().0,
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn test_validate_upload_too_large() {
        assert_eq!(
            validate_filename("photo.jpg", MAX_UPLOAD_SIZE + 1)
                .unwrap_err()
                .0,
            StatusCode::PAYLOAD_TOO_LARGE
        );
    }

    #[test]
    fn test_validate_upload_disallowed_ext() {
        assert_eq!(
            validate_filename("photo.exe", 100).unwrap_err().0,
            StatusCode::UNSUPPORTED_MEDIA_TYPE
        );
    }

    #[test]
    fn test_validate_upload_no_ext() {
        assert_eq!(
            validate_filename("photo", 100).unwrap_err().0,
            StatusCode::UNSUPPORTED_MEDIA_TYPE
        );
    }

    #[test]
    fn test_validate_upload_ext_too_long() {
        let name = format!("photo.{}", "a".repeat(51));
        assert_eq!(
            validate_filename(&name, 100).unwrap_err().0,
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn test_allowed_extensions_coverage() {
        for ext in ALLOWED_EXTENSIONS {
            assert!(
                validate_filename(&format!("file.{}", ext), 10).is_ok(),
                "ext {} should be allowed",
                ext
            );
        }
    }
}
