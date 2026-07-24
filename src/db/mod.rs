// Integration tests in `tests/*.rs` import `entities` and `orders`
// as external lib consumers — they must stay `pub mod`. The bin
// crate still flags them as unreachable_pub (it has private
// `mod db;`), hence the allow.
#[allow(unreachable_pub)]
pub mod entities;
pub(crate) mod loyalty;
pub(crate) mod macros;
#[allow(unreachable_pub)]
pub mod orders;
pub(crate) mod referrals;
pub(crate) mod strains;
pub(crate) mod users;

#[allow(unreachable_pub)]
// Glob re-export of crate-internal types (used as `crate::db::Strain` etc.); flavour-tightening would force renames everywhere.
pub use strains::*;

use anyhow::{Context, Result};

/// Every migration as `(name, sql)`, in apply order. Replaces the old single
/// `concat!` blob so each migration can be tracked + run EXACTLY ONCE (see
/// `run_migrations` + the `_schema_migrations` table). Re-running the whole blob
/// every boot was resurrecting admin-deleted catalog rows and overwriting admin
/// edits (seed migrations 013/018 `ON CONFLICT DO UPDATE`, the price/image
/// `UPDATE` migrations) — the bug this fix closes.
const MIGRATIONS: &[(&str, &str)] = &[
    (
        "001_initial.sql",
        include_str!("../../migrations/001_initial.sql"),
    ),
    (
        "002_catalog.sql",
        include_str!("../../migrations/002_catalog.sql"),
    ),
    (
        "003_quest.sql",
        include_str!("../../migrations/003_quest.sql"),
    ),
    (
        "004_garden.sql",
        include_str!("../../migrations/004_garden.sql"),
    ),
    (
        "005_alter_strains_sotd.sql",
        include_str!("../../migrations/005_alter_strains_sotd.sql"),
    ),
    (
        "006_seed_strain_images.sql",
        include_str!("../../migrations/006_seed_strain_images.sql"),
    ),
    (
        "007_referral_events.sql",
        include_str!("../../migrations/007_referral_events.sql"),
    ),
    (
        "008_strains_full_seed.sql",
        include_str!("../../migrations/008_strains_full_seed.sql"),
    ),
    (
        "009_loyalty_tiers_seed.sql",
        include_str!("../../migrations/009_loyalty_tiers_seed.sql"),
    ),
    (
        "010_tech_tree_seed.sql",
        include_str!("../../migrations/010_tech_tree_seed.sql"),
    ),
    (
        "011_strains_force_prices.sql",
        include_str!("../../migrations/011_strains_force_prices.sql"),
    ),
    (
        "012_strains_fix_numeric_to_float8.sql",
        include_str!("../../migrations/012_strains_fix_numeric_to_float8.sql"),
    ),
    (
        "013_catalog_full_seed.sql",
        include_str!("../../migrations/013_catalog_full_seed.sql"),
    ),
    (
        "014_force_prices.sql",
        include_str!("../../migrations/014_force_prices.sql"),
    ),
    (
        "015_catalog_fix_numeric_to_float8.sql",
        include_str!("../../migrations/015_catalog_fix_numeric_to_float8.sql"),
    ),
    (
        "016_bilingual_fields.sql",
        include_str!("../../migrations/016_bilingual_fields.sql"),
    ),
    (
        "017_restore_old_prices_and_discounts.sql",
        include_str!("../../migrations/017_restore_old_prices_and_discounts.sql"),
    ),
    (
        "018_seed_tea_products.sql",
        include_str!("../../migrations/018_seed_tea_products.sql"),
    ),
    (
        "019_add_default_images.sql",
        include_str!("../../migrations/019_add_default_images.sql"),
    ),
    (
        "020_force_double_precision.sql",
        include_str!("../../migrations/020_force_double_precision.sql"),
    ),
    (
        "021_hunt_checkpoints_fields.sql",
        include_str!("../../migrations/021_hunt_checkpoints_fields.sql"),
    ),
    (
        "022_strains_video_url.sql",
        include_str!("../../migrations/022_strains_video_url.sql"),
    ),
    (
        "023_managers_commission_rate.sql",
        include_str!("../../migrations/023_managers_commission_rate.sql"),
    ),
    (
        "024_catalog_video_url.sql",
        include_str!("../../migrations/024_catalog_video_url.sql"),
    ),
    (
        "025_orders_telegram_nullable.sql",
        include_str!("../../migrations/025_orders_telegram_nullable.sql"),
    ),
    (
        "026_garden_backfill_from_orders.sql",
        include_str!("../../migrations/026_garden_backfill_from_orders.sql"),
    ),
    (
        "027_game_high_scores.sql",
        include_str!("../../migrations/027_game_high_scores.sql"),
    ),
    (
        "028_strain_marketing_flags.sql",
        include_str!("../../migrations/028_strain_marketing_flags.sql"),
    ),
    (
        "029_order_idempotency_keys.sql",
        include_str!("../../migrations/029_order_idempotency_keys.sql"),
    ),
    (
        "030_order_fraud_events.sql",
        include_str!("../../migrations/030_order_fraud_events.sql"),
    ),
    (
        "031_block_history.sql",
        include_str!("../../migrations/031_block_history.sql"),
    ),
    (
        "032_loyalty_config_marketing_badges_hidden.sql",
        include_str!("../../migrations/032_loyalty_config_marketing_badges_hidden.sql"),
    ),
    (
        "033_loyalty_idempotency_keys.sql",
        include_str!("../../migrations/033_loyalty_idempotency_keys.sql"),
    ),
    (
        "034_sets_image_url.sql",
        include_str!("../../migrations/034_sets_image_url.sql"),
    ),
    (
        "035_referral_code_telegram_id.sql",
        include_str!("../../migrations/035_referral_code_telegram_id.sql"),
    ),
    (
        "036_garden_backfill_non_strain_orders.sql",
        include_str!("../../migrations/036_garden_backfill_non_strain_orders.sql"),
    ),
    (
        "037_garden_universal_backfill.sql",
        include_str!("../../migrations/037_garden_universal_backfill.sql"),
    ),
    (
        "038_sets_bilingual.sql",
        include_str!("../../migrations/038_sets_bilingual.sql"),
    ),
    (
        "039_sets_columns.sql",
        include_str!("../../migrations/039_sets_columns.sql"),
    ),
    (
        "040_sets_packs_fields.sql",
        include_str!("../../migrations/040_sets_packs_fields.sql"),
    ),
    (
        "041_garden_choose_product.sql",
        include_str!("../../migrations/041_garden_choose_product.sql"),
    ),
    (
        "042_stars_currency.sql",
        include_str!("../../migrations/042_stars_currency.sql"),
    ),
    (
        "043_events_booking.sql",
        include_str!("../../migrations/043_events_booking.sql"),
    ),
    (
        "044_order_delivery_fields.sql",
        include_str!("../../migrations/044_order_delivery_fields.sql"),
    ),
    (
        "045_events_waitlist_status.sql",
        include_str!("../../migrations/045_events_waitlist_status.sql"),
    ),
    (
        "046_opaque_referral_codes.sql",
        include_str!("../../migrations/046_opaque_referral_codes.sql"),
    ),
    (
        "047_age_verification.sql",
        include_str!("../../migrations/047_age_verification.sql"),
    ),
    (
        "048_client_error_logs.sql",
        include_str!("../../migrations/048_client_error_logs.sql"),
    ),
    (
        "049_strain_reviews.sql",
        include_str!("../../migrations/049_strain_reviews.sql"),
    ),
    (
        "050_lab_certificates.sql",
        include_str!("../../migrations/050_lab_certificates.sql"),
    ),
    (
        "051_events_stars_price.sql",
        include_str!("../../migrations/051_events_stars_price.sql"),
    ),
];

