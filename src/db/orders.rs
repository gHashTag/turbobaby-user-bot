use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Order {
    pub id: String,
    pub telegram_id: Option<i64>,
    pub customer_name: Option<String>,
    pub customer_phone: Option<String>,
    pub customer_telegram: Option<String>,
    pub items: Value,
    pub subtotal: f64,
    pub bonus_used: f64,
    pub total: f64,
    pub status: String,
    pub shop_id: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

// Cycle #85: SeaORM `order::Model` → wire `Order` conversion. Wire shape
// preserves the f64 finite-clamp from the old `from_row` path so a
// `NUMERIC → DOUBLE PRECISION` migration artifact (NaN/Inf in the column)
// can't poison clients. Entity has identical column set; `DateTimeWithTimeZone`
// converts to `chrono::DateTime<chrono::Utc>` via `.into()` (re-zones).
impl From<crate::db::entities::order::Model> for Order {
    fn from(m: crate::db::entities::order::Model) -> Self {
        let clamp = |v: f64| -> f64 {
            if v.is_finite() {
                v.max(0.0)
            } else {
                0.0
            }
        };
        Self {
            id: m.id,
            telegram_id: m.telegram_id,
            customer_name: m.customer_name,
            customer_phone: m.customer_phone,
            customer_telegram: m.customer_telegram,
            items: m.items,
            subtotal: clamp(m.subtotal),
            bonus_used: clamp(m.bonus_used),
            total: clamp(m.total),
            status: m.status,
            shop_id: m.shop_id,
            created_at: m.created_at.with_timezone(&chrono::Utc),
        }
    }
}

/// Atomically mark an order as completed and update the customer's loyalty profile.
/// Returns `Some((telegram_id, is_first_order))` if the order was newly completed,
/// or `None` if it was already completed (idempotent).
///
/// Cycle #88: migrated to SeaORM transaction. The flow stays identical —
/// FOR UPDATE the order row (race guard against concurrent admin actions),
/// count prior completions for `is_first`, flip status, accumulate
/// total_spent on loyalty_profiles via upsert, recompute tier with a
/// CASE-WHEN that consults loyalty_config thresholds. Drop = auto-rollback;
/// only `commit()` on the happy path.
/// Outcome of a successful order completion. Returned so callers can
/// drive user-facing side effects (Telegram notifications, etc.)
/// without re-running DB lookups the completion function already did.
#[derive(Debug, Clone)]
#[allow(unreachable_pub)] // Returned by `complete_order_and_update_loyalty` which integration tests use.
pub struct OrderCompletion {
    pub customer_telegram_id: i64,
    /// Currently unread — kept for analytics / future-caller hooks
    /// (e.g. "first-order welcome bonus" or signup-completion
    /// metrics). The cycle-#171 callers use
    /// `referral_bonus_credited.is_some()` as a stricter predicate
    /// for the referral-notification path.
    #[allow(dead_code)]
    pub is_first_order: bool,
    /// `Some(amount)` if this was a first order from a referred user
    /// AND `confirm_referral` succeeded. `None` if not-first,
    /// not-referred, or the credit failed (logged inside the
    /// function; callers receive `None` and skip downstream side
    /// effects like referrer notifications).
    pub referral_bonus_credited: Option<f64>,
}

#[allow(unreachable_pub)] // Used by tests/integration_use_bonus.rs to set up scenarios.
pub async fn complete_order_and_update_loyalty(
    orm: &sea_orm::DatabaseConnection,
    order_id: &str,
) -> Result<Option<OrderCompletion>, sea_orm::DbErr> {
    use crate::db::entities::{
        loyalty_profile::{ActiveModel as LpAm, Column as LpCol, Entity as LpEntity},
        order::{Column as OrderCol, Entity as OrderEntity},
    };
    use sea_orm::sea_query::OnConflict;
    use sea_orm::{
        ActiveValue::Set, ColumnTrait, ConnectionTrait, DbBackend, EntityTrait, QueryFilter,
        QuerySelect, Statement, TransactionTrait,
    };

    let tx = orm.begin().await?;

    // 1. Lock the order row (FOR UPDATE) and read its current state.
    let order = OrderEntity::find_by_id(order_id.to_string())
        .lock_exclusive()
        .one(&tx)
        .await?;

    let Some(order) = order else {
        tx.commit().await?;
        return Ok(None);
    };
    if order.status == "completed" {
        // Idempotent: already completed.
        tx.commit().await?;
        return Ok(None);
    }
    let Some(cid) = order.telegram_id else {
        // No customer attribution — flip status but no loyalty side-effect.
        OrderEntity::update_many()
            .col_expr(
                OrderCol::Status,
                sea_orm::sea_query::Expr::value("completed"),
            )
            .filter(OrderCol::Id.eq(order_id))
            .exec(&tx)
            .await?;
        tx.commit().await?;
        return Ok(None);
    };
    let total = if order.total.is_finite() {
        order.total.max(0.0)
    } else {
        0.0
    };

    // 2. Count prior completions BEFORE the flip — gives the correct
    //    `is_first_order` even under concurrent admin actions (the FOR
    //    UPDATE earlier serialises this).
    let count_row = tx
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT COUNT(*) AS cnt FROM orders WHERE telegram_id = $1 AND status = 'completed' AND id != $2",
            [cid.into(), order_id.into()],
        ))
        .await?
        .ok_or_else(|| sea_orm::DbErr::Custom("COUNT(*) returned no rows".into()))?;
    // Fail loud: a silent `.unwrap_or(0)` here would mark an *existing*
    // customer's order as their first on any read error, wrongly granting
    // first-order tier promotion + referral-bonus eligibility. The fn already
    // returns DbErr, so propagate.
    let count_before: i64 = count_row.try_get("", "cnt")?;
    let is_first = count_before == 0;

    // 3. Flip the order to completed.
    OrderEntity::update_many()
        .col_expr(
            OrderCol::Status,
            sea_orm::sea_query::Expr::value("completed"),
        )
        .filter(OrderCol::Id.eq(order_id))
        .exec(&tx)
        .await?;

    // 4. Accumulate total_spent + first_purchase_at on the loyalty profile.
    //    `total_spent = COALESCE(existing, 0) + new_total` via column-expr
    //    UPDATE inside the OnConflict clause (pattern #10 in seaorm-patterns).
    let lp_am = LpAm {
        telegram_id: Set(cid),
        total_spent: Set(Some(total)),
        first_purchase_at: Set(Some(chrono::Utc::now().into())),
        ..Default::default()
    };
    LpEntity::insert(lp_am)
        .on_conflict(
            OnConflict::column(LpCol::TelegramId)
                .value(
                    LpCol::TotalSpent,
                    sea_orm::sea_query::Expr::cust_with_values(
                        "COALESCE(loyalty_profiles.total_spent, 0) + $1",
                        [total],
                    ),
                )
                .value(
                    LpCol::FirstPurchaseAt,
                    sea_orm::sea_query::Expr::cust(
                        "COALESCE(loyalty_profiles.first_purchase_at, NOW())",
                    ),
                )
                .to_owned(),
        )
        .exec(&tx)
        .await?;

    // 5. Recompute tier from the loyalty_config JSONB thresholds. The
    //    CASE-WHEN-subquery shape is too custom for the typed builder; raw
    //    Statement (pattern #15) keeps the original logic intact.
    tx.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "UPDATE loyalty_profiles SET tier = CASE \
            WHEN loyalty_profiles.total_spent >= (SELECT (config->>'gold_threshold')::float8 FROM loyalty_config WHERE id = 1 LIMIT 1) THEN 'gold' \
            WHEN loyalty_profiles.total_spent >= (SELECT (config->>'silver_threshold')::float8 FROM loyalty_config WHERE id = 1 LIMIT 1) THEN 'silver' \
            WHEN loyalty_profiles.total_spent >= (SELECT (config->>'bronze_threshold')::float8 FROM loyalty_config WHERE id = 1 LIMIT 1) THEN 'bronze' \
            ELSE 'none' \
         END \
         WHERE telegram_id = $1",
        [cid.into()],
    ))
    .await?;

    // 6. Cycle #168: seed a garden plant if this order contains a
    //    strain AND the user has no active plant. Closes the gap
    //    flagged by user-report — the Garden UI says "Order a strain
    //    to get your first seed!" but `complete_order_and_update_loyalty`
    //    never seeded plants for orders completed AFTER the cycle-#15
    //    one-shot backfill (migration 026) ran. The backfill SQL was
    //    documented as a forward-design but the matching forward
    //    write was never landed.
    //
    //    Match migration 026's shape:
    //      * pick the FIRST strain item in `order.items` (a JSONB array)
    //      * stage = 'seed', water_count = 0
    //      * INSERT ... WHERE NOT EXISTS (active plant) — preserves
    //        the cycle-#94 one-active-plant-at-a-time invariant
    //
    //    Side-effect contract: an order with no strain items (pure
    //    accessory/tea/set orders) doesn't plant — same as the
    //    backfill, intentional.
    // Wave #66: seed precedence is one SSOT now. This was a line-for-line
    // logical clone of `first_seedable_item` (the `force_seed` endpoint) and
    // migration 037's SQL backfill — three copies of "which order item becomes
    // the seed", each drifting independently (that's the 026→036→037 hotfix
    // history). Call the pure core so the completion side-effect, force_seed,
    // and the backfill can't diverge again. Precedence strain→set→accessory→tea
    // within an item is locked vs SQL 037 by `seed_keys_match_sql_backfill_037`.
    let strain_seed = crate::trios::garden::first_seedable_item(&order.items);
    if let Some((strain_id, strain_name)) = strain_seed {
        let plant_id = uuid::Uuid::new_v4().to_string();
        let planted_at = chrono::Utc::now().timestamp_millis();
        // Bug fix (discounts too frequent): also refuse to seed within
        // POST_HARVEST_COOLDOWN_MS of the user's most recent harvest, so a new
        // garden discount can't be earned more than ~once per day. The existing
        // "no active plant" guard alone allowed back-to-back reward cycles.
        let harvest_cooldown_floor =
            planted_at.saturating_sub(crate::trios::garden::POST_HARVEST_COOLDOWN_MS);
        tx.execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            // B1: only seed a plant whose product still exists AND is available
            // in the LIVE catalog. Order JSON is a stale snapshot — a strain the
            // shop later deleted/hid used to be seeded as a "phantom" not in the
            // menu. The EXISTS gate (any of the 6 catalogs, available=true) keeps
            // the garden in sync with what's actually buyable.
            "INSERT INTO garden_plants \
                  (id, user_id, strain_id, strain_name, current_stage, planted_at, is_completed, water_count) \
             SELECT $1, $2, $3, $4, 'seed', $5, false, 0 \
             WHERE NOT EXISTS ( \
                 SELECT 1 FROM garden_plants WHERE user_id = $2 AND is_completed = false \
             ) \
             AND NOT EXISTS ( \
                 SELECT 1 FROM garden_plants \
                 WHERE user_id = $2 AND harvested_at IS NOT NULL AND harvested_at > $6 \
             ) \
             AND EXISTS ( \
                 SELECT 1 FROM strains        WHERE id = $3 AND is_available = true \
                 UNION ALL SELECT 1 FROM sets           WHERE id = $3 AND is_available = true \
                 UNION ALL SELECT 1 FROM accessory_sets WHERE id = $3 AND is_available = true \
                 UNION ALL SELECT 1 FROM tea_sets       WHERE id = $3 AND is_available = true \
                 UNION ALL SELECT 1 FROM accessories    WHERE id = $3 AND is_available = true \
                 UNION ALL SELECT 1 FROM tea_products   WHERE id = $3 AND is_available = true \
             )",
            [
                plant_id.into(),
                cid.to_string().into(),
                strain_id.into(),
                strain_name.into(),
                planted_at.into(),
                harvest_cooldown_floor.into(),
            ],
        ))
        .await?;
    }

    tx.commit().await?;

    // 7. Cycle #171: referral bonus credit, lifted from the two
    //    completion callers into the canonical completion function.
    //    Pre-cycle, only the bot-callback path (`bot/callbacks.rs`)
    //    called `confirm_referral` — cycle #170 mirrored it in
    //    `update_order_status`, but a future third completion path
    //    (payment webhook, batch completion, etc.) could miss it
    //    again. Lifting closes the bug class.
    //
    //    Side-effect chain: read referral_bonus from loyalty_config,
    //    call confirm_referral (which credits the referrer's
    //    bonus_balance + writes a `bonus_transactions` ledger row
    //    + sets `referral_events.status = 'paid'`). Outside the
    //    completion tx — confirm_referral has its own transaction
    //    semantics and a failure here shouldn't roll back the order
    //    completion (the order is already committed; referrals can
    //    be reconciled).
    let referral_bonus_credited: Option<f64> = if is_first {
        let bonus = match orm
            .query_one(Statement::from_string(
                DbBackend::Postgres,
                "SELECT config->>'referral_bonus' AS bonus FROM loyalty_config WHERE id = 1"
                    .to_string(),
            ))
            .await
        {
            Ok(Some(row)) => row
                .try_get::<Option<String>>("", "bonus")
                .ok()
                .flatten()
                .and_then(|s| s.parse().ok())
                .unwrap_or(200.0),
            Ok(None) | Err(_) => 200.0,
        };
        match crate::db::referrals::confirm_referral(orm, cid, bonus).await {
            Ok(_) => Some(bonus),
            Err(e) => {
                tracing::error!(
                    "complete_order: confirm_referral failed for cid={}: {:?}",
                    cid,
                    e
                );
                None
            }
        }
    } else {
        None
    };

    Ok(Some(OrderCompletion {
        customer_telegram_id: cid,
        is_first_order: is_first,
        referral_bonus_credited,
    }))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(unreachable_pub)] // Used as a field of pub structs in src/api/*.rs request bodies; pub(crate) would cascade.
