use anyhow::Result;
use crate::config::Config;

#[allow(dead_code)]
pub async fn upload_to_s3(config: &Config, filename: &str, data: &[u8]) -> Result<String> {
    use aws_config::Region;
    use aws_sdk_s3::{config::Credentials, Client, primitives::ByteStream};

    let bucket = config.s3_bucket.as_deref().unwrap_or("");
    let endpoint = config.s3_endpoint.as_deref().unwrap_or("");
    let public_url = config.s3_public_url.as_deref().unwrap_or("");
    let region = config.s3_region.as_deref().unwrap_or("us-east-1");
    let access_key = config.s3_access_key.as_deref().unwrap_or("");
    let secret_key = config.s3_secret_key.as_deref().unwrap_or("");

    let creds = Credentials::new(access_key, secret_key, None, None, "static");
    let s3_config = aws_sdk_s3::config::Builder::new()
        .region(Region::new(region.to_string()))
        .credentials_provider(creds)
        .endpoint_url(endpoint)
        .force_path_style(true)
        .build();

    let client = Client::from_conf(s3_config);
    let safe_name = {
        let mut s = filename
            .replace("..", "_")
            .replace(['/', '\\', '\0'], "_");
        if s.len() > 255 { s.truncate(255); }
        s
    };
    let key = format!("uploads/{}", safe_name);

    client
        .put_object()
        .bucket(bucket)
        .key(&key)
        .body(ByteStream::from(data.to_vec()))
        .content_type(mime_from_filename(filename))
        .send()
        .await?;

    Ok(format!("{}/{}/{}", public_url, bucket, key))
}

#[allow(dead_code)]
fn mime_from_filename(filename: &str) -> &'static str {
    match filename.rsplit('.').next().unwrap_or("").to_lowercase().as_str() {
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
    use super::mime_from_filename;

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
        assert_eq!(mime_from_filename("archive.tar.gz"), "application/octet-stream");
        assert_eq!(mime_from_filename("video.min.mp4"), "video/mp4");
    }
}