/// Columns the catalog endpoints SELECT that were added by *later* migrations
/// (016/019/024/034) — exactly the ones that go missing when a prod migration
/// run doesn't reach them, 500-ing `GET /api/sets`. Base columns from 002 are
/// omitted (they exist whenever the table does). Used by the startup schema
/// self-check ([`Database::missing_critical_columns`]).
pub(crate) const CRITICAL_COLUMNS: &[(&str, &[&str])] = &[
    (
        "accessory_sets",
        &["image_url", "video_url", "name_en", "description_en"],
    ),
    (
        "tea_sets",
        &["image_url", "video_url", "name_en", "description_en"],
    ),
    (
        "sets",
        &["image_url", "video_url", "total_weight_grams", "badge"],
    ),
];

/// Pure diff: which `expected` (table, column) pairs are absent from `present`
/// (the live `information_schema` snapshot). Returned as `"table.column"`.
fn missing_columns(
    expected: &[(&str, &[&str])],
    present: &std::collections::HashSet<(String, String)>,
) -> Vec<String> {
    let mut missing = Vec::new();
    for (table, cols) in expected {
        for col in *cols {
            if !present.contains(&((*table).to_string(), (*col).to_string())) {
                missing.push(format!("{table}.{col}"));
            }
        }
    }
    missing
}

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