pub struct OrderItem {
    pub strain_id: Option<String>,
    pub strain_name: Option<String>,
    pub accessory_id: Option<String>,
    pub accessory_name: Option<String>,
    pub tea_id: Option<String>,
    pub tea_name: Option<String>,
    pub set_id: Option<String>,
    pub set_name: Option<String>,
    pub quantity: f64,
    pub is_set: Option<bool>,
    pub is_accessory: Option<bool>,
    pub is_tea: Option<bool>,
    pub is_tea_set: Option<bool>,
    /// A3: per-drink fulfillment — "dine_in" (на месте) or "takeaway" (с собой).
    /// Only set for drink (tea) items; None for everything else. Stored in the
    /// order `items` JSONB so staff see how each drink should be served.
    #[serde(default)]
    pub fulfillment: Option<String>,
}

// ─── Audit-table TTL sweeps (cycles #58 / #63 / #66) ──────────────────────
//
// The three append-only audit tables — order_idempotency_keys (24 h),
// order_fraud_events (30 d), block_history (90 d) — each got their own
// near-identical sweep builder. After the third copy (cycle #66) the
// rule-of-three (Fowler, "Refactoring" §3.4) said: extract. The generic
// helper below is the only place the actual DELETE template lives. Each
// specific builder remains as a thin alias so call-sites stay
// self-documenting at their use point.

/// Build the canonical `DELETE` SQL fragment for any append-only audit
/// table swept by `created_at`. Pure, unit-testable.
///
/// **Security note:** `table` is interpolated as a SQL identifier (cannot
/// be parameterised). Callers MUST pass a static `&str` literal —
/// internal-only at the time of writing. `interval_clause` is the body
/// of `INTERVAL '...'` (e.g. `"24 hours"`, `"30 days"`).
#[allow(dead_code)] // Called only from main.rs's spawn_ttl_sweep loop (bin); lib has no user.
pub(crate) fn audit_sweep_sql(table: &str, interval_clause: &str) -> String {
    format!(
        "DELETE FROM {} WHERE created_at < NOW() - INTERVAL '{}'",
        table, interval_clause
    )
}

