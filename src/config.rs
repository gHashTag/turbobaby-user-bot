use anyhow::{Context, Result};

#[derive(Debug, Clone)]
pub struct Config {
    pub bot_token: String,
    pub web_app_url: String,
    pub port: u16,
    pub webhook_path: String,
    pub app_url: String,
    pub is_production: bool,
    pub admin_ids: Vec<i64>,
    pub bot_username: String,
    pub database_url: String,
    pub grok_api_key: String,
    pub glm_api_key: String,
    pub s3_bucket: Option<String>,
    pub s3_endpoint: Option<String>,
    pub s3_public_url: Option<String>,
    pub s3_region: Option<String>,
    pub s3_access_key: Option<String>,
    pub s3_secret_key: Option<String>,
    pub backup_assets: bool,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        dotenvy::dotenv().ok();

        let bot_token = std::env::var("BOT_TOKEN")
            .context("BOT_TOKEN not set")?;

        let admin_ids = std::env::var("ADMIN_IDS")
            .unwrap_or_default()
            .split(',')
            .filter_map(|s| s.trim().parse::<i64>().ok())
            .collect();

        Ok(Self {
            bot_token,
            web_app_url: std::env::var("WEB_APP_URL").unwrap_or_default(),
            port: std::env::var("PORT").unwrap_or("3000".into()).parse().unwrap_or(3000),
            webhook_path: std::env::var("WEBHOOK_PATH").unwrap_or("/webhook".into()),
            app_url: std::env::var("APP_URL").unwrap_or_default(),
            is_production: std::env::var("NODE_ENV").unwrap_or_default() == "production"
                || std::env::var("RAILWAY_ENVIRONMENT").is_ok(),
            admin_ids,
            bot_username: std::env::var("BOT_USERNAME").unwrap_or("Woody_WeedPecker_bot".into()),
            database_url: std::env::var("DATABASE_URL").context("DATABASE_URL not set")?,
            grok_api_key: std::env::var("GROK_API_KEY").unwrap_or_default(),
            glm_api_key: std::env::var("GLM_API_KEY").unwrap_or_default(),
            s3_bucket: std::env::var("S3_BUCKET").ok(),
            s3_endpoint: std::env::var("S3_ENDPOINT").ok(),
            s3_public_url: std::env::var("S3_PUBLIC_URL").ok(),
            s3_region: std::env::var("S3_REGION").ok(),
            s3_access_key: std::env::var("S3_ACCESS_KEY_ID").ok(),
            s3_secret_key: std::env::var("S3_SECRET_ACCESS_KEY").ok(),
            backup_assets: std::env::var("BACKUP_ASSETS").unwrap_or("true".into()) != "false",
        })
    }

    pub fn s3_enabled(&self) -> bool {
        self.s3_bucket.is_some() && self.s3_endpoint.is_some()
    }
}