/// Cycle #123: parse `DB_POOL_MAX` env var with strict semantics —
/// `None` (unset) returns the default 10; `Some(s)` requires a valid
/// `u32 >= 1` or errors out. Mirrors `config::parse_port_env` from
/// cycle #118 — same shape, same failure-loud principle so a typo
/// like `DB_POOL_MAX="ten"` doesn't silently fall back.
///
/// Capped at the SeaORM/sqlx-postgres natural upper bound (u32::MAX
/// for the type, but in practice Railway's free PG plan caps at 60
/// connections shared across all clients; values >50 are almost
/// certainly mistakes). We don't enforce that cap here because what
/// "too high" means depends on the PG plan — surface the warning in
/// docs instead.
pub(crate) fn parse_db_pool_max_env(raw: Option<String>) -> Result<u32> {
    const DEFAULT: u32 = 10;
    match raw {
        None => Ok(DEFAULT),
        Some(s) => {
            let trimmed = s.trim();
            if trimmed.is_empty() {
                return Ok(DEFAULT);
            }
            let n: u32 = trimmed.parse().map_err(|e| {
                anyhow::anyhow!(
                    "DB_POOL_MAX must be a non-negative integer, got {:?}: {}",
                    s,
                    e
                )
            })?;
            if n == 0 {
                anyhow::bail!("DB_POOL_MAX must be >= 1, got 0 (would deadlock the first query)");
            }
            Ok(n)
        }
    }
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

        // Cycle #123: max_connections is env-tunable. Default 10 fits
        // both small Railway free-tier deployments (PG limit 60, so a
        // single container leaves ample headroom for migrations +
        // additional replicas) and local dev. Set DB_POOL_MAX to bump
        // when running with more replicas or higher concurrency.
        let pool_max = parse_db_pool_max_env(std::env::var("DB_POOL_MAX").ok())?;
        orm_opts
            .max_connections(pool_max)
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
        // Each migration runs EXACTLY ONCE, tracked in `_schema_migrations`.
        // Previously the whole concat'd SQL re-ran every boot, so seed migrations
        // (013/018 `ON CONFLICT DO UPDATE`) resurrected admin-deleted catalog rows
        // and the price/image `UPDATE` migrations overwrote admin edits — on every
        // deploy. The tracker stops that permanently.
        use sea_orm::{ConnectionTrait, DbBackend, Statement};

        self.orm
            .execute_unprepared(
                "CREATE TABLE IF NOT EXISTS _schema_migrations (\
                     name TEXT PRIMARY KEY, \
                     applied_at TIMESTAMPTZ NOT NULL DEFAULT now())",
            )
            .await
            .context("run_migrations: create _schema_migrations")?;

        // One-time transition for ALREADY-MIGRATED databases: if the tracker is
        // empty but the DB is established (the `strains` table — created by 001 —
        // already exists), every migration listed here has demonstrably run
        // before (repeatedly). Mark them all applied WITHOUT re-running, so the
        // seed/price migrations can't clobber admin data even once more. A truly
        // fresh DB has no `strains` table → this is skipped and everything runs.
        let tracked: i64 = self
            .orm
            .query_one(Statement::from_string(
                DbBackend::Postgres,
                "SELECT count(*)::bigint AS c FROM _schema_migrations".to_string(),
            ))
            .await
            .context("run_migrations: count tracker")?
            .and_then(|r| r.try_get::<i64>("", "c").ok())
            .unwrap_or(0);
        if tracked == 0 {
            let established: bool = self
                .orm
                .query_one(Statement::from_string(
                    DbBackend::Postgres,
                    "SELECT to_regclass('public.strains') IS NOT NULL AS est".to_string(),
                ))
                .await
                .context("run_migrations: established check")?
                .and_then(|r| r.try_get::<bool>("", "est").ok())
                .unwrap_or(false);
            if established {
                for (name, _) in MIGRATIONS {
                    self.orm
                        .execute(Statement::from_sql_and_values(
                            DbBackend::Postgres,
                            "INSERT INTO _schema_migrations (name) VALUES ($1) ON CONFLICT DO NOTHING",
                            [(*name).into()],
                        ))
                        .await
                        .context("run_migrations: backfill tracker")?;
                }
                tracing::warn!(
                    "run_migrations: established DB — backfilled {} migrations as applied \
                     (one-time; future migrations run normally, seeds no longer re-run)",
                    MIGRATIONS.len()
                );
            }
        }

        // Apply each not-yet-recorded migration once, in order.
        for (name, sql) in MIGRATIONS {
            let applied: bool = self
                .orm
                .query_one(Statement::from_sql_and_values(
                    DbBackend::Postgres,
                    "SELECT EXISTS(SELECT 1 FROM _schema_migrations WHERE name = $1) AS a",
                    [(*name).into()],
                ))
                .await
                .with_context(|| format!("run_migrations: check {name}"))?
                .and_then(|r| r.try_get::<bool>("", "a").ok())
                .unwrap_or(false);
            if applied {
                continue;
            }
            self.orm
                .execute_unprepared(sql)
                .await
                .with_context(|| format!("run_migrations: apply {name}"))?;
            self.orm
                .execute(Statement::from_sql_and_values(
                    DbBackend::Postgres,
                    "INSERT INTO _schema_migrations (name) VALUES ($1) ON CONFLICT DO NOTHING",
                    [(*name).into()],
                ))
                .await
                .with_context(|| format!("run_migrations: record {name}"))?;
            tracing::info!("run_migrations: applied {name}");
        }
        Ok(())
    }

    /// Best-effort schema self-check run once at startup (after migrations).
    ///
    /// `GET /api/sets` 500'd on prod twice (2026-06-05, 2026-06-16) because a
    /// migration that adds a column to a set-table didn't apply on prod — a
    /// state invisible to `schema_drift_tests` (which only proves code↔
    /// migrations agree, not that prod *ran* them). This queries the LIVE
    /// `information_schema` and returns any [`CRITICAL_COLUMNS`] entry the
    /// database is actually missing, so startup can log it loudly by name
    /// instead of waiting for the first 500. Never blocks startup: a query
    /// error yields an empty list.
    pub async fn missing_critical_columns(&self) -> Vec<String> {
        use sea_orm::{ConnectionTrait, DbBackend, Statement};
        let rows = match self
            .orm
            .query_all(Statement::from_string(
                DbBackend::Postgres,
                "SELECT table_name, column_name FROM information_schema.columns \
                 WHERE table_schema = 'public' \
                   AND table_name IN ('accessory_sets', 'tea_sets', 'sets')"
                    .to_string(),
            ))
            .await
        {
            Ok(r) => r,
            Err(e) => {
                tracing::error!("schema self-check: information_schema query failed: {e}");
                return Vec::new();
            }
        };
        let present: std::collections::HashSet<(String, String)> = rows
            .iter()
            .filter_map(|r| {
                Some((
                    r.try_get::<String>("", "table_name").ok()?,
                    r.try_get::<String>("", "column_name").ok()?,
                ))
            })
            .collect();
        missing_columns(CRITICAL_COLUMNS, &present)
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
            // Carousel: owner picks UP TO 3 featured strains; show the 3 most
            // recently set. Admin enforcement of the cap lives in set_strain_of_day.
            .limit(3)
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

/// Cycle #102: companion to `migration_manifest_tests` — catches the
/// *other* drift direction. The manifest test ensures every migration
/// file gets applied; this one ensures every entity column actually
/// has a migration that creates or adds it.
///
/// Implementation: substring match in concatenated migration corpus.
/// Catches the common drift cases (column renamed in SQL but entity
/// still expects the old name, or column dropped). Misses pathological
/// cases like "same column name lives in a different table than the
/// entity expects" — a real SQL parser would catch those, but the
/// cost (full parser or pg_query dependency) doesn't justify the rare
/// catch.
///
/// SeaORM field-name → column-name mapping is implicit snake_case.
/// We don't use `#[sea_orm(column_name = "...")]` overrides anywhere
/// (verified at audit time), so Rust field name = SQL column name 1:1.
/// Cycle (Wave loop): `MIGRATION_SQL` above is a hand-maintained
/// `concat!(include_str!(...))` list. Twice already a migration file landed
/// in `migrations/` but was forgotten in that list — so the table/column it
/// created never existed on a fresh DB, surfacing as a runtime 500 long
/// after merge (commit 114de7d "wire 036 and 037 into MIGRATION_SQL").
///
/// This is an *architectural fitness function* (Ford/Parsons/Kua): it encodes
/// the invariant "every migration file is wired into the embedded runner" so a
/// dropped `include_str!` fails at `cargo test` time, not in production. Same
/// category as `entity_schema_consistency_tests` and the UI wiring defense.
#[cfg(test)]
mod schema_self_check_tests {
    use super::{missing_columns, CRITICAL_COLUMNS};
    use std::collections::HashSet;

    fn present(pairs: &[(&str, &str)]) -> HashSet<(String, String)> {
        pairs
            .iter()
            .map(|(t, c)| (t.to_string(), c.to_string()))
            .collect()
    }

    #[test]
    fn reports_missing_column() {
        // accessory_sets has image_url but not video_url → only video_url missing.
        let p = present(&[("accessory_sets", "image_url")]);
        let m = missing_columns(&[("accessory_sets", &["image_url", "video_url"])], &p);
        assert_eq!(m, vec!["accessory_sets.video_url".to_string()]);
    }

    #[test]
    fn empty_when_all_present() {
        let p = present(&[("sets", "image_url"), ("sets", "video_url")]);
        let m = missing_columns(&[("sets", &["image_url", "video_url"])], &p);
        assert!(m.is_empty());
    }

    #[test]
    fn all_missing_when_table_absent() {
        // The exact /api/sets failure mode: a set-table never got its later
        // columns → every expected column reported.
        let m = missing_columns(
            &[("tea_sets", &["name_en", "description_en"])],
            &present(&[]),
        );
        assert_eq!(m, vec!["tea_sets.name_en", "tea_sets.description_en"]);
    }

    #[test]
    fn critical_columns_list_is_sane() {
        // Guard the const itself: non-empty, only the set-tables, no dupes.
        assert!(!CRITICAL_COLUMNS.is_empty());
        for (table, cols) in CRITICAL_COLUMNS {
            assert!(
                matches!(*table, "accessory_sets" | "tea_sets" | "sets"),
                "unexpected table in CRITICAL_COLUMNS: {table}"
            );
            assert!(!cols.is_empty(), "{table} has no columns listed");
        }
    }
}

#[cfg(test)]
mod migration_wiring_tests {
    use std::collections::BTreeSet;

    /// All `NNN_*.sql` filenames present on disk under `migrations/`.
    fn migration_files() -> Vec<String> {
        let manifest = env!("CARGO_MANIFEST_DIR");
        let mig_dir = std::path::Path::new(manifest).join("migrations");
        let mut names: Vec<String> = std::fs::read_dir(&mig_dir)
            .expect("migrations/ readable")
            .filter_map(|e| e.ok())
            .filter_map(|e| e.file_name().into_string().ok())
            .filter(|n| n.ends_with(".sql"))
            .collect();
        names.sort();
        names
    }

    /// Source of `src/db/mod.rs` — where the `MIGRATION_SQL` concat lives.
    fn mod_rs_source() -> String {
        let manifest = env!("CARGO_MANIFEST_DIR");
        let path = std::path::Path::new(manifest).join("src/db/mod.rs");
        std::fs::read_to_string(path).expect("read src/db/mod.rs")
    }

    /// Every migration file on disk must be referenced by an `include_str!`
    /// inside `MIGRATION_SQL`. A file with no matching `include_str!` is dead
    /// schema that will never run on a fresh database.
    #[test]
    fn every_migration_file_is_wired_into_migration_sql() {
        let src = mod_rs_source();
        let files = migration_files();
        assert!(
            !files.is_empty(),
            "no migration files found — parser broken?"
        );

        let mut missing = Vec::new();
        for name in &files {
            // Matches the canonical `include_str!("../../migrations/NNN_x.sql")`.
            let needle = format!("migrations/{name}\"");
            if !src.contains(&needle) {
                missing.push(name.clone());
            }
        }
        assert!(
            missing.is_empty(),
            "{} migration file(s) on disk are NOT wired into MIGRATION_SQL \
             (add `include_str!(\"../../migrations/<name>\")` in src/db/mod.rs):\n  {}",
            missing.len(),
            missing.join("\n  ")
        );
    }

    /// Migration numeric prefixes must form a gap-free, duplicate-free run
    /// starting at 001. A gap (036 present, 035 missing) or a duplicate
    /// (`036_a.sql` + `036_b.sql`) means two devs branched off the same number
    /// or a file was deleted — both produce non-deterministic apply order.
    #[test]
    fn migration_numbers_are_sequential_and_unique() {
        let files = migration_files();
        let mut nums = BTreeSet::new();
        let mut dupes = Vec::new();
        for name in &files {
            let prefix: String = name.chars().take_while(|c| c.is_ascii_digit()).collect();
            let n: u32 = prefix
                .parse()
                .unwrap_or_else(|_| panic!("migration `{name}` has no numeric prefix"));
            if !nums.insert(n) {
                dupes.push(n);
            }
        }
        assert!(dupes.is_empty(), "duplicate migration number(s): {dupes:?}");

        let max = *nums.iter().next_back().expect("at least one migration");
        let gaps: Vec<u32> = (1..=max).filter(|n| !nums.contains(n)).collect();
        assert!(
            gaps.is_empty(),
            "gap(s) in migration numbering (missing): {gaps:?} — files run in \
             filename order, a gap usually means a deleted or misnamed migration"
        );
    }
}

#[cfg(test)]
mod entity_schema_consistency_tests {
    /// Extracts the `pub <field>:` names between `pub struct Model {`
    /// and the closing `}` of the struct.
    fn entity_fields(source: &str) -> Vec<String> {
        let mut in_model = false;
        let mut out = Vec::new();
        for line in source.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("pub struct Model") {
                in_model = true;
                continue;
            }
            if in_model && trimmed == "}" {
                break;
            }
            if !in_model {
                continue;
            }
            if let Some(rest) = trimmed.strip_prefix("pub ") {
                if let Some(colon) = rest.find(':') {
                    let name = &rest[..colon];
                    if name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
                        && !name.is_empty()
                    {
                        out.push(name.to_string());
                    }
                }
            }
        }
        out
    }

    fn read_migrations_corpus() -> String {
        let manifest = env!("CARGO_MANIFEST_DIR");
        let mig_dir = std::path::Path::new(manifest).join("migrations");
        let mut entries: Vec<_> = std::fs::read_dir(&mig_dir)
            .expect("migrations/ readable")
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_str().is_some_and(|n| n.ends_with(".sql")))
            .collect();
        entries.sort_by_key(|e| e.file_name());
        let mut buf = String::new();
        for e in entries {
            buf.push_str(&std::fs::read_to_string(e.path()).expect("read migration"));
            buf.push('\n');
        }
        buf
    }

    /// Word-boundary check: returns `true` if `needle` appears in `haystack`
    /// as a full identifier (surrounded by non-identifier characters or
    /// string boundaries).
    fn contains_word(haystack: &str, needle: &str) -> bool {
        let mut start = 0;
        while let Some(pos) = haystack[start..].find(needle) {
            let abs = start + pos;
            let before_ok = abs == 0
                || !haystack.as_bytes()[abs - 1].is_ascii_alphanumeric()
                    && haystack.as_bytes()[abs - 1] != b'_';
            let end = abs + needle.len();
            let after_ok = end >= haystack.len()
                || !haystack.as_bytes()[end].is_ascii_alphanumeric()
                    && haystack.as_bytes()[end] != b'_';
            if before_ok && after_ok {
                return true;
            }
            start = abs + needle.len();
        }
        false
    }

    #[test]
    fn every_entity_field_appears_in_some_migration() {
        let manifest = env!("CARGO_MANIFEST_DIR");
        let entities_dir = std::path::Path::new(manifest).join("src/db/entities");
        let corpus = read_migrations_corpus();

        let mut errors = Vec::new();
        let mut checked = 0_usize;
        for entry in std::fs::read_dir(&entities_dir).expect("entities/ readable") {
            let entry = entry.expect("entry");
            let name = entry.file_name().into_string().unwrap();
            if !name.ends_with(".rs") || name == "mod.rs" {
                continue;
            }
            let src = std::fs::read_to_string(entry.path()).expect("read entity");
            let fields = entity_fields(&src);
            for f in &fields {
                if !contains_word(&corpus, f) {
                    errors.push(format!(
                        "{}: Model field `{}` not found in any migration",
                        name, f
                    ));
                }
            }
            checked += fields.len();
        }
        assert!(checked > 0, "no fields were checked — parser broken?");
        assert!(
            errors.is_empty(),
            "entity/schema drift ({} field(s) missing from migrations):\n  {}",
            errors.len(),
            errors.join("\n  ")
        );
    }
}

