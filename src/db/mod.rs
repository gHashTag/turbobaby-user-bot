pub mod strains;
pub mod loyalty;
pub mod orders;
pub mod users;
pub mod referrals;

pub use strains::*;

use anyhow::{Context, Result};
use deadpool_postgres::{
    Config as PgConfig, Connect, Manager, ManagerConfig, Pool, Runtime, tokio_postgres,
};
use rustls::ClientConfig;
use rustls_native_certs::load_native_certs;
use tokio_postgres_rustls::MakeRustlsConnect;
use url::Url;

use std::future::Future;
use std::pin::Pin;
use tokio::task::JoinHandle;

const MIGRATION_SQL: &str = concat!(
    include_str!("../../migrations/001_initial.sql"),
    include_str!("../../migrations/002_catalog.sql"),
    include_str!("../../migrations/003_quest.sql"),
    include_str!("../../migrations/004_garden.sql"),
    include_str!("../../migrations/005_alter_strains_sotd.sql"),
    include_str!("../../migrations/006_seed_strain_images.sql"),
    include_str!("../../migrations/007_referral_events.sql"),
    include_str!("../../migrations/008_strains_full_seed.sql"),
    include_str!("../../migrations/009_loyalty_tiers_seed.sql"),
    include_str!("../../migrations/010_tech_tree_seed.sql"),
    include_str!("../../migrations/011_strains_force_prices.sql"),
    include_str!("../../migrations/012_strains_fix_numeric_to_float8.sql"),
    include_str!("../../migrations/013_catalog_full_seed.sql"),
    include_str!("../../migrations/014_force_prices.sql"),
    include_str!("../../migrations/015_catalog_fix_numeric_to_float8.sql"),
);

type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

struct NoTlsConnect;

impl Connect for NoTlsConnect {
    fn connect(
        &self,
        pg_config: &tokio_postgres::Config,
    ) -> BoxFuture<'_, Result<(tokio_postgres::Client, JoinHandle<()>), tokio_postgres::Error>>
    {
        let pg_config = pg_config.clone();
        Box::pin(async move {
            let (client, connection) = pg_config.connect(tokio_postgres::NoTls).await?;
            let conn_task = tokio::spawn(async move {
                if let Err(e) = connection.await {
                    tracing::warn!("Connection error: {}", e);
                }
            });
            Ok((client, conn_task))
        })
    }
}

struct RustlsConnect {
    tls: MakeRustlsConnect,
}

impl Connect for RustlsConnect {
    fn connect(
        &self,
        pg_config: &tokio_postgres::Config,
    ) -> BoxFuture<'_, Result<(tokio_postgres::Client, JoinHandle<()>), tokio_postgres::Error>>
    {
        let tls = self.tls.clone();
        let pg_config = pg_config.clone();
        Box::pin(async move {
            let (client, connection) = pg_config.connect(tls).await?;
            let conn_task = tokio::spawn(async move {
                if let Err(e) = connection.await {
                    tracing::warn!("Connection error: {}", e);
                }
            });
            Ok((client, conn_task))
        })
    }
}

pub struct Database {
    pub pool: Pool,
}

impl Database {
    pub async fn connect(database_url: &str) -> Result<Self> {
        let url = Url::parse(database_url).context("Invalid DATABASE_URL")?;

        let ssl_mode = url
            .query_pairs()
            .find(|(k, _)| k == "sslmode")
            .map(|(_, v)| v.to_string())
            .unwrap_or_else(|| "disable".to_string());

        let mut cfg = PgConfig::new();
        cfg.host = url.host_str().map(|s| s.to_string());
        cfg.port = url.port();
        cfg.dbname = Some(url.path().trim_start_matches('/').to_string());
        cfg.user = Some(url.username().to_string());
        cfg.password = url.password().map(|s| s.to_string());
        cfg.ssl_mode = Some(if ssl_mode == "disable" {
            deadpool_postgres::SslMode::Disable
        } else {
            deadpool_postgres::SslMode::Require
        });
        cfg.keepalives = Some(true);
        cfg.keepalives_idle = Some(std::time::Duration::from_secs(300));

        let manager_cfg = ManagerConfig {
            recycling_method: deadpool_postgres::RecyclingMethod::Verified,
        };

        let pg_config = cfg
            .get_pg_config()
            .map_err(|e| anyhow::anyhow!("Invalid pg config: {}", e))?;

        let manager = if ssl_mode == "disable" {
            Manager::from_connect(pg_config, NoTlsConnect, manager_cfg)
        } else {
            let mut roots = rustls::RootCertStore::empty();
            for cert in load_native_certs().certs {
                roots.add(cert)?;
            }
            let tls_config = ClientConfig::builder()
                .with_root_certificates(roots)
                .with_no_client_auth();
            let tls = MakeRustlsConnect::new(tls_config);
            Manager::from_connect(pg_config, RustlsConnect { tls }, manager_cfg)
        };

        let pool_config = cfg.get_pool_config();
        let pool = Pool::builder(manager)
            .config(pool_config)
            .runtime(Runtime::Tokio1)
            .build()
            .map_err(|e| anyhow::anyhow!("Failed to build pool: {}", e))?;

        let _ = pool.get().await.context("Failed to connect to database")?;
        Ok(Self { pool })
    }

    pub async fn run_migrations(&self) -> Result<()> {
        // Use a one-shot raw tokio_postgres connection (no deadpool, no
        // statement cache) so that ALTER TABLE ... TYPE in migration 012
        // doesn't poison the long-lived pool with stale prepared plans
        // (Postgres SQLSTATE 0A000 "cached plan must not change result type").
        let client = self.pool.get().await?;
        client.batch_execute(MIGRATION_SQL).await?;
        drop(client);
        // After migrations, evict all currently-idle connections from the
        // pool. New requests will get fresh connections that re-prepare
        // statements against the post-migration schema.
        self.pool.retain(|_, _| false);
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
        // Bust client-side prepared-statement cache (post-migration ALTER TYPE)
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let sql = format!(
            "SELECT s.id, s.name, s.category, s.thc_percent, s.price_per_gram, s.strain_of_day_discount, s.image_url FROM strains s WHERE s.is_strain_of_day = true AND s.is_available = true ORDER BY s.strain_of_day_set_at DESC LIMIT 5 -- nonce={}",
            nonce
        );
        let rows = client.query(&sql, &[]).await?;
        Ok(rows.iter().map(StrainOfDay::from_row).collect())
    }

    pub fn raw(&self) -> &Pool { &self.pool }
}
