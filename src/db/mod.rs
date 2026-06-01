pub mod entities;
pub mod loyalty;
pub mod macros;
pub mod orders;
pub mod referrals;
pub mod strains;
pub mod users;

pub use strains::*;

use anyhow::{Context, Result};

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
    include_str!("../../migrations/026_garden_backfill_from_orders.sql"),
    include_str!("../../migrations/027_game_high_scores.sql"),
    include_str!("../../migrations/028_strain_marketing_flags.sql"),
    include_str!("../../migrations/029_order_idempotency_keys.sql"),
    include_str!("../../migrations/030_order_fraud_events.sql"),
    include_str!("../../migrations/031_block_history.sql"),
);

/// Cycle #96: after the 17-cycle SeaORM migration finished, this is the
/// single connection handle to Postgres. Previously held a parallel
/// `deadpool_postgres::Pool` next to the SeaORM `DatabaseConnection`;
/// every callsite is now SeaORM, so the pool field, its rustls/tls
/// adapters, and the `deadpool_postgres` / `tokio_postgres` /
/// `tokio_postgres_rustls` / `postgres_types` deps are gone.
///
/// SeaORM wraps sqlx internally; sqlx manages the underlying pool with
/// its own TLS via `runtime-tokio-rustls`, so we don't see any of that
/// machinery here.
pub struct Database {
    pub orm: sea_orm::DatabaseConnection,
}

impl Database {
    pub async fn connect(database_url: &str) -> Result<Self> {
        // SeaORM connects via sqlx; sslmode=require in the URL is handled
        // automatically. The `channel_binding=require` knob that Neon /
        // Supabase PG16+ sometimes adds isn't a sqlx-postgres param, so
        // we strip it to avoid noisy WARNs (TLS itself still works via
        // sslmode).
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

        Ok(Self { orm })
    }

    pub async fn run_migrations(&self) -> Result<()> {
        // Cycle #96: was `pool.get().batch_execute(MIGRATION_SQL)` with a
        // post-run `pool.retain` to evict cached prepared statements
        // (defence against the tokio_postgres SQLSTATE 0A000 "cached plan
        // must not change result type" issue triggered by ALTER TYPE in
        // migration 012). sqlx (which SeaORM uses) handles prepared
        // statements differently and isn't subject to that bug, so we
        // just run the SQL and move on.
        use sea_orm::ConnectionTrait;
        self.orm
            .execute_unprepared(MIGRATION_SQL)
            .await
            .context("run_migrations: execute_unprepared")?;
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

/// Cycle #101: catches "migration file added on disk but forgotten in the
/// `MIGRATION_SQL` concat" — the exact bug found that prompted this test
/// (migrations 026-031 existed for ~6 release cycles but were never run
/// at startup; entities/code that depended on them only worked because
/// someone ran the SQL manually against prod).
///
/// The test walks `migrations/` at compile time via `include_str!` of
/// its own source: every `*.sql` in the directory must appear in the
/// module source above. Pure-string check, no DB needed.
#[cfg(test)]
mod migration_manifest_tests {
    /// We embed the source of this module at compile time and grep it
    /// for `include_str!("../../migrations/<file>.sql")` references.
    /// Equivalent to running the check at build time but lives in the
    /// regular test suite for visibility.
    const THIS_FILE: &str = include_str!("mod.rs");

    #[test]
    fn every_migration_file_is_included() {
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let mig_dir = std::path::Path::new(manifest_dir).join("migrations");
        let mut on_disk: Vec<String> = std::fs::read_dir(&mig_dir)
            .expect("migrations/ readable")
            .filter_map(|e| e.ok())
            .filter_map(|e| e.file_name().into_string().ok())
            .filter(|n| n.ends_with(".sql"))
            .collect();
        on_disk.sort();

        let missing: Vec<&str> = on_disk
            .iter()
            .filter(|name| {
                let needle = format!("../../migrations/{}", name);
                !THIS_FILE.contains(&needle)
            })
            .map(String::as_str)
            .collect();

        assert!(
            missing.is_empty(),
            "migration files on disk but missing from MIGRATION_SQL in src/db/mod.rs: {:?}",
            missing
        );
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