/// Cycle #103: orphan-table detection — the reverse of cycle #102's
/// check. #102 ensures every entity column has a backing migration;
/// this one ensures every table created in migrations is actually
/// *used* in the codebase (via SeaORM entity, raw `Statement`, or
/// any other reference).
///
/// Dead schema costs nothing at runtime, but accumulates: every
/// future migration touching the dead table is wasted work, every
/// `try_get_warn!` reverse-audit has to mentally skip it, and
/// re-onboarding devs ask "what's this for?" Cycle #103 found
/// `hunt_checkpoints` orphaned this way — table created in
/// migration 003, ALTER COLUMN added in 021, zero code references
/// (the `get_checkpoints()` UI helper returns hardcoded
/// `vec![QuestCheckpoint::new(...)]`, no DB read).
///
/// Allowlist mechanism: known-orphan tables go in `ALLOWED_ORPHANS`
/// with an inline rationale. The test fails on *new* orphans only.
#[cfg(test)]
mod orphan_table_tests {
    /// Tables created in migrations but intentionally unused in code.
    /// Each entry must carry a rationale comment so future contributors
    /// know whether to wire it up or drop the migration.
    const ALLOWED_ORPHANS: &[&str] = &[
        // Cycle #103: created in migrations 003/021 to back a DB-driven
        // location-quest checkpoint feature that never shipped. The UI
        // (src/ui/game/quest.rs) uses a hardcoded `get_checkpoints()`
        // helper in src/trios/quest.rs. Leaving the table in place is
        // cheap (idempotent CREATE TABLE IF NOT EXISTS); dropping it
        // would require a new migration + prod coordination. Re-evaluate
        // when location quests are revisited.
        "hunt_checkpoints",
        // Cycle #144: created in migration 027 to back Woody Catch
        // cross-device high-score persistence. The backend code that
        // would have read/written it (`src/api/game.rs`, 244 lines)
        // was deleted this cycle because it had been orphaned since
        // cycle #91 — `pub mod game` was never declared, so the file
        // wasn't even compiled, and the UI (src/ui/game/woody_catch.rs)
        // stores scores in browser localStorage instead. The
        // route-wiring defense added in cycle #143 surfaced the orphan;
        // this allowlist entry replaces the previous false-negative
        // (the textual `game_high_scores` reference inside the
        // unbuilt game.rs that fooled this very test). Dropping the
        // table requires a new migration + prod coordination; the
        // idempotent CREATE TABLE IF NOT EXISTS in migration 027
        // costs nothing on each startup. Re-evaluate if Woody Catch
        // gains server-side leaderboards.
        "game_high_scores",
    ];