/// Build the `DELETE` SQL fragment for the idempotency-key TTL sweep.
/// Thin alias over [`audit_sweep_sql`] so the existing tests + callers
/// keep their idempotency-specific naming.
#[allow(dead_code)] // Called only from main.rs's spawn_ttl_sweep loop (bin); lib has no user.
pub(crate) fn idempotency_sweep_sql(retention_hours: u32) -> String {
    audit_sweep_sql(
        "order_idempotency_keys",
        &format!("{} hours", retention_hours),
    )
}

/// Delete `order_idempotency_keys` rows older than `retention_hours`.
/// Returns the number of rows deleted (for metrics / structured logs).
///
/// Idempotent and safe to run concurrently with `create_order` — Postgres
/// handles concurrent DELETE/INSERT on the same table cleanly, and the
/// 24 h cutoff is far older than any in-flight order's retry window.
///
/// Cycle #86: signature `&Pool` → `&sea_orm::DatabaseConnection`. The
/// SQL builder (`idempotency_sweep_sql`) stays — its INTERVAL literal
/// is unit-tested in the same module and switching the call site to
/// `Statement::from_string` is the minimal change that preserves the
/// builder's contract.
#[allow(dead_code)] // Called only from main.rs's spawn_ttl_sweep loop (bin); lib has no user.
pub(crate) async fn cleanup_old_idempotency_keys(
    orm: &sea_orm::DatabaseConnection,
    retention_hours: u32,
) -> Result<u64, sea_orm::DbErr> {
    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    let sql = idempotency_sweep_sql(retention_hours);
    let res = orm
        .execute(Statement::from_string(DbBackend::Postgres, sql))
        .await?;
    Ok(res.rows_affected())
}

// ─── Fraud-event TTL sweep (cycle #63 / A) ───────────────────────────────
//
// `order_fraud_events` is append-only (cycle #59). 30-day retention is
// longer than the idempotency table's 24h because trend / pattern review
// often goes back weeks (Pareto offenders, repeat tampering, etc.), but
// after 30 days the data is more noise than signal — admin acts on the
// /engage 24h window. Same pure-builder pattern as the idempotency sweep
// so the INTERVAL literal is unit-testable.

/// Build the `DELETE` SQL fragment for the fraud-event TTL sweep.
/// Thin alias over [`audit_sweep_sql`].
#[allow(dead_code)] // Called only from main.rs's spawn_ttl_sweep loop (bin); lib has no user.
pub(crate) fn fraud_events_sweep_sql(retention_days: u32) -> String {
    audit_sweep_sql("order_fraud_events", &format!("{} days", retention_days))
}

/// Delete `order_fraud_events` rows older than `retention_days`. Returns
/// the row count for the spawn-loop's structured log line.
///
/// Cycle #86: same migration pattern as [`cleanup_old_idempotency_keys`].
#[allow(dead_code)] // Called only from main.rs's spawn_ttl_sweep loop (bin); lib has no user.
pub(crate) async fn cleanup_old_fraud_events(
    orm: &sea_orm::DatabaseConnection,
    retention_days: u32,
) -> Result<u64, sea_orm::DbErr> {
    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    let sql = fraud_events_sweep_sql(retention_days);
    let res = orm
        .execute(Statement::from_string(DbBackend::Postgres, sql))
        .await?;
    Ok(res.rows_affected())
}

// ─── block_history TTL sweep (cycle #66) ─────────────────────────────────
//
// `block_history` is the third append-only audit table (after
// `order_idempotency_keys` from cycle #58/A and `order_fraud_events` from
// cycle #63/A). Same shape as both: pure SQL builder, async cleanup, daily
// tokio::spawn loop. Retention is the longest of the three — 90 days —
// because block decisions are operational/compliance records that admins
// genuinely look back at across quarters ("did we wrongly block user X
// three months ago?"). Beyond 90 days a row is more noise than signal.

/// Build the `DELETE` SQL fragment for the block-history TTL sweep.
/// Thin alias over [`audit_sweep_sql`].
#[allow(dead_code)] // Called only from main.rs's spawn_ttl_sweep loop (bin); lib has no user.
pub(crate) fn block_history_sweep_sql(retention_days: u32) -> String {
    audit_sweep_sql("block_history", &format!("{} days", retention_days))
}

/// Delete `block_history` rows older than `retention_days`. Returns the
/// number of rows deleted for the spawn-loop's structured log line.
///
/// Cycle #86: same migration pattern as [`cleanup_old_idempotency_keys`].
#[allow(dead_code)] // Called only from main.rs's spawn_ttl_sweep loop (bin); lib has no user.
pub(crate) async fn cleanup_old_block_history(
    orm: &sea_orm::DatabaseConnection,
    retention_days: u32,
) -> Result<u64, sea_orm::DbErr> {
    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    let sql = block_history_sweep_sql(retention_days);
    let res = orm
        .execute(Statement::from_string(DbBackend::Postgres, sql))
        .await?;
    Ok(res.rows_affected())
}

// ─── Fraud-event audit log (cycle #59) ────────────────────────────────────
//
// Every 422 reject in `create_order` emits a structured `tracing::warn!`,
// but those lines live in stdout — invisible to an admin who only opens
// the Telegram bot. This append-only table mirrors the same data so the
// `/engage` panel can show "Suspicious activity (24h)" without grep.
//
// Inserts are best-effort: a failure here MUST NOT block the reject path
// the customer is already seeing. Worst case we lose a row, not a request.

/// Stable codes for `order_fraud_events.code`. They mirror the JSON error
/// codes the client UI may eventually parse for friendly messages.
pub(crate) const FRAUD_CODE_SUBTOTAL_MISMATCH: &str = "subtotal_mismatch";
pub(crate) const FRAUD_CODE_UNKNOWN_ITEM: &str = "unknown_item";
pub(crate) const FRAUD_CODE_UNAVAILABLE: &str = "unavailable";
pub(crate) const FRAUD_CODE_MALFORMED: &str = "malformed";

/// Number of `subtotal_mismatch` events in the lookback window that triggers
/// an automatic block (cycle #60). 3 is conservative: a real shopper hitting
/// stale-cart prices would clear and retry, not produce 3+ price tampering
/// events in a day. 1–2 might be a buggy client or race condition; 3+ is a
/// sustained pattern that warrants an automatic stop-the-bleeding action.
pub(crate) const FRAUD_AUTO_BLOCK_THRESHOLD: i64 = 3;
/// Lookback window for the auto-block decision. 24 h matches the rest of
/// the audit pipeline (/engage panel, idempotency TTL).
pub(crate) const FRAUD_AUTO_BLOCK_LOOKBACK_HOURS: i32 = 24;

/// Pure decision: should the auto-blocker engage given the count of
/// `subtotal_mismatch` events seen for this user in the lookback window?
/// Extracted as a standalone function so the threshold semantics are
/// table-tested instead of buried inside an async DB-bound function.
pub(crate) fn should_auto_block_for_fraud(subtotal_mismatch_events_24h: i64) -> bool {
    subtotal_mismatch_events_24h >= FRAUD_AUTO_BLOCK_THRESHOLD
}

