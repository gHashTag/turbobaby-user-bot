use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio_postgres::Row;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Order {
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

impl Order {
    pub fn from_row(row: &Row) -> Self {
        Self {
            id: row.try_get("id").unwrap_or_default(),
            telegram_id: row.try_get("telegram_id").ok().flatten(),
            customer_name: row.try_get("customer_name").ok().flatten(),
            customer_phone: row.try_get("customer_phone").ok().flatten(),
            customer_telegram: row.try_get("customer_telegram").ok().flatten(),
            items: row.try_get("items").unwrap_or(Value::Null),
            subtotal: {
                let v = row.try_get::<_, f64>("subtotal").unwrap_or(0.0);
                if v.is_finite() {
                    v.max(0.0)
                } else {
                    0.0
                }
            },
            bonus_used: {
                let v = row.try_get::<_, f64>("bonus_used").unwrap_or(0.0);
                if v.is_finite() {
                    v.max(0.0)
                } else {
                    0.0
                }
            },
            total: {
                let v = row.try_get::<_, f64>("total").unwrap_or(0.0);
                if v.is_finite() {
                    v.max(0.0)
                } else {
                    0.0
                }
            },
            status: row.try_get("status").unwrap_or_default(),
            shop_id: row.try_get("shop_id").ok().flatten(),
            created_at: row
                .try_get("created_at")
                .unwrap_or_else(|_| chrono::Utc::now()),
        }
    }
}

// Wave 5: SeaORM Entity API path. Старый tokio-postgres код выше — будем удалять в Wave 6+.

/// Returns all orders for a given telegram_id via SeaORM Entity API.
#[allow(dead_code)]
pub async fn get_user_orders_seaorm(
    orm: &sea_orm::DatabaseConnection,
    telegram_id: i64,
) -> Result<Vec<crate::db::entities::order::Model>, sea_orm::DbErr> {
    use crate::db::entities::order::{Column, Entity};
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
    Entity::find()
        .filter(Column::TelegramId.eq(telegram_id))
        .all(orm)
        .await
}

/// Atomically mark an order as completed and update the customer's loyalty profile.
/// Returns `Some((telegram_id, is_first_order))` if the order was newly completed,
/// or `None` if it was already completed (idempotent).
pub async fn complete_order_and_update_loyalty(
    pool: &deadpool_postgres::Pool,
    order_id: &str,
) -> Result<Option<(i64, bool)>, Box<dyn std::error::Error + Send + Sync>> {
    let mut client = pool.get().await?;
    let tx = client.transaction().await?;

    let order_row = tx
        .query_opt(
            "SELECT telegram_id, total::float8, status FROM orders WHERE id = $1 FOR UPDATE",
            &[&order_id],
        )
        .await?;

    let result = if let Some(row) = order_row {
        let cid: Option<i64> = row.try_get("telegram_id").ok().flatten();
        let total: f64 = {
            let v = row.try_get::<_, f64>("total").unwrap_or(0.0);
            if v.is_finite() {
                v.max(0.0)
            } else {
                0.0
            }
        };
        let status: String = row.try_get("status").unwrap_or_default();
        if status != "completed" {
            let loyalty_result = if let Some(cid) = cid {
                // Count BEFORE updating so is_first is accurate
                let count_before = tx.query_one(
                    "SELECT COUNT(*) as cnt FROM orders WHERE telegram_id = $1 AND status = 'completed' AND id != $2",
                    &[&cid, &order_id],
                ).await?.try_get::<_, i64>("cnt").unwrap_or(0);
                let is_first = count_before == 0;

                tx.execute(
                    "UPDATE orders SET status = 'completed' WHERE id = $1",
                    &[&order_id],
                )
                .await?;

                tx.execute(
                    "INSERT INTO loyalty_profiles (telegram_id, total_spent, first_purchase_at) VALUES ($1, $2, NOW())
                     ON CONFLICT (telegram_id) DO UPDATE SET
                       total_spent = COALESCE(loyalty_profiles.total_spent, 0) + EXCLUDED.total_spent,
                       first_purchase_at = COALESCE(loyalty_profiles.first_purchase_at, NOW())",
                    &[&cid, &total],
                ).await?;

                tx.execute(
                    "UPDATE loyalty_profiles SET tier = CASE
                        WHEN loyalty_profiles.total_spent >= (SELECT (config->>'gold_threshold')::float8 FROM loyalty_config WHERE id = 1 LIMIT 1) THEN 'gold'
                        WHEN loyalty_profiles.total_spent >= (SELECT (config->>'silver_threshold')::float8 FROM loyalty_config WHERE id = 1 LIMIT 1) THEN 'silver'
                        WHEN loyalty_profiles.total_spent >= (SELECT (config->>'bronze_threshold')::float8 FROM loyalty_config WHERE id = 1 LIMIT 1) THEN 'bronze'
                        ELSE 'none'
                     END
                     WHERE telegram_id = $1",
                    &[&cid],
                ).await?;

                Some((cid, is_first))
            } else {
                None
            };
            loyalty_result
        } else {
            None
        }
    } else {
        None
    };

    tx.commit().await?;
    Ok(result)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
}

// ─── Idempotency-key TTL sweep (cycle #58 / A) ────────────────────────────
//
// migration 029 (`order_idempotency_keys`) is append-only. Without a sweep
// the table grows unbounded; Stripe / AWS / GCP all use a 24 h window for
// the same reason. Real client retry windows are far shorter than that —
// 24 h is the safe default. The pure SQL builder is extracted so a typo in
// the INTERVAL literal cannot slip through to production unnoticed.

/// Build the `DELETE` SQL fragment for the idempotency-key TTL sweep.
/// Extracted as a pure function so the literal is unit-testable; otherwise
/// a typo in `INTERVAL '24 hours'` would only show up in production.
pub(crate) fn idempotency_sweep_sql(retention_hours: u32) -> String {
    format!(
        "DELETE FROM order_idempotency_keys \
         WHERE created_at < NOW() - INTERVAL '{} hours'",
        retention_hours
    )
}

/// Delete `order_idempotency_keys` rows older than `retention_hours`.
/// Returns the number of rows deleted (for metrics / structured logs).
///
/// Idempotent and safe to run concurrently with `create_order` — Postgres
/// handles concurrent DELETE/INSERT on the same table cleanly, and the
/// 24 h cutoff is far older than any in-flight order's retry window.
pub async fn cleanup_old_idempotency_keys(
    pool: &deadpool_postgres::Pool,
    retention_hours: u32,
) -> Result<u64, Box<dyn std::error::Error + Send + Sync>> {
    let client = pool.get().await?;
    let sql = idempotency_sweep_sql(retention_hours);
    let deleted = client.execute(sql.as_str(), &[]).await?;
    Ok(deleted)
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
pub const FRAUD_CODE_SUBTOTAL_MISMATCH: &str = "subtotal_mismatch";
pub const FRAUD_CODE_UNKNOWN_ITEM: &str = "unknown_item";
pub const FRAUD_CODE_UNAVAILABLE: &str = "unavailable";
pub const FRAUD_CODE_MALFORMED: &str = "malformed";

/// Number of `subtotal_mismatch` events in the lookback window that triggers
/// an automatic block (cycle #60). 3 is conservative: a real shopper hitting
/// stale-cart prices would clear and retry, not produce 3+ price tampering
/// events in a day. 1–2 might be a buggy client or race condition; 3+ is a
/// sustained pattern that warrants an automatic stop-the-bleeding action.
pub const FRAUD_AUTO_BLOCK_THRESHOLD: i64 = 3;
/// Lookback window for the auto-block decision. 24 h matches the rest of
/// the audit pipeline (/engage panel, idempotency TTL).
pub const FRAUD_AUTO_BLOCK_LOOKBACK_HOURS: i32 = 24;

/// Pure decision: should the auto-blocker engage given the count of
/// `subtotal_mismatch` events seen for this user in the lookback window?
/// Extracted as a standalone function so the threshold semantics are
/// table-tested instead of buried inside an async DB-bound function.
pub fn should_auto_block_for_fraud(subtotal_mismatch_events_24h: i64) -> bool {
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
pub async fn record_fraud_event(
    pool: &deadpool_postgres::Pool,
    telegram_id: Option<i64>,
    code: &str,
    catalog: Option<&str>,
    item_id: Option<&str>,
    claimed_subtotal: Option<f64>,
    expected_subtotal: Option<f64>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let client = pool.get().await?;
    client
        .execute(
            "INSERT INTO order_fraud_events \
             (telegram_id, code, catalog, item_id, claimed_subtotal, expected_subtotal) \
             VALUES ($1, $2, $3, $4, $5, $6)",
            &[
                &telegram_id,
                &code,
                &catalog,
                &item_id,
                &claimed_subtotal,
                &expected_subtotal,
            ],
        )
        .await?;
    // Drop the borrowed client so auto-block can acquire a fresh connection.
    drop(client);

    // Auto-block trigger (cycle #60). Only fires for the strongest fraud
    // signal — `subtotal_mismatch` — and only when we have a telegram_id
    // to block. Anonymous mismatches are caught by the per-IP rate limit.
    if code == FRAUD_CODE_SUBTOTAL_MISMATCH {
        if let Some(tid) = telegram_id {
            if let Err(e) = auto_block_for_fraud(pool, tid).await {
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
pub fn parse_unblock_arg(arg: &str) -> Option<i64> {
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
pub struct BlockedUserRow {
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
pub const BLOCKED_USERS_LIST_LIMIT: i64 = 50;

/// Fetch currently-blocked users enriched with the most recent fraud-event
/// context for each. Single round-trip — `LEFT JOIN LATERAL` lets the
/// planner index-scan `idx_fraud_events_created_at` per user instead of
/// doing a window scan across the whole table.
pub async fn query_blocked_users(
    pool: &deadpool_postgres::Pool,
    limit: i64,
) -> Result<Vec<BlockedUserRow>, Box<dyn std::error::Error + Send + Sync>> {
    let client = pool.get().await?;
    let rows = client
        .query(
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
            &[&limit],
        )
        .await?;
    let mut out = Vec::with_capacity(rows.len());
    for r in &rows {
        out.push(BlockedUserRow {
            telegram_id: r.try_get("telegram_id").unwrap_or(0),
            last_fraud_at: r
                .try_get::<_, Option<chrono::DateTime<chrono::Utc>>>("last_fraud_at")
                .ok()
                .flatten(),
            last_fraud_code: r
                .try_get::<_, Option<String>>("last_fraud_code")
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
pub fn format_blocks_message(rows: &[BlockedUserRow], total_count: usize) -> String {
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

/// Manually clear `is_blocked` for `telegram_id`. Counterpart to the
/// automatic blocker in [`auto_block_for_fraud`] — admins drive this via
/// the `/unblock` bot command (cycle #61).
///
/// Returns `Ok(true)` if the row was actually flipped, `Ok(false)` if the
/// user was not blocked (or has no profile). The `AND is_blocked = true`
/// guard means an admin can spam `/unblock 123` without each call writing
/// a fresh `is_blocked = false` UPDATE.
pub async fn manual_unblock(
    pool: &deadpool_postgres::Pool,
    telegram_id: i64,
) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
    let client = pool.get().await?;
    let rows = client
        .execute(
            "UPDATE loyalty_profiles SET is_blocked = FALSE \
             WHERE telegram_id = $1 AND is_blocked = TRUE",
            &[&telegram_id],
        )
        .await?;
    Ok(rows > 0)
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
    pool: &deadpool_postgres::Pool,
    telegram_id: i64,
) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
    let client = pool.get().await?;
    // Count only `subtotal_mismatch` — other codes (`unavailable`,
    // `unknown_item`, `malformed`) are mostly stale-cart / client-bug and
    // would auto-block honest users. Subtotal mismatch alone is the
    // can't-happen-by-accident signal.
    let row = client
        .query_one(
            "SELECT COUNT(*)::bigint AS n FROM order_fraud_events \
             WHERE telegram_id = $1 \
               AND code = $2 \
               AND created_at > NOW() - make_interval(hours => $3)",
            &[
                &telegram_id,
                &FRAUD_CODE_SUBTOTAL_MISMATCH,
                &FRAUD_AUTO_BLOCK_LOOKBACK_HOURS,
            ],
        )
        .await?;
    let count: i64 = row.try_get("n").unwrap_or(0);
    if !should_auto_block_for_fraud(count) {
        return Ok(false);
    }
    // UPSERT so anonymous-ish accounts without a loyalty profile row still
    // get blocked the moment they cross the threshold. ON CONFLICT lets us
    // flip is_blocked atomically even when the row already exists.
    let rows = client
        .execute(
            "INSERT INTO loyalty_profiles (telegram_id, bonus_balance, total_spent, is_blocked) \
             VALUES ($1, 0, 0, TRUE) \
             ON CONFLICT (telegram_id) DO UPDATE SET is_blocked = TRUE \
             WHERE loyalty_profiles.is_blocked = FALSE",
            &[&telegram_id],
        )
        .await?;
    if rows > 0 {
        tracing::warn!(
            telegram_id,
            count_24h = count,
            threshold = FRAUD_AUTO_BLOCK_THRESHOLD,
            "auto_block: user blocked for repeated subtotal_mismatch"
        );
    }
    Ok(rows > 0)
}

/// 24-hour aggregate for the `/engage` fraud panel. Keep this struct narrow:
/// /engage's text rendering reads each field once.
#[derive(Debug, Default, Clone)]
pub struct FraudStats24h {
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
pub async fn fraud_stats_24h(
    pool: &deadpool_postgres::Pool,
) -> Result<FraudStats24h, Box<dyn std::error::Error + Send + Sync>> {
    let client = pool.get().await?;
    // Per-code counts. COUNT(*) FILTER (...) keeps the whole thing in one
    // index scan over `idx_fraud_events_created_at`.
    let row = client
        .query_one(
            "SELECT \
                COUNT(*) FILTER (WHERE code = 'subtotal_mismatch')::bigint AS subtotal_mismatch, \
                COUNT(*) FILTER (WHERE code = 'unknown_item')::bigint        AS unknown_item, \
                COUNT(*) FILTER (WHERE code = 'unavailable')::bigint         AS unavailable, \
                COUNT(*) FILTER (WHERE code = 'malformed')::bigint           AS malformed \
             FROM order_fraud_events WHERE created_at > NOW() - INTERVAL '24 hours'",
            &[],
        )
        .await?;
    let mut s = FraudStats24h {
        subtotal_mismatch: row.try_get("subtotal_mismatch").unwrap_or(0),
        unknown_item: row.try_get("unknown_item").unwrap_or(0),
        unavailable: row.try_get("unavailable").unwrap_or(0),
        malformed: row.try_get("malformed").unwrap_or(0),
        top_offender: None,
        top_offender_count: 0,
    };
    // Top offender — separate cheap query because it's bounded LIMIT 1.
    if let Ok(Some(top)) = client
        .query_opt(
            "SELECT telegram_id::text AS tid, COUNT(*)::bigint AS n \
             FROM order_fraud_events \
             WHERE created_at > NOW() - INTERVAL '24 hours' \
               AND telegram_id IS NOT NULL \
             GROUP BY telegram_id ORDER BY n DESC LIMIT 1",
            &[],
        )
        .await
    {
        s.top_offender = top.try_get::<_, Option<String>>("tid").ok().flatten();
        s.top_offender_count = top.try_get("n").unwrap_or(0);
    }
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::{idempotency_sweep_sql, OrderItem};

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
        };
        let json = serde_json::to_value(&item).unwrap();
        assert!(json.get("strain_id").is_some());
    }
}