    fn migration_tables() -> Vec<String> {
        let manifest = env!("CARGO_MANIFEST_DIR");
        let mig_dir = std::path::Path::new(manifest).join("migrations");
        let mut out = Vec::new();
        let mut entries: Vec<_> = std::fs::read_dir(&mig_dir)
            .expect("migrations/ readable")
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_str().is_some_and(|n| n.ends_with(".sql")))
            .collect();
        entries.sort_by_key(|e| e.file_name());
        for entry in entries {
            let content = std::fs::read_to_string(entry.path()).expect("read migration");
            for line in content.lines() {
                let t = line.trim();
                // Match "CREATE TABLE [IF NOT EXISTS] <name>" — case-insensitive
                // on the keywords, identifier ends at first non-alnum/underscore.
                let lower = t.to_ascii_lowercase();
                let rest = if let Some(r) = lower.strip_prefix("create table if not exists ") {
                    &t[t.len() - r.len()..]
                } else if let Some(r) = lower.strip_prefix("create table ") {
                    &t[t.len() - r.len()..]
                } else {
                    continue;
                };
                let name: String = rest
                    .chars()
                    .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                    .collect();
                if !name.is_empty() {
                    out.push(name);
                }
            }
        }
        out.sort();
        out.dedup();
        out
    }

    fn code_corpus() -> String {
        let manifest = env!("CARGO_MANIFEST_DIR");
        let src = std::path::Path::new(manifest).join("src");
        // The file this test lives in carries the allowlist constant and
        // the `migrations/<file>.sql` include_str! lines — including it
        // would create false-positive code references for the very names
        // we're auditing. Real callsites live in api/, bot/, ui/,
        // db/entities/, etc.
        let self_path = std::path::Path::new(manifest).join("src/db/mod.rs");
        let mut buf = String::new();
        fn walk(p: &std::path::Path, skip: &std::path::Path, buf: &mut String) {
            for entry in std::fs::read_dir(p)
                .expect("readable")
                .filter_map(|e| e.ok())
            {
                let path = entry.path();
                if path.is_dir() {
                    walk(&path, skip, buf);
                } else if path == skip {
                    continue;
                } else if path
                    .extension()
                    .and_then(|s| s.to_str())
                    .is_some_and(|s| s == "rs")
                {
                    if let Ok(s) = std::fs::read_to_string(&path) {
                        // Strip line comments so a table name appearing
                        // in documentation prose elsewhere in the tree
                        // doesn't fool `contains_word` into thinking
                        // the table is wired (cycle #145, after #144
                        // hit this with `game_high_scores` mentioned in
                        // a cycle-#143 doc-comment in src/api/mod.rs).
                        buf.push_str(&strip_line_comments(&s));
                        buf.push('\n');
                    }
                }
            }
        }
        walk(&src, &self_path, &mut buf);
        buf
    }

    /// Strip `//` line comments. Block `/* … */` comments aren't used
    /// anywhere this corpus walks; if that changes, broaden this.
    /// Mirrors `src/api/mod.rs::route_wiring_tests::strip_line_comments`
    /// — duplicated rather than shared because the existing pattern
    /// in this file duplicates `contains_word` for the same reason
    /// (each `#[cfg(test)]` module is self-contained).
    fn strip_line_comments(src: &str) -> String {
        let mut out = String::with_capacity(src.len());
        for line in src.lines() {
            let cut = match line.find("//") {
                Some(i) => &line[..i],
                None => line,
            };
            out.push_str(cut);
            out.push('\n');
        }
        out
    }

    fn contains_word(haystack: &str, needle: &str) -> bool {
        let mut start = 0;
        while let Some(pos) = haystack[start..].find(needle) {
            let abs = start + pos;
            let before_ok = abs == 0
                || (!haystack.as_bytes()[abs - 1].is_ascii_alphanumeric()
                    && haystack.as_bytes()[abs - 1] != b'_');
            let end = abs + needle.len();
            let after_ok = end >= haystack.len()
                || (!haystack.as_bytes()[end].is_ascii_alphanumeric()
                    && haystack.as_bytes()[end] != b'_');
            if before_ok && after_ok {
                return true;
            }
            start = abs + needle.len();
        }
        false
    }

    #[test]
    fn no_unexpected_orphan_tables() {
        let tables = migration_tables();
        assert!(!tables.is_empty(), "no CREATE TABLE statements parsed?");
        let corpus = code_corpus();

        let mut orphans = Vec::new();
        for t in &tables {
            if ALLOWED_ORPHANS.iter().any(|a| *a == t.as_str()) {
                continue;
            }
            if !contains_word(&corpus, t) {
                orphans.push(t.clone());
            }
        }
        assert!(
            orphans.is_empty(),
            "tables created in migrations but never referenced in src/ ({}): {:?}\n\
             Either wire them up, drop the migration, or add to ALLOWED_ORPHANS with rationale.",
            orphans.len(),
            orphans
        );
    }

    /// Catches the case where a table gets wired up later but the
    /// allowlist entry is forgotten. Failing fast keeps the allowlist
    /// from rotting into a "things we used to ignore" graveyard.
    #[test]
    fn allowlist_entries_are_still_orphans() {
        let corpus = code_corpus();
        let stale: Vec<&&str> = ALLOWED_ORPHANS
            .iter()
            .filter(|t| contains_word(&corpus, t))
            .collect();
        assert!(
            stale.is_empty(),
            "ALLOWED_ORPHANS contains tables that are now referenced in src/: {:?}\n\
             Remove from the allowlist — the orphan check will start covering them.",
            stale
        );
    }
}

