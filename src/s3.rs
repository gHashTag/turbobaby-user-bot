use anyhow::Result;
use crate::config::Config;

#[allow(dead_code)]
pub async fn upload_to_s3(config: &Config, filename: &str, data: &[u8]) -> Result<String> {
    use aws_config::Region;
    use aws_sdk_s3::{config::Credentials, Client, primitives::ByteStream};

    let bucket = config.s3_bucket.as_ref().map(|s| s.as_str()).unwrap_or("");
    let endpoint = config.s3_endpoint.as_ref().map(|s| s.as_str()).unwrap_or("");
    let public_url = config.s3_public_url.as_ref().map(|s| s.as_str()).unwrap_or("");
    let region = config.s3_region.as_ref().map(|s| s.as_str()).unwrap_or("us-east-1");
    let access_key = config.s3_access_key.as_ref().map(|s| s.as_str()).unwrap_or("");
    let secret_key = config.s3_secret_key.as_ref().map(|s| s.as_str()).unwrap_or("");

    let creds = Credentials::new(access_key, secret_key, None, None, "static");
    let s3_config = aws_sdk_s3::config::Builder::new()
        .region(Region::new(region.to_string()))
        .credentials_provider(creds)
        .endpoint_url(endpoint)
        .force_path_style(true)
        .build();

    let client = Client::from_conf(s3_config);
    let key = format!("uploads/{}", filename);

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
    match filename.rsplit('.').next().unwrap_or("") {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "webp" => "image/webp",
        "gif" => "image/gif",
        _ => "application/octet-stream",
    }
}