/// Insert one row into `order_fraud_events`. Returns `Result<(), ...>` so
/// the caller can log a warning, but the caller MUST NOT propagate — the
/// 422 reject path is more important than the audit row landing.
///
/// Cycle #60: after a successful `subtotal_mismatch` insert with a known
/// telegram_id, this function also fires the auto-block check. The check
/// runs serially (not spawned) so a race between two concurrent mismatches
/// can't both decide independently to skip the block — Postgres
/// `pg_advisory_xact_lock` inside the helper serialises them.
pub(crate) async fn record_fraud_event(
    orm: &sea_orm::DatabaseConnection,
    telegram_id: Option<i64>,
    code: &str,
    catalog: Option<&str>,
    item_id: Option<&str>,
    claimed_subtotal: Option<f64>,
    expected_subtotal: Option<f64>,
) -> Result<(), sea_orm::DbErr> {
    use crate::db::entities::order_fraud_event::{ActiveModel as FraudAm, Entity as FraudEntity};
    use sea_orm::{ActiveValue::Set, EntityTrait};
    // Cycle #89: SeaORM ActiveModel insert. The `drop(client)` dance for
    // freeing the deadpool connection before the recursive call to
    // `auto_block_for_fraud` is gone — `DatabaseConnection` clones cheaply
    // and SeaORM handles connection lifecycle internally.
    let am = FraudAm {
        telegram_id: Set(telegram_id),
        code: Set(code.to_string()),
        catalog: Set(catalog.map(|s| s.to_string())),
        item_id: Set(item_id.map(|s| s.to_string())),
        claimed_subtotal: Set(claimed_subtotal),
        expected_subtotal: Set(expected_subtotal),
        ..Default::default()
    };
    FraudEntity::insert(am).exec(orm).await?;

    // Auto-block trigger (cycle #60). Only fires for the strongest fraud
    // signal — `subtotal_mismatch` — and only when we have a telegram_id
    // to block. Anonymous mismatches are caught by the per-IP rate limit.
    if code == FRAUD_CODE_SUBTOTAL_MISMATCH {
        if let Some(tid) = telegram_id {
            if let Err(e) = auto_block_for_fraud(orm, tid).await {
                tracing::warn!("auto_block check failed for telegram_id={}: {}", tid, e);
            }
        }
    }
    Ok(())
}

/// Parse a `/unblock <arg>` argument into a positive telegram_id.
///
/// Pure helper extracted so the parsing rules can be exhaustively tested
/// (off-by-one, leading whitespace, dot decimals, `0` and negative) without
/// needing a Telegram bot harness. The handler then dispatches to
/// [`manual_unblock`] using the returned `i64`.
///
/// Returns `None` for empty, non-numeric, zero, or negative inputs.
/// Negative is rejected explicitly so a future SQL rewrite like
/// `WHERE telegram_id > $1` can't accidentally unblock the entire user
/// base when admin types `/unblock -1`.
pub(crate) fn parse_unblock_arg(arg: &str) -> Option<i64> {
    let trimmed = arg.trim();
    if trimmed.is_empty() {
        return None;
    }
    let parsed: i64 = trimmed.parse().ok()?;
    if parsed <= 0 {
        return None;
    }
    Some(parsed)
}

/// One row from `query_blocked_users` — currently-blocked user plus the
/// most recent fraud event we have on file for them (when any).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct BlockedUserRow {
    pub telegram_id: i64,
    /// `created_at` of the most recent `order_fraud_events` row for this
    /// user. `None` when blocked by hand (e.g. SQL or pre-cycle-#60 setups)
    /// — the audit table simply has no row to point at.
    pub last_fraud_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Code from that same row. Lets admin see "what got them blocked"
    /// at a glance — usually `subtotal_mismatch`.
    pub last_fraud_code: Option<String>,
}

/// Cap on how many rows `/blocks` lists in one Telegram message. 50 keeps
/// the message well under Telegram's 4096-char limit even with long codes.
pub(crate) const BLOCKED_USERS_LIST_LIMIT: i64 = 50;

/// Fetch currently-blocked users enriched with the most recent fraud-event
/// context for each. Single round-trip — `LEFT JOIN LATERAL` lets the
/// planner index-scan `idx_fraud_events_created_at` per user instead of
/// doing a window scan across the whole table.
pub(crate) async fn query_blocked_users(
    orm: &sea_orm::DatabaseConnection,
    limit: i64,
) -> Result<Vec<BlockedUserRow>, sea_orm::DbErr> {
    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    // Cycle #89: raw Statement (pattern #15) — LATERAL JOIN + NULLS LAST
    // ordering doesn't have a typed builder equivalent in SeaORM 1.1.
    let rows = orm
        .query_all(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT lp.telegram_id, \
                    fe.created_at AS last_fraud_at, \
                    fe.code       AS last_fraud_code \
             FROM loyalty_profiles lp \
             LEFT JOIN LATERAL ( \
                 SELECT created_at, code \
                 FROM order_fraud_events \
                 WHERE telegram_id = lp.telegram_id \
                 ORDER BY created_at DESC LIMIT 1 \
             ) fe ON TRUE \
             WHERE lp.is_blocked = TRUE \
             ORDER BY fe.created_at DESC NULLS LAST, lp.telegram_id \
             LIMIT $1",
            [limit.into()],
        ))
        .await?;
    let mut out = Vec::with_capacity(rows.len());
    for r in &rows {
        out.push(BlockedUserRow {
            telegram_id: r.try_get::<i64>("", "telegram_id").unwrap_or(0),
            last_fraud_at: r
                .try_get::<Option<chrono::DateTime<chrono::Utc>>>("", "last_fraud_at")
                .ok()
                .flatten(),
            last_fraud_code: r
                .try_get::<Option<String>>("", "last_fraud_code")
                .ok()
                .flatten(),
        });
    }
    Ok(out)
}

/// Format the `/blocks` response as Telegram-flavored HTML. Pure helper —
/// testable without a Telegram client or a DB. Caller wraps in
/// `parse_mode(Html)`.
///
/// `total_count` may exceed `rows.len()` when the query was truncated to
/// `BLOCKED_USERS_LIST_LIMIT`; in that case a tail line tells admin how
/// many entries didn't fit so they don't think the list is complete.
pub(crate) fn format_blocks_message(rows: &[BlockedUserRow], total_count: usize) -> String {
    use crate::util::html_escape;
    if rows.is_empty() {
        return "🛡 <b>Blocked users</b>\n━━━━━━━━━━━━━━━━\n✅ <i>none</i>".to_string();
    }
    let mut s = String::from("🛡 <b>Blocked users</b>\n━━━━━━━━━━━━━━━━\n");
    for r in rows {
        let when = r
            .last_fraud_at
            .map(|t| t.format("%Y-%m-%d %H:%M UTC").to_string())
            .unwrap_or_else(|| "—".to_string());
        let code = r
            .last_fraud_code
            .as_deref()
            .map(html_escape)
            .unwrap_or_else(|| "—".to_string());
        s.push_str(&format!(
            "👤 <code>{}</code> · {}\n   📝 {}\n",
            r.telegram_id, when, code,
        ));
    }
    if total_count > rows.len() {
        s.push_str(&format!(
            "\n…and <b>{}</b> more (use SQL for full list)",
            total_count - rows.len()
        ));
    }
    s.push_str("\n<i>Unblock with /unblock &lt;telegram_id&gt;</i>");
    s
}

// ─── block_history audit log (cycle #64) ─────────────────────────────────
//
// migration 031 (`block_history`) has been on disk since cycle #46, but no
// code wrote to it. Cycles #60 and #61 emit `tracing::warn!` / `info!` on
// every auto-block and `/unblock` action, but those lines disappear into
// stdout. This wires every state transition into the table so:
//   * Compliance: "who unblocked telegram_id 42 on date X" is one query.
//   * Forensics: scan repeat offenders across multiple block cycles.
//   * Future /engage extensions can show "N auto-blocks today".

/// Stable values for `block_history.action`. Kept here next to the writers
/// so a future cycle that adds a new transition (e.g. admin manual block)
/// has a single source of truth.
pub(crate) const BLOCK_ACTION_AUTO: &str = "auto_block";
pub(crate) const BLOCK_ACTION_UNBLOCK: &str = "unblock";

