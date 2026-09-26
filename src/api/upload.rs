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

// ── Reading a local upload back: only what the rental data references ──────
//
// Owner, 2026-09-26, asked about the previous shop's media still reachable by
// a direct link, verbatim: «Зачем они вообще нужны мне?». The operator reads it
// as: stop serving them. Deleting the files themselves is the owner's own
// irreversible act and is not done here: nothing below deletes, moves or
// rewrites a file, and no row is written.
//
// The local branch above still writes to this folder whenever no object store
// is configured, so the read path cannot simply go. Until this change
// `src/main.rs` served the whole folder, ungated, to anyone who knew a name.
// Since then a name is served only when a bike's stored `image_url` is exactly
// `/uploads/<name>` -- the value the local branch returns and the admin's bike
// form stores. Every other name answers 404, as for a file that does not exist:
// what the previous shop left on the volume, and anything else nothing in the
// rental catalogue points at. The object store is not this server's to gate:
// its objects are fetched from the bucket's own public address, under the same
// `uploads/` key prefix both shops' code has written since the first commit
// (`src/s3.rs`), so which of them the previous shop used cannot be told by
// code. `specs/turbobaby/upload_media.t27` records both halves.

/// The folder the local branch writes to and the only folder
/// `/uploads/<name>` reads from.
pub(crate) const LOCAL_UPLOAD_DIR: &str = "/data/uploads";

/// The longest upload name read back: the sanitiser's clamp in `src/s3.rs`.
const SERVED_UPLOAD_NAME_MAX_CHARS: usize = 255;

/// Whether `name` can be an upload's file name at all: one path segment of
/// ASCII letters, digits, `.`, `-` and `_`, not starting with a dot. The local
/// branch writes eight hex digits, a dot and an extension. Anything else --
/// a separator, a traversal, a hidden file -- is refused before the database
/// or the disk is asked.
pub(crate) fn is_servable_upload_name(name: &str) -> bool {
    !name.is_empty()
        && name.chars().count() <= SERVED_UPLOAD_NAME_MAX_CHARS
        && !name.starts_with('.')
        && !name.contains("..")
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_')
}

/// The stored reference a bike holds to the local upload `name`: the value the
/// local branch of [`upload_file`] returns.
pub(crate) fn local_upload_reference(name: &str) -> String {
    format!("/uploads/{name}")
}

/// Whether the rental data references the local upload `name`: some bike's
/// stored `image_url` is exactly [`local_upload_reference`]. The bikes are the
/// rental catalogue, and their picture is the one media column a rental row
/// holds.
pub(crate) async fn rental_references_upload(
    orm: &sea_orm::DatabaseConnection,
    name: &str,
) -> Result<bool, sea_orm::DbErr> {
    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    let row = orm
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT EXISTS (SELECT 1 FROM bikes WHERE image_url = $1) AS referenced",
            [local_upload_reference(name).into()],
        ))
        .await?;
    Ok(row
        .and_then(|r| r.try_get::<bool>("", "referenced").ok())
        .unwrap_or(false))
}

/// The `/uploads` service `src/main.rs` nests: [`LOCAL_UPLOAD_DIR`], read
/// back only for a name the rental data references.
#[allow(dead_code)] // Called from src/main.rs, which compiles its own module tree.
pub(crate) fn served_uploads(db: std::sync::Arc<crate::db::Database>) -> Router {
    served_uploads_from(db, LOCAL_UPLOAD_DIR)
}

/// [`served_uploads`] over `dir`, so a test can point it at a folder of its own.
/// The folder is served as before, and every request passes
/// [`only_referenced_uploads`] first.
pub(crate) fn served_uploads_from(db: std::sync::Arc<crate::db::Database>, dir: &str) -> Router {
    Router::new()
        .fallback_service(tower_http::services::ServeDir::new(dir))
        .layer(axum::middleware::from_fn_with_state(
            db,
            only_referenced_uploads,
        ))
}

