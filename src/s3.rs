use crate::config::Config;
use anyhow::Result;
use bytes::Bytes;

pub async fn upload_to_s3(config: &Config, filename: &str, data: Bytes) -> Result<String> {
    use aws_config::{BehaviorVersion, Region};
    use aws_sdk_s3::{config::Credentials, primitives::ByteStream, Client};

    let bucket = config.s3_bucket.as_deref().unwrap_or("");
    // Prefer the Railway-private endpoint for the S3 client itself so the
    // upload travels over the internal network. Fall back to the public one
    // when S3_INTERNAL_ENDPOINT is unset.
    let endpoint = config
        .s3_internal_endpoint
        .as_deref()
        .filter(|s| !s.is_empty())
        .or(config.s3_endpoint.as_deref())
        .unwrap_or("");
    let public_url = config.s3_public_url.as_deref().unwrap_or("");
    let region = config.s3_region.as_deref().unwrap_or("us-east-1");
    let access_key = config.s3_access_key.as_deref().unwrap_or("");
    let secret_key = config.s3_secret_key.as_deref().unwrap_or("");

    let creds = Credentials::new(access_key, secret_key, None, None, "static");
    // BehaviorVersion is required by aws-config 1.x and aws-sdk-s3 1.x to
    // avoid a runtime panic when none is configured.
    //
    // CRITICAL: aws-sdk-s3 is built with default-features = false, which omits
    // the default `rt-tokio` feature that auto-installs an async sleep impl.
    // Without a sleep_impl the SDK PANICS at request time ("An async sleep
    // implementation is required for retry to work"), which kills the worker
    // thread and surfaces as an HTTP 502. Install TokioSleep explicitly.
    use aws_smithy_async::rt::sleep::{SharedAsyncSleep, TokioSleep};
    let s3_config = aws_sdk_s3::config::Builder::new()
        .behavior_version(BehaviorVersion::latest())
        .sleep_impl(SharedAsyncSleep::new(TokioSleep::new()))
        .region(Region::new(region.to_string()))
        .credentials_provider(creds)
        .endpoint_url(endpoint)
        .force_path_style(true)
        .build();

    let client = Client::from_conf(s3_config);
    // Cycle #75: was inline duplicate that used `s.truncate(255)` —
    // panics if byte 255 lands mid-codepoint (cyrillic, emoji file
    // names trigger it). Module-level `safe_name` routes through
    // `util::truncate_string` which counts chars, not bytes.
    let key = format!("uploads/{}", safe_name(filename));
    let size = data.len();
    let content_type = mime_from_filename(filename);

    tracing::info!(
        "s3: PutObject begin bucket={} key={} size={} content_type={} endpoint={} region={}",
        bucket,
        key,
        size,
        content_type,
        endpoint,
        region
    );

    // ByteStream::from(Bytes) reuses the underlying buffer — no extra copy
    // of the upload body, unlike ByteStream::from(Vec<u8>) from a slice.
    let resp = client
        .put_object()
        .bucket(bucket)
        .key(&key)
        .body(ByteStream::from(data))
        .content_type(content_type)
        .send()
        .await;

    match resp {
        Ok(out) => {
            tracing::info!(
                "s3: PutObject ok bucket={} key={} size={} etag={:?}",
                bucket,
                key,
                size,
                out.e_tag()
            );
        }
        Err(e) => {
            tracing::error!(
                "s3: PutObject error bucket={} key={} size={} err={:?}",
                bucket,
                key,
                size,
                e
            );
            return Err(e.into());
        }
    }

    let url = build_s3_public_url(public_url, bucket, &key);
    tracing::info!("s3: public url={}", url);
    Ok(url)
}

fn build_s3_public_url(public_url: &str, bucket: &str, key: &str) -> String {
    let base = public_url.trim_end_matches('/');
    if base.is_empty() {
        format!("s3://{}/{}", bucket, key)
    } else {
        format!("{}/{}/{}", base, bucket, key)
    }
}

/// Sanitise an upload filename: strip path separators, traversal, query/
/// fragment/percent, and clamp to 255 *chars* (not bytes) so a long
/// cyrillic or emoji name doesn't panic mid-codepoint.
fn safe_name(filename: &str) -> String {
    let s = filename
        .replace("..", "_")
        .replace(['/', '\\', '\0', '?', '#', '%'], "_")
        .replace(' ', "_");
    crate::util::truncate_string(&s, 255)
}