/// Append one row to `block_history`. Returns `Result<(), _>` so callers can
/// log a warning on failure, but the caller MUST swallow — losing an audit
/// row is preferable to blocking the underlying block / unblock action.
pub(crate) async fn record_block_history(
    orm: &sea_orm::DatabaseConnection,
    telegram_id: i64,
    action: &str,
    reason: Option<&str>,
    actor_admin_id: Option<i64>,
) -> Result<(), sea_orm::DbErr> {
    use crate::db::entities::block_history::{ActiveModel as BhAm, Entity as BhEntity};
    use sea_orm::{ActiveValue::Set, EntityTrait};
    let am = BhAm {
        telegram_id: Set(telegram_id),
        action: Set(action.to_string()),
        reason: Set(reason.map(|s| s.to_string())),
        actor_admin_id: Set(actor_admin_id),
        ..Default::default()
    };
    BhEntity::insert(am).exec(orm).await?;
    Ok(())
}

/// Manually clear `is_blocked` for `telegram_id`. Counterpart to the
/// automatic blocker in [`auto_block_for_fraud`] — admins drive this via
/// the `/unblock` bot command (cycle #61).
///
/// Returns `Ok(true)` if the row was actually flipped, `Ok(false)` if the
/// user was not blocked (or has no profile). The `AND is_blocked = true`
/// guard means an admin can spam `/unblock 123` without each call writing
/// a fresh `is_blocked = false` UPDATE.
pub(crate) async fn manual_unblock(
    orm: &sea_orm::DatabaseConnection,
    telegram_id: i64,
) -> Result<bool, sea_orm::DbErr> {
    use crate::db::entities::loyalty_profile::{Column as LpCol, Entity as LpEntity};
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
    // The `is_blocked = TRUE` guard means an admin can spam `/unblock 123`
    // without each call writing a fresh UPDATE.
    let result = LpEntity::update_many()
        .col_expr(LpCol::IsBlocked, sea_orm::sea_query::Expr::value(false))
        .filter(LpCol::TelegramId.eq(telegram_id))
        .filter(LpCol::IsBlocked.eq(true))
        .exec(orm)
        .await?;
    Ok(result.rows_affected > 0)
}

/// If the user has accumulated >= `FRAUD_AUTO_BLOCK_THRESHOLD`
/// `subtotal_mismatch` events in the last `FRAUD_AUTO_BLOCK_LOOKBACK_HOURS`,
/// flip `loyalty_profiles.is_blocked = true`. After that the existing
/// `check_not_blocked` guard at the top of `create_order` rejects every
/// subsequent attempt with 403 before any DB work happens.
///
/// Returns `Ok(true)` when the user was newly blocked by this call,
/// `Ok(false)` when no action was needed (count below threshold or already
/// blocked). Defensive: an error here is logged by the caller and ignored —
/// auto-block is defence in depth, not the primary 422 protection.
async fn auto_block_for_fraud(
    orm: &sea_orm::DatabaseConnection,
    telegram_id: i64,
) -> Result<bool, sea_orm::DbErr> {
    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    // Count only `subtotal_mismatch` — other codes (`unavailable`,
    // `unknown_item`, `malformed`) are mostly stale-cart / client-bug and
    // would auto-block honest users. Subtotal mismatch alone is the
    // can't-happen-by-accident signal.
    let row = orm
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT COUNT(*)::bigint AS n FROM order_fraud_events \
             WHERE telegram_id = $1 \
               AND code = $2 \
               AND created_at > NOW() - make_interval(hours => $3)",
            [
                telegram_id.into(),
                FRAUD_CODE_SUBTOTAL_MISMATCH.into(),
                FRAUD_AUTO_BLOCK_LOOKBACK_HOURS.into(),
            ],
        ))
        .await?
        .ok_or_else(|| sea_orm::DbErr::Custom("auto_block_for_fraud: COUNT no rows".into()))?;
    // Fail loud: a silent `.unwrap_or(0)` here would read as "0 fraud events"
    // on any error → the auto-block gate silently disables itself (fail-open
    // security hole), letting a fraudster past the threshold. The fn returns
    // DbErr, so propagate and let the caller decide.
    let count: i64 = row.try_get("", "n")?;
    if !should_auto_block_for_fraud(count) {
        return Ok(false);
    }
    // UPSERT so anonymous-ish accounts without a loyalty profile row still
    // get blocked the moment they cross the threshold. The
    // `WHERE loyalty_profiles.is_blocked = FALSE` action filter is the
    // "only flip if currently unblocked" guard — keeps `rows > 0` as the
    // "newly blocked" signal that drives the audit log + warn line.
    //
    // Cycle #89 note: SeaORM's `OnConflict::value(...)` doesn't support a
    // conditional `WHERE` on the action clause; the raw Statement keeps
    // the original semantics intact. This is pattern #15 (aggregates +
    // unusual conflict shapes use raw Statement).
    let result = orm
        .execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "INSERT INTO loyalty_profiles (telegram_id, bonus_balance, total_spent, is_blocked) \
             VALUES ($1, 0, 0, TRUE) \
             ON CONFLICT (telegram_id) DO UPDATE SET is_blocked = TRUE \
             WHERE loyalty_profiles.is_blocked = FALSE",
            [telegram_id.into()],
        ))
        .await?;
    let newly_blocked = result.rows_affected() > 0;
    if newly_blocked {
        tracing::warn!(
            telegram_id,
            count_24h = count,
            threshold = FRAUD_AUTO_BLOCK_THRESHOLD,
            "auto_block: user blocked for repeated subtotal_mismatch"
        );
        // Cycle #89: no more drop(client) dance — SeaORM connections are
        // pooled internally and `&orm` clones for free.
        if let Err(e) = record_block_history(
            orm,
            telegram_id,
            BLOCK_ACTION_AUTO,
            Some("subtotal_mismatch_threshold"),
            None, // server-initiated, no admin actor
        )
        .await
        {
            tracing::warn!(
                telegram_id,
                "auto_block: block_history audit insert failed: {}",
                e
            );
        }
    }
    Ok(newly_blocked)
}

/// 24-hour aggregate for the `/engage` orders block (cycle #63 / B).
#[derive(Debug, Default, Clone)]
pub(crate) struct OrderStats24h {
    pub total_orders: i64,
    /// Sum of `total` across all 24h orders. Float because the column is
    /// `DOUBLE PRECISION`; admin rendering rounds to a baht integer.
    pub revenue: f64,
    pub unique_buyers: i64,
    /// `total_orders > 0` ? `revenue / total_orders` : 0. Pre-computed so
    /// the render side doesn't have to deal with div-by-zero.
    pub avg_order_value: f64,
    /// Currently `pending` orders — admin's "right now" backlog signal.
    /// Includes orders older than 24h.
    pub pending_total: i64,
}