/// The gate in front of the folder: a request reaches it only for a
/// well-formed name the rental data references, and is otherwise answered
/// exactly as a missing file is. The path is read as sent, so a name that
/// needs percent-encoding is refused rather than decoded. A gate in front of
/// the directory service, rather than a handler of its own, keeps everything
/// that service already answered for a file it serves: its media type, its
/// ranges and its conditional requests.
async fn only_referenced_uploads(
    State(db): State<std::sync::Arc<crate::db::Database>>,
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    let path = request.uri().path().to_string();
    let name = path.strip_prefix('/').unwrap_or_default();
    if !is_servable_upload_name(name) {
        return StatusCode::NOT_FOUND.into_response();
    }
    match rental_references_upload(&db.orm, name).await {
        Ok(true) => next.run(request).await,
        Ok(false) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => {
            tracing::error!("uploads read: reference lookup failed: {e}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        content_matches_extension, validate_extension, validate_filename, ALLOWED_EXTENSIONS,
        MAX_UPLOAD_SIZE,
    };

    /// Regression pin for W-91 (Wave #56): `err(...)` is the only client-facing
    /// String-message error sink in the whole API (other handlers return a bare
    /// StatusCode). Its messages must NOT interpolate the raw error binding —
    /// raw AWS-SDK / IO / fs errors belong in the server-side `tracing::error!`,
    /// not the HTTP response body (OWASP improper error handling). This scans
    /// the production part of this file for a `format!` that interpolates the
    /// error var `e` outside a logging macro.
    #[test]
    fn upload_responses_never_interpolate_raw_errors() {
        let src = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/api/upload.rs"),
        )
        .expect("read upload.rs");
        // Only the production code (the test module legitimately discusses `e`).
        let prod = &src[..src.find("#[cfg(test)]").unwrap_or(src.len())];
        let offenders: Vec<(usize, &str)> = prod
            .lines()
            .enumerate()
            .filter(|(_, l)| {
                let t = l.trim_start();
                // Skip server-side logs — those SHOULD carry the detail.
                !t.starts_with("tracing::")
                    && l.contains("format!")
                    && (l.contains(", e)") || l.contains("{e}") || l.contains("{:?}\", e"))
            })
            .map(|(i, l)| (i + 1, l.trim()))
            .collect();
        assert!(
            offenders.is_empty(),
            "upload err()/response messages must not interpolate the raw error \
             (leaks internals; log it via tracing::error! instead). Offenders:\n  {}",
            offenders
                .iter()
                .map(|(n, l)| format!("L{n}: {l}"))
                .collect::<Vec<_>>()
                .join("\n  ")
        );
    }
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

    // ── Reading a local upload back (owner, 2026-09-26) ─────────────────

    #[test]
    fn a_name_the_local_branch_writes_can_be_read_back_and_nothing_else_can() {
        use super::{is_servable_upload_name, local_upload_reference};
        // Eight hex digits and an extension, as the local branch names a file.
        for name in [
            "7f3a9c2e.jpg",
            "0badf00d.webp",
            "a1b2c3d4.mp4",
            "stored_name-2.png",
        ] {
            assert!(is_servable_upload_name(name), "{name}");
            assert_eq!(local_upload_reference(name), format!("/uploads/{name}"));
        }
        for name in [
            "",
            ".",
            "..",
            ".hidden.jpg",
            "../7f3a9c2e.jpg",
            "a..b.jpg",
            "dir/7f3a9c2e.jpg",
            "dir\\7f3a9c2e.jpg",
            "7f3a9c2e.jpg%2F",
            "7f3a9c2e .jpg",
            "фото.jpg",
            "nul\0.jpg",
        ] {
            assert!(!is_servable_upload_name(name), "{name:?}");
        }
        assert!(is_servable_upload_name(&"a".repeat(255)));
        assert!(!is_servable_upload_name(&"a".repeat(256)));
    }

    /// Against a real database: a file is served only when a bike's stored
    /// picture is exactly `/uploads/<name>`; every other name, a file on the
    /// disk or not, answers 404. Writes two small files under the system temp
    /// folder (run with `TMP` pointed at a scratch folder) and one bike row,
    /// which it removes again.
    #[tokio::test]
    #[ignore = "needs DATABASE_URL env var; run with --ignored"]
    async fn only_a_file_a_bike_references_is_served() {
        use axum::body::Body;
        use axum::http::Request;
        use sea_orm::{ConnectionTrait, DbBackend, Statement};
        let Ok(url) = std::env::var("DATABASE_URL") else {
            return;
        };
        assert!(
            url.contains("127.0.0.1") || url.contains("localhost") || url.contains("test"),
            "refusing to run against {url:?}: this test writes a row"
        );
        let db = crate::db::Database::connect(&url).await.expect("connect");
        db.run_migrations().await.expect("migrate");
        let db = std::sync::Arc::new(db);

        let dir = std::env::temp_dir().join("uploads-read-test");
        std::fs::create_dir_all(&dir).expect("a scratch folder");
        let (referenced, stray) = ("5eed0001.jpg", "5eed0002.jpg");
        std::fs::write(dir.join(referenced), b"rental picture").expect("write");
        std::fs::write(dir.join(stray), b"stored by nobody we know").expect("write");

        let key = "uploads-read-test";
        let exec = |sql: &'static str, values: Vec<sea_orm::Value>| {
            let db = db.clone();
            async move {
                db.orm
                    .execute(Statement::from_sql_and_values(
                        DbBackend::Postgres,
                        sql,
                        values,
                    ))
                    .await
                    .expect(sql);
            }
        };
        exec("DELETE FROM bikes WHERE key = $1", vec![key.into()]).await;
        exec(
            "INSERT INTO bikes (key, brand, model, class, body, displacement_cc, offered, image_url) \
             VALUES ($1, 'Test', 'Upload read', 'scooter', 'scooter', 125, FALSE, $2)",
            vec![key.into(), format!("/uploads/{referenced}").into()],
        )
        .await;

        let get = |name: &str| {
            let service = super::served_uploads_from(db.clone(), dir.to_str().expect("utf-8"));
            let request = Request::builder()
                .uri(format!("/{name}"))
                .body(Body::empty())
                .expect("request");
            async move {
                tower::ServiceExt::oneshot(service, request)
                    .await
                    .expect("answer")
            }
        };
        let served = get(referenced).await;
        assert_eq!(served.status(), StatusCode::OK);
        let bytes = http_body_util::BodyExt::collect(served.into_body())
            .await
            .expect("body")
            .to_bytes();
        assert_eq!(&bytes[..], b"rental picture");
        // On the disk, referenced by nothing: not served.
        assert_eq!(get(stray).await.status(), StatusCode::NOT_FOUND);
        // Referenced by nothing and not on the disk; a traversal; a bad name.
        for name in ["5eed0003.jpg", "..%2F5eed0001.jpg", ".hidden"] {
            assert_eq!(get(name).await.status(), StatusCode::NOT_FOUND, "{name}");
        }

        exec("DELETE FROM bikes WHERE key = $1", vec![key.into()]).await;
        // With the reference gone, the same file is no longer served.
        assert_eq!(get(referenced).await.status(), StatusCode::NOT_FOUND);
    }
}
