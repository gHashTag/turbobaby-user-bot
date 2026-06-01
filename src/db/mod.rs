pub mod entities;
pub mod loyalty;
pub mod orders;
pub mod referrals;
pub mod strains;
pub mod users;

pub use strains::*;

use anyhow::{Context, Result};
use deadpool_postgres::{
    tokio_postgres, Config as PgConfig, Connect, Manager, ManagerConfig, Pool, Runtime,
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
    include_str!("../../migrations/016_bilingual_fields.sql"),
    include_str!("../../migrations/017_restore_old_prices_and_discounts.sql"),
    include_str!("../../migrations/018_seed_tea_products.sql"),
    include_str!("../../migrations/019_add_default_images.sql"),
    include_str!("../../migrations/020_force_double_precision.sql"),
    include_str!("../../migrations/021_hunt_checkpoints_fields.sql"),
    include_str!("../../migrations/022_strains_video_url.sql"),
    include_str!("../../migrations/023_managers_commission_rate.sql"),
    include_str!("../../migrations/024_catalog_video_url.sql"),
    include_str!("../../migrations/025_orders_telegram_nullable.sql"),
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
    // SeaORM connection живёт параллельно с deadpool. Используется в новых/переписанных эндпоинтах.
    pub orm: sea_orm::DatabaseConnection,
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

        // Clean recycling runs DISCARD ALL on each returned connection,
        // dropping all cached prepared statements. This is required so that
        // ALTER TYPE migrations don't leave stale plans (Postgres error
        // SQLSTATE 0A000 "cached plan must not change result type").
        let manager_cfg = ManagerConfig {
            recycling_method: deadpool_postgres::RecyclingMethod::Clean,
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

        cfg.pool = Some(deadpool_postgres::PoolConfig {
            max_size: 16,
            timeouts: deadpool_postgres::Timeouts::wait_millis(30000),
            ..Default::default()
        });
        let pool_config = cfg.get_pool_config();
        let pool = Pool::builder(manager)
            .config(pool_config)
            .runtime(Runtime::Tokio1)
            .build()
            .map_err(|e| anyhow::anyhow!("Failed to build pool: {}", e))?;

        let _ = pool.get().await.context("Failed to connect to database")?;

        // Поднимаем SeaORM-подключение по той же DATABASE_URL.
        // SeaORM использует sqlx внутри, ssl-режим в URL (sslmode=require) обрабатывается автоматически.
        //
        // Чистим неподдерживаемые sqlx-postgres параметры (например channel_binding=require
        // у Neon/Supabase PG16+) чтобы не плодить WARN в логах. Сам TLS работает через sslmode.
        let sanitized_url = sanitize_pg_url_for_sqlx(database_url);
        let mut orm_opts = sea_orm::ConnectOptions::new(sanitized_url);
        orm_opts
            .max_connections(10)
            .min_connections(1)
            .connect_timeout(std::time::Duration::from_secs(10))
            .idle_timeout(std::time::Duration::from_secs(300))
            .sqlx_logging(false);
        let orm = sea_orm::Database::connect(orm_opts)
            .await
            .context("Failed to connect SeaORM")?;

        Ok(Self { pool, orm })
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

    /// Cycle #79: migrated from raw `tokio_postgres` to SeaORM entity
    /// (see docs/SEAORM_MIGRATION.md). Returns `None` for both "no row"
    /// and "query error" — matches the prior `.ok()??` semantics.
    pub async fn get_user_lang(&self, telegram_id: i64) -> Option<String> {
        use sea_orm::EntityTrait;
        match entities::user::Entity::find_by_id(telegram_id)
            .one(&self.orm)
            .await
        {
            Ok(Some(m)) => Some(m.language),
            Ok(None) => None,
            Err(e) => {
                tracing::warn!(
                    "db.get_user_lang: SeaORM query failed for telegram_id={}: {}",
                    telegram_id,
                    e
                );
                None
            }
        }
    }

    /// Cycle #79: migrated to SeaORM upsert. `ON CONFLICT DO UPDATE`
    /// translates to `OnConflict::column(...).update_columns(...)`. The
    /// `updated_at = NOW()` happens via the table's DEFAULT NOW() on
    /// insert path; for the update path we set it explicitly through
    /// the entity since SeaORM doesn't carry DB-side `updated_at` triggers.
    pub async fn set_user_lang(&self, telegram_id: i64, lang: &str) -> Result<()> {
        use sea_orm::sea_query::OnConflict;
        use sea_orm::{ActiveValue::Set, EntityTrait};
        let trimmed = crate::util::truncate_string(lang, 50);
        let am = entities::user::ActiveModel {
            telegram_id: Set(telegram_id),
            language: Set(trimmed.clone()),
            updated_at: Set(Some(chrono::Utc::now().into())),
            ..Default::default()
        };
        entities::user::Entity::insert(am)
            .on_conflict(
                OnConflict::column(entities::user::Column::TelegramId)
                    .update_columns([
                        entities::user::Column::Language,
                        entities::user::Column::UpdatedAt,
                    ])
                    .to_owned(),
            )
            .exec(&self.orm)
            .await
            .context("set_user_lang upsert")?;
        Ok(())
    }

    /// Cycle #79: SeaORM update via `Entity::update_many` + filter. The
    /// raw-SQL version was a bare UPDATE that no-ops when the row doesn't
    /// exist; SeaORM's `update` would error in that case, so we use
    /// `update_many` which silently affects zero rows for the no-row path
    /// — same semantics as before.
    pub async fn set_user_timezone(&self, telegram_id: i64, tz: &str) -> Result<()> {
        use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
        let trimmed = crate::util::truncate_string(tz, 100);
        entities::user::Entity::update_many()
            .col_expr(
                entities::user::Column::Timezone,
                sea_orm::sea_query::Expr::value(trimmed),
            )
            .filter(entities::user::Column::TelegramId.eq(telegram_id))
            .exec(&self.orm)
            .await
            .context("set_user_timezone")?;
        Ok(())
    }

    /// Cycle #79: SeaORM upsert (see [`Self::set_user_lang`] for pattern).
    /// Note: the original raw SQL didn't bump `updated_at` for this path,
    /// preserved here for behaviour parity — the upsert only writes
    /// `first_name`.
    pub async fn save_user_name(&self, telegram_id: i64, first_name: &str) -> Result<()> {
        use sea_orm::sea_query::OnConflict;
        use sea_orm::{ActiveValue::Set, EntityTrait};
        let trimmed = crate::util::truncate_string(first_name, 200);
        let am = entities::user::ActiveModel {
            telegram_id: Set(telegram_id),
            first_name: Set(Some(trimmed.clone())),
            // `language` column is NOT NULL with default 'en' — Set::default
            // skips it on insert, letting the DB default fire.
            ..Default::default()
        };
        entities::user::Entity::insert(am)
            .on_conflict(
                OnConflict::column(entities::user::Column::TelegramId)
                    .update_columns([entities::user::Column::FirstName])
                    .to_owned(),
            )
            .exec(&self.orm)
            .await
            .context("save_user_name upsert")?;
        Ok(())
    }

    /// Cycle #79: SeaORM update on `loyalty_profiles`. Same no-row tolerance
    /// as the raw SQL via `update_many`.
    pub async fn mark_user_unblocked(&self, telegram_id: i64) -> Result<()> {
        use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
        entities::loyalty_profile::Entity::update_many()
            .col_expr(
                entities::loyalty_profile::Column::IsBlocked,
                sea_orm::sea_query::Expr::value(false),
            )
            .filter(entities::loyalty_profile::Column::TelegramId.eq(telegram_id))
            .exec(&self.orm)
            .await
            .context("mark_user_unblocked")?;
        Ok(())
    }

    /// Cycle #79: migrated to SeaORM. Returns `false` when there's no
    /// loyalty_profiles row (first-time visitor) — matches prior
    /// `unwrap_or(false)` behaviour. Real query failures still bubble up
    /// via `Result` so callers can distinguish "not blocked" from "DB
    /// broken".
    pub async fn is_user_blocked(&self, telegram_id: i64) -> Result<bool> {
        use sea_orm::EntityTrait;
        let row = entities::loyalty_profile::Entity::find_by_id(telegram_id)
            .one(&self.orm)
            .await
            .context("is_user_blocked: SeaORM query")?;
        Ok(row.map(|m| m.is_blocked).unwrap_or(false))
    }

    /// Cycle #80: SeaORM migration. Same shape as before — top 5 strains
    /// flagged as `is_strain_of_day`, freshest first by
    /// `strain_of_day_set_at`.
    pub async fn get_strains_of_day(&self) -> Result<Vec<StrainOfDay>> {
        use sea_orm::{ColumnTrait, EntityTrait, Order, QueryFilter, QueryOrder, QuerySelect};
        let models = entities::strain::Entity::find()
            .filter(entities::strain::Column::IsStrainOfDay.eq(true))
            .filter(entities::strain::Column::IsAvailable.eq(true))
            .order_by(entities::strain::Column::StrainOfDaySetAt, Order::Desc)
            .limit(5)
            .all(&self.orm)
            .await
            .context("get_strains_of_day SeaORM")?;
        Ok(models.into_iter().map(StrainOfDay::from).collect())
    }

    pub fn raw(&self) -> &Pool {
        &self.pool
    }
}

/// Убирает из connection-URL параметры, которые sqlx-postgres не понимает
/// и пишет про них WARN (например channel_binding=require у Neon/Supabase).
///
/// tokio-postgres тоже их игнорирует, но мы чистим URL только для SeaORM/sqlx,
/// чтобы не влиять на deadpool_postgres-путь.
fn sanitize_pg_url_for_sqlx(url: &str) -> String {
    const UNSUPPORTED: &[&str] = &["channel_binding"];
    let (base, query) = match url.split_once('?') {
        Some((b, q)) => (b, q),
        None => return url.to_string(),
    };
    let kept: Vec<&str> = query
        .split('&')
        .filter(|pair| {
            let key = pair.split('=').next().unwrap_or("");
            !UNSUPPORTED.iter().any(|u| key.eq_ignore_ascii_case(u))
        })
        .collect();
    if kept.is_empty() {
        base.to_string()
    } else {
        format!("{}?{}", base, kept.join("&"))
    }
}

#[cfg(test)]
mod url_sanitize_tests {
    use super::sanitize_pg_url_for_sqlx as s;
    #[test]
    fn drops_channel_binding() {
        assert_eq!(
            s("postgres://u:p@h/d?sslmode=require&channel_binding=require"),
            "postgres://u:p@h/d?sslmode=require"
        );
    }
    #[test]
    fn keeps_others() {
        assert_eq!(
            s("postgres://u:p@h/d?sslmode=require"),
            "postgres://u:p@h/d?sslmode=require"
        );
    }
    #[test]
    fn no_query() {
        assert_eq!(s("postgres://u:p@h/d"), "postgres://u:p@h/d");
    }
    #[test]
    fn only_unsupported() {
        assert_eq!(
            s("postgres://u:p@h/d?channel_binding=require"),
            "postgres://u:p@h/d"
        );
    }
}
