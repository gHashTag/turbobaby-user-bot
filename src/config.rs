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
    /// Optional Railway-private endpoint used for the S3 client itself.
    /// When set, uploads go over the internal network (e.g. http://bucket.railway.internal:9000)
    /// while `s3_public_url` keeps producing browser-facing URLs over the public edge.
    /// Falls back to `s3_endpoint` when unset.
    pub s3_internal_endpoint: Option<String>,
    pub s3_public_url: Option<String>,
    pub s3_region: Option<String>,
    pub s3_access_key: Option<String>,
    pub s3_secret_key: Option<String>,
    pub backup_assets: bool,
    pub admin_password: Option<String>,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        dotenvy::dotenv().ok();

        let bot_token = std::env::var("BOT_TOKEN")
            .context("BOT_TOKEN not set")?
            .trim()
            .to_string();

        // Admin IDs from env; if ADMIN_IDS is set it is the only source of truth.
        // Hard-coded defaults are used ONLY as a fallback when ADMIN_IDS is empty
        // (local development before env is configured).
        let is_production = std::env::var("NODE_ENV").unwrap_or_default() == "production"
            || std::env::var("RAILWAY_ENVIRONMENT").is_ok();

        let env_ids: Vec<i64> = std::env::var("ADMIN_IDS")
            .unwrap_or_default()
            .split(',')
            .filter_map(|s| s.trim().parse::<i64>().ok())
            .collect();
        let admin_ids = if !env_ids.is_empty() {
            env_ids
        } else {
            tracing::warn!("ADMIN_IDS not set — using default admins 144022504, 8420420131");
            vec![144022504, 8420420131]
        };

        // The backend service serves BOTH the API and the WASM frontend on the same origin.
        // If WEB_APP_URL is empty or points to the broken legacy TMA service, fall back to the
        // canonical backend URL so the Telegram WebApp button always opens a working page.
        let raw_web_app_url = std::env::var("WEB_APP_URL").unwrap_or_default();
        let canonical_web_app_url = "https://woody-weed-bot-production.up.railway.app/".to_string();
        let web_app_url = if raw_web_app_url.trim().is_empty()
            || raw_web_app_url.contains("woody-woodpecker-tma-production")
        {
            canonical_web_app_url
        } else {
            raw_web_app_url
        };

        Ok(Self {
            bot_token,
            web_app_url,
            port: std::env::var("PORT")
                .unwrap_or("3000".into())
                .parse()
                .unwrap_or(3000),
            webhook_path: std::env::var("WEBHOOK_PATH").unwrap_or("/webhook".into()),
            app_url: std::env::var("APP_URL").unwrap_or_default(),
            is_production,
            admin_ids,
            bot_username: {
                let raw = std::env::var("BOT_USERNAME").unwrap_or("Woody_WeedPecker_bot".into());
                let trimmed = raw.trim();
                if trimmed.is_empty() {
                    "Woody_WeedPecker_bot".into()
                } else {
                    trimmed.to_string()
                }
            },
            database_url: std::env::var("DATABASE_URL").context("DATABASE_URL not set")?,
            grok_api_key: std::env::var("GROK_API_KEY").unwrap_or_default(),
            glm_api_key: std::env::var("GLM_API_KEY").unwrap_or_default(),
            s3_bucket: std::env::var("S3_BUCKET").ok(),
            s3_endpoint: std::env::var("S3_ENDPOINT").ok(),
            s3_internal_endpoint: std::env::var("S3_INTERNAL_ENDPOINT")
                .ok()
                .filter(|s| !s.trim().is_empty()),
            s3_public_url: std::env::var("S3_PUBLIC_URL").ok(),
            s3_region: std::env::var("S3_REGION").ok(),
            s3_access_key: std::env::var("S3_ACCESS_KEY_ID").ok(),
            s3_secret_key: std::env::var("S3_SECRET_ACCESS_KEY").ok(),
            backup_assets: std::env::var("BACKUP_ASSETS").unwrap_or("true".into()) != "false",
            admin_password: std::env::var("ADMIN_PASSWORD").ok(),
        })
    }

    pub fn s3_enabled(&self) -> bool {
        self.s3_bucket.is_some() && self.s3_endpoint.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::Config;

    #[test]
    fn test_s3_enabled_both_set() {
        let cfg = Config {
            bot_token: String::new(),
            web_app_url: String::new(),
            port: 3000,
            webhook_path: String::new(),
            app_url: String::new(),
            is_production: false,
            admin_ids: vec![],
            bot_username: String::new(),
            database_url: String::new(),
            grok_api_key: String::new(),
            glm_api_key: String::new(),
            s3_bucket: Some("bucket".into()),
            s3_endpoint: Some("http://s3".into()),
            s3_internal_endpoint: None,
            s3_public_url: None,
            s3_region: None,
            s3_access_key: None,
            s3_secret_key: None,
            backup_assets: false,
            admin_password: None,
        };
        assert!(cfg.s3_enabled());
    }

    #[test]
    fn test_s3_enabled_missing_endpoint() {
        let cfg = Config {
            s3_bucket: Some("bucket".into()),
            s3_endpoint: None,
            ..test_config()
        };
        assert!(!cfg.s3_enabled());
    }

    #[test]
    fn test_s3_enabled_missing_bucket() {
        let cfg = Config {
            s3_bucket: None,
            s3_endpoint: Some("http://s3".into()),
            ..test_config()
        };
        assert!(!cfg.s3_enabled());
    }

    #[test]
    fn test_s3_enabled_both_missing() {
        let cfg = Config {
            s3_bucket: None,
            s3_endpoint: None,
            ..test_config()
        };
        assert!(!cfg.s3_enabled());
    }

    fn test_config() -> Config {
        Config {
            bot_token: String::new(),
            web_app_url: String::new(),
            port: 3000,
            webhook_path: String::new(),
            app_url: String::new(),
            is_production: false,
            admin_ids: vec![],
            bot_username: String::new(),
            database_url: String::new(),
            grok_api_key: String::new(),
            glm_api_key: String::new(),
            s3_bucket: None,
            s3_endpoint: None,
            s3_internal_endpoint: None,
            s3_public_url: None,
            s3_region: None,
            s3_access_key: None,
            s3_secret_key: None,
            backup_assets: false,
            admin_password: None,
        }
    }
}