/// One round-trip aggregate over `orders` for the last 24h, plus a tail
/// `pending_total` for the right-now view. Uses partial indexes already on
/// `created_at` and `status`; cheap even on large `orders` tables.
pub(crate) async fn order_stats_24h(
    orm: &sea_orm::DatabaseConnection,
) -> Result<OrderStats24h, sea_orm::DbErr> {
    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    let row = orm
        .query_one(Statement::from_string(
            DbBackend::Postgres,
            "SELECT \
                COUNT(*)::bigint                                    AS total_orders, \
                COALESCE(SUM(total::float8), 0)::float8             AS revenue, \
                COUNT(DISTINCT telegram_id) \
                    FILTER (WHERE telegram_id IS NOT NULL)::bigint  AS unique_buyers \
             FROM orders WHERE created_at > NOW() - INTERVAL '24 hours'"
                .to_string(),
        ))
        .await?
        .ok_or_else(|| sea_orm::DbErr::Custom("order_stats_24h: aggregate row missing".into()))?;
    let total_orders: i64 = crate::try_get_warn!(row, "total_orders", 0_i64);
    let revenue: f64 = crate::try_get_warn!(row, "revenue", 0.0);
    // Cycle #147 fix → cycle #148 helper. `SUM(total::float8)` could
    // in principle return NaN if a `total` row got NaN-poisoned, and
    // the `/engage` admin panel would then render "Revenue: NaN ฿".
    // `avg_order_value` was already guarded; `revenue` wasn't.
    let revenue = crate::trios::validation::clamp_finite_non_negative(revenue);
    let unique_buyers: i64 = crate::try_get_warn!(row, "unique_buyers", 0_i64);

    let avg_order_value = if total_orders > 0 && revenue.is_finite() {
        revenue / total_orders as f64
    } else {
        0.0
    };

    let pending_row = orm
        .query_one(Statement::from_string(
            DbBackend::Postgres,
            "SELECT COUNT(*)::bigint AS pending FROM orders WHERE status = 'pending'".to_string(),
        ))
        .await?
        .ok_or_else(|| sea_orm::DbErr::Custom("order_stats_24h: pending row missing".into()))?;
    let pending_total: i64 = pending_row.try_get("", "pending").unwrap_or(0);

    Ok(OrderStats24h {
        total_orders,
        revenue,
        unique_buyers,
        avg_order_value,
        pending_total,
    })
}

/// 24-hour aggregate for the `/engage` blocks panel (cycle #68).
/// Mirrors the shape of `OrderStats24h` / `FraudStats24h` so the bot
/// handler can stack all three panels symmetrically.
#[derive(Debug, Default, Clone)]
pub(crate) struct BlockStats24h {
    pub auto_blocks: i64,
    pub unblocks: i64,
    /// Admin who issued the most `/unblock` commands in the window, as
    /// a stringified telegram_id (matches FraudStats24h::top_offender).
    /// None when no manual unblocks happened.
    pub top_actor_admin: Option<String>,
    pub top_actor_count: i64,
}

/// One round-trip aggregate over `block_history` for the last 24h.
/// Uses `COUNT(*) FILTER (WHERE action = ...)` so the per-action counts
/// come from a single index scan over `(created_at)`.
pub(crate) async fn block_stats_24h(
    orm: &sea_orm::DatabaseConnection,
) -> Result<BlockStats24h, sea_orm::DbErr> {
    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    let row = orm
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT \
                COUNT(*) FILTER (WHERE action = $1)::bigint AS auto_blocks, \
                COUNT(*) FILTER (WHERE action = $2)::bigint AS unblocks \
             FROM block_history WHERE created_at > NOW() - INTERVAL '24 hours'",
            [BLOCK_ACTION_AUTO.into(), BLOCK_ACTION_UNBLOCK.into()],
        ))
        .await?
        .ok_or_else(|| sea_orm::DbErr::Custom("block_stats_24h: aggregate row missing".into()))?;
    let mut s = BlockStats24h {
        auto_blocks: row.try_get("", "auto_blocks").unwrap_or(0),
        unblocks: row.try_get("", "unblocks").unwrap_or(0),
        top_actor_admin: None,
        top_actor_count: 0,
    };
    // Top admin actor — scoped to manual `unblock` rows because
    // `auto_block` events have NULL `actor_admin_id` by design.
    if let Ok(Some(top)) = orm
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT actor_admin_id::text AS aid, COUNT(*)::bigint AS n \
             FROM block_history \
             WHERE created_at > NOW() - INTERVAL '24 hours' \
               AND action = $1 \
               AND actor_admin_id IS NOT NULL \
             GROUP BY actor_admin_id ORDER BY n DESC LIMIT 1",
            [BLOCK_ACTION_UNBLOCK.into()],
        ))
        .await
    {
        s.top_actor_admin = top.try_get::<Option<String>>("", "aid").ok().flatten();
        s.top_actor_count = top.try_get("", "n").unwrap_or(0);
    }
    Ok(s)
}

/// Pure helper: render the `/engage` blocks block as Telegram HTML.
/// Symmetric with `format_order_stats` / fraud rendering inside the
/// engage handler. Empty window → ✅ none so admin doesn't see a panel
/// full of zeros every quiet day.
pub(crate) fn format_block_stats(s: &BlockStats24h) -> String {
    use crate::util::html_escape;
    if s.auto_blocks == 0 && s.unblocks == 0 {
        return "<b>🚫 Blocks (24h)</b>\n✅ <i>none</i>".to_string();
    }
    let top = s
        .top_actor_admin
        .as_deref()
        .map(|t| {
            format!(
                "<code>{}</code> ({} unblocks)",
                html_escape(t),
                s.top_actor_count
            )
        })
        .unwrap_or_else(|| "—".to_string());
    format!(
        "<b>🚫 Blocks (24h)</b>\n\
         🤖 Auto-blocks: <b>{}</b>\n\
         🔓 Manual unblocks: <b>{}</b>\n\
         👮 Top admin: {}",
        s.auto_blocks, s.unblocks, top,
    )
}

/// Pure helper: render the `/engage` orders block as Telegram HTML.
/// Caller wraps in `parse_mode(Html)`. Extracted from the bot handler so
/// the layout is unit-testable.
pub(crate) fn format_order_stats(s: &OrderStats24h) -> String {
    format!(
        "<b>📦 Orders (24h)</b>\n\
         🛒 Total: <b>{}</b>\n\
         💰 Revenue: <b>{:.0} ฿</b>\n\
         👥 Unique buyers: <b>{}</b>\n\
         📊 Avg order: <b>{:.0} ฿</b>\n\
         ⏳ Pending right now: <b>{}</b>",
        s.total_orders, s.revenue, s.unique_buyers, s.avg_order_value, s.pending_total,
    )
}

/// 24-hour aggregate for the `/engage` fraud panel. Keep this struct narrow:
/// /engage's text rendering reads each field once.
#[derive(Debug, Default, Clone)]
pub(crate) struct FraudStats24h {
    pub subtotal_mismatch: i64,
    pub unknown_item: i64,
    pub unavailable: i64,
    pub malformed: i64,
    /// telegram_id with the most events in the window. None if no events
    /// had a telegram_id (anonymous-only). String to allow easy `format!`
    /// without an extra cast.
    pub top_offender: Option<String>,
    pub top_offender_count: i64,
}

