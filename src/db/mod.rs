pub mod strains;
pub mod loyalty;
pub mod orders;
pub mod users;
pub mod referrals;

pub use strains::*;

use anyhow::{Context, Result};
use deadpool_postgres::{Config as PgConfig, Pool, Runtime, ManagerConfig};
use rustls::ClientConfig;
use rustls_native_certs::load_native_certs;
use tokio_postgres_rustls::MakeRustlsConnect;
use url::Url;

const MIGRATION_SQL: &str = concat!(
    include_str!("../../migrations/001_initial.sql"),
    include_str!("../../migrations/002_catalog.sql"),
    include_str!("../../migrations/003_quest.sql"),
    include_str!("../../migrations/004_garden.sql"),
    include_str!("../../migrations/005_alter_strains_sotd.sql"),
    include_str!("../../migrations/006_seed_strain_images.sql"),
    include_str!("../../migrations/007_referral_events.sql"),
    include_str!("../../migrations/008_strains_full_seed.sql"),
);

pub struct Database {
    pub pool: Pool,
}

impl Database {
    pub async fn connect(database_url: &str) -> Result<Self> {
        let url = Url::parse(database_url).context("Invalid DATABASE_URL")?;

        let mut cfg = PgConfig::new();
        cfg.host = url.host_str().map(|s| s.to_string());
        cfg.port = url.port();
        cfg.dbname = Some(url.path().trim_start_matches('/').to_string());
        cfg.user = Some(url.username().to_string());
        cfg.password = url.password().map(|s| s.to_string());
        cfg.ssl_mode = Some(deadpool_postgres::SslMode::Require);
        cfg.keepalives = Some(true);
        cfg.keepalives_idle = Some(std::time::Duration::from_secs(300));

        let mut roots = rustls::RootCertStore::empty();
        for cert in load_native_certs().certs {
            roots.add(cert)?;
        }
        let tls_config = ClientConfig::builder()
            .with_root_certificates(roots)
            .with_no_client_auth();
        let tls = MakeRustlsConnect::new(tls_config);

        cfg.manager = Some(ManagerConfig {
            recycling_method: deadpool_postgres::RecyclingMethod::Verified,
        });

        let pool = cfg.create_pool(Some(Runtime::Tokio1), tls)?;
        let _ = pool.get().await.context("Failed to connect to database")?;
        Ok(Self { pool })
    }

    pub async fn run_migrations(&self) -> Result<()> {
        let client = self.pool.get().await?;
        client.batch_execute(MIGRATION_SQL).await?;
        Ok(())
    }

    pub async fn get_user_lang(&self, telegram_id: i64) -> Option<String> {
        let client = self.pool.get().await.ok()?;
        let row = client
            .query_opt(
                "SELECT language FROM user_languages WHERE telegram_id = $1",
                &[&telegram_id],
            )
            .await
            .ok()??;
        row.try_get("language").ok()
    }

    pub async fn set_user_lang(&self, telegram_id: i64, lang: &str) -> Result<()> {
        let client = self.pool.get().await?;
        client.execute(
            "INSERT INTO user_languages (telegram_id, language) VALUES ($1, $2)
             ON CONFLICT (telegram_id) DO UPDATE SET language = $2, updated_at = NOW()",
            &[&telegram_id, &lang],
        ).await?;
        Ok(())
    }

    pub async fn set_user_timezone(&self, telegram_id: i64, tz: &str) -> Result<()> {
        let client = self.pool.get().await?;
        client.execute(
            "UPDATE user_languages SET timezone = $2 WHERE telegram_id = $1",
            &[&telegram_id, &tz],
        ).await?;
        Ok(())
    }

    pub async fn save_user_name(&self, telegram_id: i64, first_name: &str) -> Result<()> {
        let client = self.pool.get().await?;
        client.execute(
            "INSERT INTO user_languages (telegram_id, first_name) VALUES ($1, $2)
             ON CONFLICT (telegram_id) DO UPDATE SET first_name = $2",
            &[&telegram_id, &first_name],
        ).await?;
        Ok(())
    }

    pub async fn mark_user_unblocked(&self, telegram_id: i64) -> Result<()> {
        let client = self.pool.get().await?;
        client.execute(
            "UPDATE loyalty_profiles SET is_blocked = false WHERE telegram_id = $1",
            &[&telegram_id],
        ).await?;
        Ok(())
    }

    pub async fn get_strains_of_day(&self) -> Result<Vec<StrainOfDay>> {
        let client = self.pool.get().await?;
        let rows = client.query(
            "SELECT s.id, s.name, s.category, s.thc_percent, s.price_per_gram,
                    s.strain_of_day_discount, s.image_url
             FROM strains s WHERE s.is_strain_of_day = true AND s.is_available = true
             ORDER BY s.strain_of_day_set_at DESC LIMIT 5",
            &[],
        ).await?;
        Ok(rows.iter().map(StrainOfDay::from_row).collect())
    }

    pub fn raw(&self) -> &Pool { &self.pool }
}