fn mime_from_filename(filename: &str) -> &'static str {
    match filename
        .rsplit('.')
        .next()
        .unwrap_or("")
        .to_lowercase()
        .as_str()
    {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "webp" => "image/webp",
        "gif" => "image/gif",
        "mp4" => "video/mp4",
        "mov" => "video/quicktime",
        "webm" => "video/webm",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::{build_s3_public_url, mime_from_filename, safe_name};

    #[test]
    fn test_mime_from_filename_lowercase() {
        assert_eq!(mime_from_filename("photo.jpg"), "image/jpeg");
        assert_eq!(mime_from_filename("clip.mp4"), "video/mp4");
        assert_eq!(mime_from_filename("data.bin"), "application/octet-stream");
    }

    #[test]
    fn test_mime_from_filename_uppercase() {
        assert_eq!(mime_from_filename("photo.JPG"), "image/jpeg");
        assert_eq!(mime_from_filename("clip.MP4"), "video/mp4");
        assert_eq!(mime_from_filename("image.PNG"), "image/png");
    }

    #[test]
    fn test_mime_from_filename_mixed_case() {
        assert_eq!(mime_from_filename("anim.WebP"), "image/webp");
        assert_eq!(mime_from_filename("MOVIE.mOV"), "video/quicktime");
    }

    #[test]
    fn test_mime_from_filename_no_ext() {
        assert_eq!(mime_from_filename("unknown"), "application/octet-stream");
    }

    #[test]
    fn test_mime_from_filename_multiple_dots() {
        assert_eq!(
            mime_from_filename("archive.tar.gz"),
            "application/octet-stream"
        );
        assert_eq!(mime_from_filename("video.min.mp4"), "video/mp4");
    }

    #[test]
    fn test_build_s3_public_url_basic() {
        assert_eq!(
            build_s3_public_url("https://cdn.example.com", "bucket", "uploads/file.jpg"),
            "https://cdn.example.com/bucket/uploads/file.jpg"
        );
    }

    #[test]
    fn test_build_s3_public_url_trailing_slash() {
        assert_eq!(
            build_s3_public_url("https://cdn.example.com/", "bucket", "uploads/file.jpg"),
            "https://cdn.example.com/bucket/uploads/file.jpg"
        );
    }

    #[test]
    fn test_build_s3_public_url_multiple_slashes() {
        assert_eq!(
            build_s3_public_url("https://cdn.example.com//", "bucket", "uploads/file.jpg"),
            "https://cdn.example.com/bucket/uploads/file.jpg"
        );
    }

    #[test]
    fn test_build_s3_public_url_empty_falls_back() {
        assert_eq!(
            build_s3_public_url("", "bucket", "uploads/file.jpg"),
            "s3://bucket/uploads/file.jpg"
        );
    }

    #[test]
    fn test_safe_name_traversal() {
        assert!(!safe_name("../../../etc/passwd").contains(".."));
        assert!(!safe_name("../../../etc/passwd").contains('/'));
    }

    #[test]
    fn test_safe_name_query_and_space() {
        assert_eq!(safe_name("file?name#hash%20.txt"), "file_name_hash_20.txt");
        assert_eq!(safe_name("my file.jpg"), "my_file.jpg");
    }

    #[test]
    fn test_safe_name_long() {
        let long = "a".repeat(300);
        assert_eq!(safe_name(&long).len(), 255);
    }

    #[test]
    fn test_safe_name_unicode_long() {
        let long = "🔥".repeat(300);
        let result = safe_name(&long);
        assert_eq!(result.chars().count(), 255);
    }

    #[test]
    fn test_safe_name_cyrillic_boundary_does_not_panic() {
        // Regression for cycle #75: the inline `s.truncate(255)` panicked
        // when byte 255 landed mid-codepoint. Cyrillic chars are 2 bytes
        // in UTF-8, so a 200-char Russian filename is 400 bytes — when
        // truncating to 255 *bytes*, byte 255 falls inside a codepoint.
        // After the fix `safe_name` routes through `truncate_string`,
        // which counts chars; this call must succeed and the result must
        // contain only valid UTF-8 (which is invariant for `String` but
        // we still assert char count for documentation).
        let name = "файл".repeat(200); // 4 chars × 200 = 800 chars / 1600 bytes
        let result = safe_name(&name);
        assert!(result.chars().count() <= 255);
        // Sanity: result still recognisable as cyrillic.
        assert!(result.starts_with("файл"));
    }

    #[test]
    fn test_safe_name_emoji_boundary_does_not_panic() {
        // Emoji are 4 bytes in UTF-8 — 255 / 4 = 63 remainder 3, so byte
        // 255 always falls mid-codepoint for a long enough emoji run.
        // Pre-fix, this would have panicked via the old inline path.
        let name = "🔥".repeat(100);
        let result = safe_name(&name);
        assert!(result.chars().count() <= 255);
    }
}