/// Cycle #157C: defensive test against unclamped `LIMIT $N` SQL.
///
/// Pre-cycle-#156 `validate_leaderboard_query` capped `?limit=` from
/// above but not from below — `LIMIT -1` hit PostgreSQL with `ERROR:
/// LIMIT must not be negative`. The fix landed in the validator, but
/// nothing in the codebase ensures a *new* raw-SQL `LIMIT $N` site
/// gets the same clamp.
///
/// Walk `src/db/*.rs` for `LIMIT $\d` substrings, assert the count
/// equals the audited set. A new raw-SQL paginator fails this test
/// at pre-commit time, forcing the contributor to either add the
/// site to `AUDITED_LIMIT_SITES` (with rationale) or migrate the
/// new caller's clamp.
///
/// Same shape as `orphan_table_tests`, `route_wiring_tests`,
/// `entity_wiring_tests`, `metric_wiring_tests` — schema-truth
/// defenses keep growing one axis at a time.
#[cfg(test)]
mod limit_clamp_audit {
    /// Each entry: `(file_basename, count_of_LIMIT_$_in_file, why_the_clamp_is_safe)`.
    /// As of cycle #157C, all sites are either bound to a hard-coded
    /// constant or routed through a validator that clamps the limit
    /// into a known-safe range.
    const AUDITED_LIMIT_SITES: &[(&str, usize, &str)] = &[
        // cycle #89: limit is `BLOCKED_USERS_LIST_LIMIT = 50` — hard-coded
        // const in `query_blocked_users`. No client input flows through.
        ("orders.rs", 1, "BLOCKED_USERS_LIST_LIMIT (const)"),
        // cycle #156: limit is routed through `validate_leaderboard_query`
        // which clamps into [1, 50]. Three branches in `get_top_referrers`
        // for weekly / monthly / all-time SQL — same clamped param.
        ("referrals.rs", 3, "clamped via validate_leaderboard_query"),
    ];