/// One round-trip aggregate over `order_fraud_events` for the last 24h.
pub(crate) async fn fraud_stats_24h(
    orm: &sea_orm::DatabaseConnection,
) -> Result<FraudStats24h, sea_orm::DbErr> {
    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    // Per-code counts. COUNT(*) FILTER (...) keeps the whole thing in one
    // index scan over `idx_fraud_events_created_at`.
    let row = orm
        .query_one(Statement::from_string(
            DbBackend::Postgres,
            "SELECT \
                COUNT(*) FILTER (WHERE code = 'subtotal_mismatch')::bigint AS subtotal_mismatch, \
                COUNT(*) FILTER (WHERE code = 'unknown_item')::bigint        AS unknown_item, \
                COUNT(*) FILTER (WHERE code = 'unavailable')::bigint         AS unavailable, \
                COUNT(*) FILTER (WHERE code = 'malformed')::bigint           AS malformed \
             FROM order_fraud_events WHERE created_at > NOW() - INTERVAL '24 hours'"
                .to_string(),
        ))
        .await?
        .ok_or_else(|| sea_orm::DbErr::Custom("fraud_stats_24h: aggregate row missing".into()))?;
    let mut s = FraudStats24h {
        subtotal_mismatch: row.try_get("", "subtotal_mismatch").unwrap_or(0),
        unknown_item: row.try_get("", "unknown_item").unwrap_or(0),
        unavailable: row.try_get("", "unavailable").unwrap_or(0),
        malformed: row.try_get("", "malformed").unwrap_or(0),
        top_offender: None,
        top_offender_count: 0,
    };
    // Top offender — separate cheap query because it's bounded LIMIT 1.
    if let Ok(Some(top)) = orm
        .query_one(Statement::from_string(
            DbBackend::Postgres,
            "SELECT telegram_id::text AS tid, COUNT(*)::bigint AS n \
             FROM order_fraud_events \
             WHERE created_at > NOW() - INTERVAL '24 hours' \
               AND telegram_id IS NOT NULL \
             GROUP BY telegram_id ORDER BY n DESC LIMIT 1"
                .to_string(),
        ))
        .await
    {
        s.top_offender = top.try_get::<Option<String>>("", "tid").ok().flatten();
        s.top_offender_count = top.try_get("", "n").unwrap_or(0);
    }
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::{audit_sweep_sql, idempotency_sweep_sql, OrderItem};

    // ── audit_sweep_sql (cycle #67) ─────────────────────────────────────
    //
    // All three table-specific builders now delegate here. Pin the output
    // shape exactly so a "harmless cleanup" of the format! string can't
    // silently change what Postgres sees.

    #[test]
    fn audit_sweep_sql_produces_canonical_delete() {
        // Exact-equality (not `contains`) so byte-level format drift is
        // caught immediately.
        assert_eq!(
            audit_sweep_sql("any_table", "5 minutes"),
            "DELETE FROM any_table WHERE created_at < NOW() - INTERVAL '5 minutes'",
        );
    }

    #[test]
    fn audit_sweep_sql_interpolates_table_and_interval_in_order() {
        // Belt-and-braces: assert the table name appears BEFORE the
        // INTERVAL clause, so a `format!` arg swap (table↔interval) won't
        // ship a syntactically-valid but semantically-wrong query.
        let sql = audit_sweep_sql("foo", "1 days");
        let table_pos = sql.find("FROM foo").expect("table after FROM");
        let interval_pos = sql.find("INTERVAL").expect("INTERVAL clause");
        assert!(
            table_pos < interval_pos,
            "table must appear before INTERVAL: {}",
            sql
        );
    }

    #[test]
    fn audit_sweep_sql_holds_canonical_shape_across_combinations() {
        // Property-style test: for every (table, interval) drawn from a
        // matrix of realistic inputs, the output must satisfy four
        // invariants. Catches a "cleaned up the format!()" regression
        // that any single example test might miss because it pinned only
        // one specific combination.
        let tables = [
            "order_idempotency_keys",
            "order_fraud_events",
            "block_history",
            "audit_log", // hypothetical future table
            "t",         // shortest legal name
            "really_long_table_name_for_some_reason",
        ];
        let intervals = [
            "1 hours", "24 hours", "30 days", "90 days", "1 minute", "365 days",
        ];
        for &table in &tables {
            for &interval in &intervals {
                let sql = audit_sweep_sql(table, interval);
                // Invariant 1: starts with the canonical DELETE keyword.
                assert!(
                    sql.starts_with("DELETE FROM "),
                    "({}, {}) must start with DELETE FROM: {}",
                    table,
                    interval,
                    sql,
                );
                // Invariant 2: table name interpolated verbatim immediately
                // after FROM (catches table-name truncation / quoting bugs).
                assert!(
                    sql.contains(&format!("DELETE FROM {} WHERE", table)),
                    "({}, {}) must place table right after FROM: {}",
                    table,
                    interval,
                    sql,
                );
                // Invariant 3: interval literal interpolated verbatim inside
                // single quotes after INTERVAL.
                assert!(
                    sql.contains(&format!("INTERVAL '{}'", interval)),
                    "({}, {}) must interpolate interval in 'quotes': {}",
                    table,
                    interval,
                    sql,
                );
                // Invariant 4: standard time anchor — `NOW()` not CURRENT_TIMESTAMP
                // or anything else. Pin the wire-level choice.
                assert!(
                    sql.contains("created_at < NOW()"),
                    "({}, {}) must compare against NOW(): {}",
                    table,
                    interval,
                    sql,
                );
            }
        }
        // Coverage assertion: matrix size matches expectation so a
        // future shrink to e.g. one input doesn't silently weaken the test.
        assert_eq!(tables.len() * intervals.len(), 36);
    }

    #[test]
    fn idempotency_sweep_uses_correct_interval_literal() {
        let sql = idempotency_sweep_sql(24);
        assert!(sql.contains("DELETE FROM order_idempotency_keys"));
        assert!(sql.contains("INTERVAL '24 hours'"));
    }

    #[test]
    fn idempotency_sweep_accepts_arbitrary_retention() {
        // Cycle ships with 24h, but the builder must work for any value
        // (staging / tests may want a shorter window).
        let sql = idempotency_sweep_sql(1);
        assert!(sql.contains("INTERVAL '1 hours'"));
    }

    use super::fraud_events_sweep_sql;

    #[test]
    fn fraud_sweep_uses_correct_interval_literal() {
        let sql = fraud_events_sweep_sql(30);
        assert!(sql.contains("DELETE FROM order_fraud_events"));
        assert!(sql.contains("INTERVAL '30 days'"));
    }

    #[test]
    fn fraud_sweep_accepts_arbitrary_retention() {
        let sql = fraud_events_sweep_sql(7);
        assert!(sql.contains("INTERVAL '7 days'"));
    }

    use super::block_history_sweep_sql;

    #[test]
    fn block_history_sweep_uses_correct_interval_literal() {
        // Production ships with 90 — assert the literal lands intact so a
        // typo in the format!() arg can't reach prod.
        let sql = block_history_sweep_sql(90);
        assert!(sql.contains("DELETE FROM block_history"));
        assert!(sql.contains("INTERVAL '90 days'"));
    }

    #[test]
    fn block_history_sweep_accepts_arbitrary_retention() {
        // Different from production default, on purpose: catches "retention
        // hardcoded to 90 inside the builder" regressions.
        let sql = block_history_sweep_sql(14);
        assert!(sql.contains("INTERVAL '14 days'"));
    }

    // ── /engage orders panel (cycle #63 / B) ─────────────────────────────

    use super::{format_order_stats, OrderStats24h};

    #[test]
    fn order_stats_format_renders_all_fields() {
        let s = OrderStats24h {
            total_orders: 12,
            revenue: 6_900.0,
            unique_buyers: 7,
            avg_order_value: 575.0,
            pending_total: 2,
        };
        let out = format_order_stats(&s);
        // Each value lands in the rendered text exactly once.
        assert!(out.contains("12"));
        assert!(out.contains("6900"));
        assert!(out.contains("7"));
        assert!(out.contains("575"));
        assert!(out.contains("2"));
        // Headers stay so /engage layout is recognisable across cycles.
        assert!(out.contains("Orders (24h)"));
        assert!(out.contains("Revenue"));
        assert!(out.contains("Pending right now"));
    }

    // ── block_history action constants (cycle #64) ──────────────────────

    use super::{BLOCK_ACTION_AUTO, BLOCK_ACTION_UNBLOCK};

    #[test]
    fn block_action_constants_are_stable() {
        // These strings end up in `block_history.action` as a wire-level
        // enum. Renaming them silently would invalidate every existing row
        // — pin the values here so the contract breaks at compile/test
        // time instead.
        assert_eq!(BLOCK_ACTION_AUTO, "auto_block");
        assert_eq!(BLOCK_ACTION_UNBLOCK, "unblock");
    }

    #[test]
    fn order_stats_format_handles_empty_window() {
        // Cold start / quiet day: zero everything. Should still render
        // cleanly without `inf`/`NaN` (avg division-by-zero guard).
        let s = OrderStats24h::default();
        let out = format_order_stats(&s);
        assert!(!out.contains("inf"));
        assert!(!out.contains("NaN"));
        assert!(out.contains("0"));
    }

    // ── /engage blocks panel (cycle #68) ─────────────────────────────────

    use super::{format_block_stats, BlockStats24h};

    #[test]
    fn block_stats_format_renders_full_panel() {
        let s = BlockStats24h {
            auto_blocks: 3,
            unblocks: 1,
            top_actor_admin: Some("8420420131".into()),
            top_actor_count: 1,
        };
        let out = format_block_stats(&s);
        assert!(out.contains("Blocks (24h)"));
        assert!(out.contains("Auto-blocks") && out.contains("3"));
        assert!(out.contains("Manual unblocks") && out.contains("1"));
        // Admin actor id appears verbatim so admin can spot themselves.
        assert!(out.contains("8420420131"));
        // No `none` short-circuit on a populated window.
        assert!(!out.contains("none"));
    }

    #[test]
    fn block_stats_format_empty_window_says_none() {
        // Quiet day: zero auto-blocks AND zero unblocks → ✅ none. Symmetric
        // with the fraud panel — admin doesn't see a wall of zeros every
        // morning.
        let out = format_block_stats(&BlockStats24h::default());
        assert!(out.contains("Blocks (24h)"));
        assert!(out.contains("none"));
        // Make sure we're not silently leaking 0-counts behind the badge.
        assert!(!out.contains("Auto-blocks"));
    }

    #[test]
    fn block_stats_format_handles_missing_top_actor() {
        // 5 auto-blocks but 0 manual unblocks (no admin actor).
        // Panel must still render — auto_blocks count alone is meaningful.
        let s = BlockStats24h {
            auto_blocks: 5,
            unblocks: 0,
            top_actor_admin: None,
            top_actor_count: 0,
        };
        let out = format_block_stats(&s);
        assert!(out.contains("Blocks (24h)"));
        assert!(out.contains("5"));
        // Em-dash placeholder for missing admin keeps the line balanced.
        assert!(out.contains("—"));
    }

    // ── Auto-block threshold (cycle #60) ─────────────────────────────────

    use super::{should_auto_block_for_fraud, FRAUD_AUTO_BLOCK_THRESHOLD};

    #[test]
    fn auto_block_engages_exactly_at_threshold() {
        // Boundary: at-threshold MUST trigger a block. The /engage panel's
        // "Top offender: 3 events" line then matches the block decision.
        assert!(should_auto_block_for_fraud(FRAUD_AUTO_BLOCK_THRESHOLD));
    }

    #[test]
    fn auto_block_skips_just_below_threshold() {
        // Off-by-one guard for `>=` vs `>`. 2 events should NOT block — that
        // window still allows admin to investigate before user is locked out.
        assert!(!should_auto_block_for_fraud(FRAUD_AUTO_BLOCK_THRESHOLD - 1));
    }

    #[test]
    fn auto_block_engages_well_above_threshold() {
        assert!(should_auto_block_for_fraud(50));
    }

    #[test]
    fn auto_block_skips_zero_and_negative() {
        // Defensive: a corrupted COUNT() could theoretically come back 0 or
        // even negative (i64 overflow / type confusion). Neither must block.
        assert!(!should_auto_block_for_fraud(0));
        assert!(!should_auto_block_for_fraud(-1));
    }

    // ── /unblock parser (cycle #61) ──────────────────────────────────────

    use super::parse_unblock_arg;

    #[test]
    fn unblock_parser_accepts_plain_int() {
        assert_eq!(parse_unblock_arg("1234567890"), Some(1234567890));
    }

    #[test]
    fn unblock_parser_trims_whitespace() {
        // Telegram's command parser keeps leading/trailing whitespace on
        // String args — the parser must canonicalize.
        assert_eq!(parse_unblock_arg("  42  "), Some(42));
    }

    #[test]
    fn unblock_parser_rejects_empty() {
        assert_eq!(parse_unblock_arg(""), None);
        assert_eq!(parse_unblock_arg("   "), None);
    }

    #[test]
    fn unblock_parser_rejects_non_numeric() {
        assert_eq!(parse_unblock_arg("abc"), None);
        assert_eq!(parse_unblock_arg("12abc"), None);
        assert_eq!(parse_unblock_arg("1.5"), None);
    }

    // ── /blocks formatter (cycle #62) ────────────────────────────────────

    use super::{format_blocks_message, BlockedUserRow};

    #[test]
    fn blocks_format_empty_says_none() {
        let s = format_blocks_message(&[], 0);
        assert!(s.contains("Blocked users"));
        assert!(s.contains("none"));
        // Empty list shouldn't show the /unblock hint — there's nothing to act on.
        assert!(!s.contains("/unblock"));
    }

    #[test]
    fn blocks_format_single_user_shows_id_and_code() {
        let rows = vec![BlockedUserRow {
            telegram_id: 12345,
            last_fraud_at: None,
            last_fraud_code: Some("subtotal_mismatch".into()),
        }];
        let s = format_blocks_message(&rows, 1);
        assert!(s.contains("12345"));
        assert!(s.contains("subtotal_mismatch"));
        // Hint at the unblock command — admins forget the syntax.
        assert!(s.contains("/unblock"));
    }

    #[test]
    fn blocks_format_escapes_html_in_code() {
        // Defensive: `code` is server-written today but the format must be
        // safe if a future cycle lets it carry free-text reasons.
        let rows = vec![BlockedUserRow {
            telegram_id: 1,
            last_fraud_at: None,
            last_fraud_code: Some("<script>alert(1)</script>".into()),
        }];
        let s = format_blocks_message(&rows, 1);
        assert!(!s.contains("<script>"));
        assert!(s.contains("&lt;script&gt;"));
    }

    #[test]
    fn blocks_format_shows_truncation_hint_when_more_exist() {
        let rows = vec![BlockedUserRow {
            telegram_id: 1,
            last_fraud_at: None,
            last_fraud_code: None,
        }];
        let s = format_blocks_message(&rows, 75);
        // 75 total, 1 shown → 74 more
        assert!(s.contains("74"));
        assert!(s.contains("more"));
    }

    #[test]
    fn blocks_format_omits_truncation_when_count_matches() {
        let rows = vec![BlockedUserRow {
            telegram_id: 1,
            last_fraud_at: None,
            last_fraud_code: None,
        }];
        let s = format_blocks_message(&rows, 1);
        // When shown == total, no "more" hint.
        assert!(!s.contains("more"));
    }

    #[test]
    fn unblock_parser_rejects_zero_and_negative() {
        // Negative explicitly rejected so a future SQL rewrite like
        // `WHERE telegram_id > $1` can't unblock the whole base by accident
        // when admin types `/unblock -1`.
        assert_eq!(parse_unblock_arg("0"), None);
        assert_eq!(parse_unblock_arg("-42"), None);
    }

    #[test]
    fn test_order_item_serde_roundtrip() {
        let item = OrderItem {
            strain_id: Some("s1".into()),
            strain_name: Some("Indica".into()),
            accessory_id: None,
            accessory_name: None,
            tea_id: None,
            tea_name: None,
            set_id: None,
            set_name: None,
            quantity: 2.5,
            is_set: Some(false),
            is_accessory: None,
            is_tea: None,
            is_tea_set: None,
            fulfillment: None,
        };
        let json = serde_json::to_value(&item).unwrap();
        let back: OrderItem = serde_json::from_value(json).unwrap();
        assert_eq!(back.strain_id, Some("s1".into()));
        assert_eq!(back.quantity, 2.5);
    }

    #[test]
    fn test_order_item_defaults() {
        let item = OrderItem {
            strain_id: None,
            strain_name: None,
            accessory_id: None,
            accessory_name: None,
            tea_id: None,
            tea_name: None,
            set_id: None,
            set_name: None,
            quantity: 1.0,
            is_set: None,
            is_accessory: None,
            is_tea: None,
            is_tea_set: None,
            fulfillment: None,
        };
        let json = serde_json::to_value(&item).unwrap();
        assert!(json.get("strain_id").is_some());
    }
}