    fn count_limit_sites(src: &str) -> usize {
        // Match `LIMIT $1`, `LIMIT $2`, etc. — any `LIMIT $<digit>`.
        let mut count = 0;
        let bytes = src.as_bytes();
        let needle = b"LIMIT $";
        let mut i = 0;
        while i + needle.len() < bytes.len() {
            if &bytes[i..i + needle.len()] == needle && bytes[i + needle.len()].is_ascii_digit() {
                count += 1;
                i += needle.len();
            } else {
                i += 1;
            }
        }
        count
    }

    #[test]
    fn audited_limit_sites_match_codebase() {
        let manifest = env!("CARGO_MANIFEST_DIR");
        let db_dir = std::path::Path::new(manifest).join("src/db");
        let mut found: std::collections::BTreeMap<String, usize> =
            std::collections::BTreeMap::new();
        for entry in std::fs::read_dir(&db_dir)
            .expect("src/db readable")
            .flatten()
        {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("rs") {
                continue;
            }
            let Some(stem) = path.file_name().and_then(|s| s.to_str()) else {
                continue;
            };
            // Skip mod.rs — this audit module's own rationale comments
            // contain the literal `LIMIT $` substring that would otherwise
            // fool the count. Same self-reference hazard as the cycle-#103
            // orphan_table_tests / cycle-#143 route_wiring_tests handle.
            if stem == "mod.rs" {
                continue;
            }
            let Ok(src) = std::fs::read_to_string(&path) else {
                continue;
            };
            let n = count_limit_sites(&src);
            if n > 0 {
                found.insert(stem.to_string(), n);
            }
        }

        // Expected counts from the allowlist.
        let mut expected: std::collections::BTreeMap<String, usize> =
            std::collections::BTreeMap::new();
        for (file, n, _) in AUDITED_LIMIT_SITES {
            expected.insert((*file).to_string(), *n);
        }

        assert_eq!(
            found, expected,
            "raw `LIMIT $N` SQL site count drifted from the cycle-#157C audit.\n\
             If you added a new paginator: confirm the binding `limit` is either a\n\
             hard-coded const or routed through a validator that clamps the value\n\
             into a non-negative range, then update AUDITED_LIMIT_SITES.\n\
             If you removed one: just trim the entry."
        );
    }
}

/// Cycle #146: defensive test against orphan SeaORM entities.
///
/// Each `src/db/entities/<name>.rs` declares a `pub struct Model` plus
/// supporting `Entity`/`Column`/`ActiveModel` types. A `pub mod <name>;`
/// in `src/db/entities/mod.rs` compiles the file, but the compiler
/// won't complain if no production code ever imports it — the entity
/// just sits in the tree, growing dead with the schema.
///
/// Same shape as the four other schema-truth defenses
/// (`orphan_table_tests`, `metric_wiring_tests`, `route_wiring_tests`,
/// CSS-class-tests): walk the source corpus textually, find every
/// entity module, assert the name appears as an identifier outside
/// `src/db/entities/` and outside this file. Stale allowlist entries
/// fail the companion test.
#[cfg(test)]
mod entity_wiring_tests {
    /// Entities defined under `src/db/entities/` but intentionally
    /// not yet wired into production code. Each entry MUST carry a
    /// rationale comment.
    const ALLOWED_UNWIRED_ENTITIES: &[&str] = &[
        // Cycle #146: declared under `src/db/entities/` as pre-work
        // for the SeaORM migration (docs/API_MIGRATION_PLAN.md, cycle
        // #91), but the quest handlers in `src/api/quest.rs` still
        // use raw `Statement::from_sql_and_values(...)` instead of the
        // SeaORM entity API. The entity files (24 and 29 lines) cost
        // nothing — they don't even compile their `Entity` type into
        // any code path. Two ways to resolve:
        //   (a) migrate quest.rs handlers to SeaORM and remove these
        //       entries — the migration plan favours this path.
        //   (b) delete the entity files + `pub mod` declarations in
        //       `src/db/entities/mod.rs` if SeaORM migration is
        //       abandoned for quest endpoints.
        // Until that decision lands, leaving the entities in tree is
        // strictly safer than deleting them (re-deriving from the
        // schema later costs more than the 53-line allowlist tax).
        "quest_place",
        "treasure_hunt",
    ];

    /// Strip `//` line comments before the textual contains check;
    /// otherwise a doc-comment mentioning an entity name in this file
    /// or elsewhere creates a false-positive reference. Mirrors the
    /// helper installed in cycle #145 across the other source-walk
    /// defenses.
    fn strip_line_comments(src: &str) -> String {
        let mut out = String::with_capacity(src.len());
        for line in src.lines() {
            let cut = match line.find("//") {
                Some(i) => &line[..i],
                None => line,
            };
            out.push_str(cut);
            out.push('\n');
        }
        out
    }

    fn entity_modules() -> Vec<String> {
        let manifest = env!("CARGO_MANIFEST_DIR");
        let dir = std::path::Path::new(manifest).join("src/db/entities");
        let mut out = Vec::new();
        for entry in std::fs::read_dir(&dir)
            .expect("src/db/entities readable")
            .flatten()
        {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("rs") {
                continue;
            }
            let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            if stem == "mod" {
                continue;
            }
            let Ok(src) = std::fs::read_to_string(&path) else {
                continue;
            };
            // Sanity: only count files that actually define an entity.
            // A future contributor might drop a helper module under
            // entities/ that has no `Model` — not an entity, skip.
            if src.contains("pub struct Model") {
                out.push(stem.to_string());
            }
        }
        out
    }

    fn corpus_excluding_entities_and_self() -> String {
        let manifest = env!("CARGO_MANIFEST_DIR");
        let src = std::path::Path::new(manifest).join("src");
        let entities_dir = std::path::Path::new(manifest).join("src/db/entities");
        // Exclude `src/db/mod.rs` (where the allowlist + rationale
        // live — a textual mention here would create a false positive
        // for itself even after `strip_line_comments`, e.g. a literal
        // entity name inside a string).
        let self_path = std::path::Path::new(manifest).join("src/db/mod.rs");
        let mut buf = String::new();
        fn walk(
            p: &std::path::Path,
            entities_dir: &std::path::Path,
            self_path: &std::path::Path,
            buf: &mut String,
        ) {
            for entry in std::fs::read_dir(p)
                .expect("readable")
                .filter_map(|e| e.ok())
            {
                let path = entry.path();
                if path == *self_path {
                    continue;
                }
                if path.is_dir() {
                    if path == *entities_dir {
                        continue;
                    }
                    walk(&path, entities_dir, self_path, buf);
                } else if path
                    .extension()
                    .and_then(|s| s.to_str())
                    .is_some_and(|s| s == "rs")
                {
                    if let Ok(s) = std::fs::read_to_string(&path) {
                        buf.push_str(&s);
                        buf.push('\n');
                    }
                }
            }
        }
        walk(&src, &entities_dir, &self_path, &mut buf);
        buf
    }

    /// Same word-boundary helper used by `orphan_table_tests`; the
    /// codebase convention is per-module-duplication of small test
    /// helpers (see also `contains_word` already living twice in this
    /// file).
    fn contains_word(haystack: &str, needle: &str) -> bool {
        let mut start = 0;
        while let Some(pos) = haystack[start..].find(needle) {
            let abs = start + pos;
            let before_ok = abs == 0
                || (!haystack.as_bytes()[abs - 1].is_ascii_alphanumeric()
                    && haystack.as_bytes()[abs - 1] != b'_');
            let end = abs + needle.len();
            let after_ok = end >= haystack.len()
                || (!haystack.as_bytes()[end].is_ascii_alphanumeric()
                    && haystack.as_bytes()[end] != b'_');
            if before_ok && after_ok {
                return true;
            }
            start = abs + needle.len();
        }
        false
    }

    #[test]
    fn every_entity_module_is_referenced() {
        let entities = entity_modules();
        assert!(
            !entities.is_empty(),
            "no entity files found — parser broken or directory layout changed?"
        );
        let raw = corpus_excluding_entities_and_self();
        let corpus = strip_line_comments(&raw);

        let mut unused = Vec::new();
        for name in &entities {
            if ALLOWED_UNWIRED_ENTITIES.iter().any(|a| *a == name.as_str()) {
                continue;
            }
            if !contains_word(&corpus, name) {
                unused.push(name.clone());
            }
        }
        assert!(
            unused.is_empty(),
            "SeaORM entities defined in src/db/entities/ but never referenced \
             from production code ({}): {:?}\n\
             Either wire them up, delete the file, or add to \
             ALLOWED_UNWIRED_ENTITIES with rationale.",
            unused.len(),
            unused,
        );
    }

    #[test]
    fn allowlist_entries_are_still_unused() {
        let raw = corpus_excluding_entities_and_self();
        let corpus = strip_line_comments(&raw);
        let stale: Vec<&&str> = ALLOWED_UNWIRED_ENTITIES
            .iter()
            .filter(|name| contains_word(&corpus, name))
            .collect();
        assert!(
            stale.is_empty(),
            "ALLOWED_UNWIRED_ENTITIES contains entities now referenced in src/: {:?}\n\
             Remove these — the wiring check will start covering them.",
            stale,
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

/// Cycle #123: tests for `parse_db_pool_max_env`. Closure-free helper
/// (takes `Option<String>` directly) since the input shape is the same
/// as `parse_port_env` — match every branch, including the >0 invariant.
#[cfg(test)]
mod db_pool_max_tests {
    use super::parse_db_pool_max_env as p;

    #[test]
    fn unset_returns_default_10() {
        assert_eq!(p(None).unwrap(), 10);
    }

    #[test]
    fn empty_or_whitespace_returns_default() {
        assert_eq!(p(Some("".into())).unwrap(), 10);
        assert_eq!(p(Some("   ".into())).unwrap(), 10);
    }

    #[test]
    fn valid_value_parses() {
        assert_eq!(p(Some("20".into())).unwrap(), 20);
        assert_eq!(p(Some("1".into())).unwrap(), 1);
        assert_eq!(p(Some(" 50 ".into())).unwrap(), 50);
    }

    #[test]
    fn malformed_errors() {
        // Word, float, negative — none of these are u32.
        assert!(p(Some("ten".into())).is_err());
        assert!(p(Some("10.5".into())).is_err());
        assert!(p(Some("-1".into())).is_err());
    }

    #[test]
    fn zero_errors() {
        // 0 would deadlock the first query — explicit reject so the
        // operator sees the cause, not a mysterious hang.
        let e = p(Some("0".into())).unwrap_err().to_string();
        assert!(
            e.contains("must be >= 1"),
            "expected '>= 1' in error, got: {}",
            e
        );
    }
}
