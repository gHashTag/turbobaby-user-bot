use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ──────────────────────────────────────────────────────────────────
// Domain types
// ──────────────────────────────────────────────────────────────────

// Cycle #99: removed unused `ReferralEvent` struct. Cycle #84's migration
// generated `entities/referral_event.rs` (SeaORM Model) which became the
// canonical shape — the wire struct here had zero callers since then,
// just an `#[allow(dead_code)]` annotation hiding the rot.

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ReferrerStats {
    pub total_invited: i64,
    pub confirmed: i64,
    pub pending: i64,
    pub total_bonus_earned: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct TopReferrer {
    pub telegram_id: i64,
    pub first_name: Option<String>,
    pub referral_count: i64,
    pub total_bonus_earned: f64,
}

// ──────────────────────────────────────────────────────────────────
// Code generation
// ──────────────────────────────────────────────────────────────────

/// Characters used for opaque referral codes. Visually ambiguous glyphs
/// (0/O, 1/I/l) are excluded to reduce support tickets.
const REFERRAL_CODE_ALPHABET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
const REFERRAL_CODE_LEN: usize = 8;

/// Generate an opaque, random referral code. Not deterministic — the
/// caller must check for collisions against the database before persisting.
fn generate_referral_code() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    (0..REFERRAL_CODE_LEN)
        .map(|_| {
            let idx = rng.gen_range(0..REFERRAL_CODE_ALPHABET.len());
            REFERRAL_CODE_ALPHABET[idx] as char
        })
        .collect()
}

/// Maximum collision retries when generating a referral code. The alphabet
/// gives ~2.8e12 possible 8-char codes, so collisions are astronomically
/// unlikely until the user base grows enormous; the loop is defense in depth.
const MAX_CODE_RETRIES: usize = 10;

/// Get existing referral code for `telegram_id`, or create and persist a
/// new opaque code. Collisions are retried up to `MAX_CODE_RETRIES` times.
pub(crate) async fn get_or_create_referral_code(
    orm: &sea_orm::DatabaseConnection,
    telegram_id: i64,
) -> Result<String> {
    use crate::db::entities::loyalty_profile::{
        ActiveModel as LpAm, Column as LpCol, Entity as LoyaltyProfileEntity,
    };
    use sea_orm::sea_query::OnConflict;
    use sea_orm::{ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter};

    // Check existing code first.
    if let Some(m) = LoyaltyProfileEntity::find_by_id(telegram_id)
        .filter(LpCol::ReferralCode.is_not_null())
        .one(orm)
        .await
        .context("read existing referral_code")?
    {
        if let Some(existing) = m.referral_code {
            return Ok(existing);
        }
    }

    // Generate an unused opaque code. Retry if we happen to collide.
    let mut code = generate_referral_code();
    for attempt in 0..MAX_CODE_RETRIES {
        let collision = LoyaltyProfileEntity::find()
            .filter(LpCol::ReferralCode.eq(&code))
            .one(orm)
            .await
            .context("check referral_code collision")?;
        if collision.is_none() {
            break;
        }
        if attempt == MAX_CODE_RETRIES - 1 {
            anyhow::bail!(
                "failed to generate unique referral code for telegram_id={} after {} attempts",
                telegram_id,
                MAX_CODE_RETRIES
            );
        }
        code = generate_referral_code();
    }

    // Upsert: seed row + write code in one path.
    let seed = LpAm {
        telegram_id: Set(telegram_id),
        referral_code: Set(Some(code.clone())),
        ..Default::default()
    };
    LoyaltyProfileEntity::insert(seed)
        .on_conflict(
            OnConflict::column(LpCol::TelegramId)
                .update_columns([LpCol::ReferralCode])
                .to_owned(),
        )
        .exec(orm)
        .await
        .context("upsert loyalty_profile with referral_code")?;

    Ok(code)
}

// ──────────────────────────────────────────────────────────────────
// Referral lifecycle
// ──────────────────────────────────────────────────────────────────

/// Find the telegram_id of the owner of `code` (looks in loyalty_profiles.referral_code).
///
/// Cycle #84: SeaORM. Single read, no schema changes.
pub(crate) async fn find_referrer_by_code(
    orm: &sea_orm::DatabaseConnection,
    code: &str,
) -> Result<Option<i64>> {
    use crate::db::entities::loyalty_profile::{Column as LpCol, Entity as LoyaltyProfileEntity};
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
    let m = LoyaltyProfileEntity::find()
        .filter(LpCol::ReferralCode.eq(code))
        .one(orm)
        .await
        .context("find_referrer_by_code")?;
    Ok(m.map(|m| m.telegram_id))
}

/// A user can never be their own referrer. Pure predicate so the
/// data-layer invariant is unit-testable without a DB connection.
pub(crate) fn is_self_referral(referrer_id: i64, referred_id: i64) -> bool {
    referrer_id == referred_id
}

/// Record a new pending referral event.
/// Returns the new event UUID.
///
/// Cycle #84: SeaORM. Two statements — not wrapped in a tx because the
/// raw SQL wasn't either. The `COALESCE(existing, new)` semantics on the
/// `referred_by` upsert preserves a pre-existing referrer attribution
/// (first-touch wins); we express that via raw `Expr::cust_with_values`
/// since SeaORM's `OnConflict::update_columns` would overwrite.
pub(crate) async fn record_referral(
    orm: &sea_orm::DatabaseConnection,
    referrer_id: i64,
    referred_id: i64,
    code: &str,
    source: Option<&str>,
) -> Result<Uuid> {
    // Defense-in-depth: reject self-referral at the data layer. The only
    // current caller (bot `/start ref_…` in commands.rs) already guards
    // `referrer_id != user_id`, but the invariant belongs here too —
    // otherwise a future caller (admin tool, new endpoint) could create a
    // self-referral pending event that `confirm_referral` would later pay
    // out as a bonus to the user's own balance (financial fraud). Mirrors
    // `confirm_referral`'s own `bail!` input guards below.
    if is_self_referral(referrer_id, referred_id) {
        anyhow::bail!("self-referral rejected: referrer_id == referred_id ({referrer_id})");
    }
    use crate::db::entities::{
        loyalty_profile::{ActiveModel as LpAm, Column as LpCol, Entity as LoyaltyProfileEntity},
        referral_event::{
            ActiveModel as RefEventAm, Column as RefEventCol, Entity as RefEventEntity,
        },
    };
    use sea_orm::sea_query::OnConflict;
    use sea_orm::{ActiveValue::Set, EntityTrait};
    let id = Uuid::new_v4();

    // Ensure referred user has a loyalty_profile row with a referrer
    // attribution. First-touch wins via COALESCE.
    let lp_am = LpAm {
        telegram_id: Set(referred_id),
        referred_by: Set(Some(referrer_id)),
        ..Default::default()
    };
    LoyaltyProfileEntity::insert(lp_am)
        .on_conflict(
            OnConflict::column(LpCol::TelegramId)
                .value(
                    LpCol::ReferredBy,
                    sea_orm::sea_query::Expr::cust_with_values(
                        "COALESCE(loyalty_profiles.referred_by, $1)",
                        [referrer_id],
                    ),
                )
                .to_owned(),
        )
        .exec(orm)
        .await
        .context("upsert referred loyalty_profile")?;

    // Insert event — ignore if the referred_id already has one
    // (UNIQUE constraint on referred_id).
    let ev_am = RefEventAm {
        id: Set(id),
        referrer_id: Set(referrer_id),
        referred_id: Set(referred_id),
        code: Set(code.to_string()),
        status: Set("pending".to_string()),
        source: Set(source.map(|s| s.to_string())),
        ..Default::default()
    };
    RefEventEntity::insert(ev_am)
        .on_conflict(
            OnConflict::column(RefEventCol::ReferredId)
                .do_nothing()
                .to_owned(),
        )
        .do_nothing()
        .exec(orm)
        .await
        .context("insert referral_event")?;

    Ok(id)
}

/// Confirm a referral (first purchase of `referred_id`).
/// - Sets status = 'confirmed' + confirmed_at
/// - Credits bonus to referrer's balance via bonus_transactions
/// - Increments referrer's referral_count
///
/// Cycle #84: migrated to SeaORM transaction. The `SELECT ... FOR UPDATE`
/// race guard is preserved via `QuerySelect::lock_exclusive()`. All five
/// statements run inside `db.begin().await?` and commit atomically; early
/// `Err` returns auto-rollback via `DatabaseTransaction::drop`.
pub(crate) async fn confirm_referral(
    orm: &sea_orm::DatabaseConnection,
    referred_id: i64,
    bonus: f64,
) -> Result<()> {
    use crate::db::entities::{
        bonus_transaction::{ActiveModel as BonusTxAm, Entity as BonusTxEntity},
        loyalty_profile::{ActiveModel as LpAm, Column as LpCol, Entity as LoyaltyProfileEntity},
        referral_event::{
            ActiveModel as RefEventAm, Column as RefEventCol, Entity as RefEventEntity,
        },
    };
    use sea_orm::sea_query::OnConflict;
    use sea_orm::{
        ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter, QuerySelect, TransactionTrait,
    };

    if !bonus.is_finite() || bonus < 0.0 {
        anyhow::bail!("invalid bonus: {}", bonus);
    }

    let tx = orm.begin().await.context("start SeaORM tx")?;

    // 1. Find the pending event with row-level lock (FOR UPDATE) to
    //    prevent double-credit races. `QuerySelect::lock_exclusive` is
    //    SeaORM's equivalent of the raw `SELECT ... FOR UPDATE`.
    let pending = RefEventEntity::find()
        .filter(RefEventCol::ReferredId.eq(referred_id))
        .filter(RefEventCol::Status.eq("pending"))
        .lock_exclusive()
        .one(&tx)
        .await
        .context("query pending referral_event")?;

    let pending = match pending {
        Some(m) => m,
        None => {
            // No pending event — semantically a no-op. Commit the empty
            // tx (releases the lock if FOR UPDATE held anything); log on
            // failure as cycle #76 began doing.
            if let Err(e) = tx.commit().await {
                tracing::warn!(
                    "referrals.confirm_referral: empty-tx commit failed for referred_id={}: {}",
                    referred_id,
                    e
                );
            }
            return Ok(());
        }
    };
    let event_id = pending.id;
    let referrer_id = pending.referrer_id;

    // 2. Mark event confirmed.
    let mut event_am: RefEventAm = pending.into();
    event_am.status = Set("confirmed".to_string());
    event_am.confirmed_at = Set(Some(chrono::Utc::now().into()));
    event_am.bonus_paid = Set(bonus);
    RefEventEntity::update_many()
        .set(event_am)
        .filter(RefEventCol::Id.eq(event_id))
        .exec(&tx)
        .await
        .context("update referral_event status")?;

    // 3. Ensure referrer loyalty_profile row exists (idempotent upsert).
    let lp_seed = LpAm {
        telegram_id: Set(referrer_id),
        bonus_balance: Set(Some(0.0)),
        total_spent: Set(Some(0.0)),
        ..Default::default()
    };
    LoyaltyProfileEntity::insert(lp_seed)
        .on_conflict(
            OnConflict::column(LpCol::TelegramId)
                .do_nothing()
                .to_owned(),
        )
        .do_nothing()
        .exec(&tx)
        .await
        .context("upsert referrer loyalty_profile")?;

    // 4. Append bonus_transactions ledger row.
    let tx_id = Uuid::new_v4().to_string();
    let bt_am = BonusTxAm {
        id: Set(tx_id),
        telegram_id: Set(referrer_id),
        amount: Set(bonus),
        tx_type: Set("referral_bonus".to_string()),
        description: Set(Some("Referral bonus for new user".to_string())),
        related_order_id: Set(None),
        ..Default::default()
    };
    BonusTxEntity::insert(bt_am)
        .exec(&tx)
        .await
        .context("insert referral bonus_transaction")?;

    // 5. Credit balance + increment referral_count. We do them as a
    //    single update_many so the WHERE-clause check fires once.
    let updated = LoyaltyProfileEntity::update_many()
        .col_expr(
            LpCol::BonusBalance,
            sea_orm::sea_query::Expr::cust_with_values("bonus_balance + $1", [bonus]),
        )
        .col_expr(
            LpCol::ReferralCount,
            sea_orm::sea_query::Expr::cust("referral_count + 1"),
        )
        .filter(LpCol::TelegramId.eq(referrer_id))
        .exec(&tx)
        .await
        .context("credit referrer bonus + count")?;
    if updated.rows_affected == 0 {
        // tx drops → auto-rollback.
        anyhow::bail!(
            "confirm_referral: loyalty profile missing for referrer_id={} after upsert (race?)",
            referrer_id
        );
    }

    tx.commit().await.context("commit SeaORM referral tx")?;
    Ok(())
}

// ──────────────────────────────────────────────────────────────────
// Statistics / Leaderboard
// ──────────────────────────────────────────────────────────────────

/// Return referral statistics for a specific user.
///
/// Cycle #84: SeaORM via `Statement::from_sql_and_values`. The query
/// uses `COUNT(*) FILTER (WHERE ...)` aggregates which SeaORM's typed
/// builder API doesn't express idiomatically — raw SQL is the right
/// trade-off here (same approach as `api/loyalty.rs::get_leaderboard`).
pub(crate) async fn get_referrer_stats(
    orm: &sea_orm::DatabaseConnection,
    telegram_id: i64,
) -> Result<ReferrerStats> {
    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    let stmt = Statement::from_sql_and_values(
        DbBackend::Postgres,
        "SELECT
                COUNT(*)                                    AS total_invited,
                COUNT(*) FILTER (WHERE status = 'confirmed' OR status = 'paid') AS confirmed,
                COUNT(*) FILTER (WHERE status = 'pending')  AS pending,
                COALESCE(SUM(bonus_paid)::float8, 0)        AS total_bonus_earned
             FROM referral_events
             WHERE referrer_id = $1",
        [telegram_id.into()],
    );
    let row = orm
        .query_one(stmt)
        .await
        .context("get_referrer_stats")?
        .ok_or_else(|| anyhow::anyhow!("get_referrer_stats: empty result (impossible — COUNT)"))?;
    Ok(ReferrerStats {
        total_invited: row.try_get::<i64>("", "total_invited").unwrap_or(0),
        confirmed: row.try_get::<i64>("", "confirmed").unwrap_or(0),
        pending: row.try_get::<i64>("", "pending").unwrap_or(0),
        total_bonus_earned: {
            let v: f64 = crate::try_get_warn!(row, "total_bonus_earned", 0.0);
            if v.is_finite() {
                v.max(0.0)
            } else {
                0.0
            }
        },
    })
}

/// Return top referrers leaderboard.
/// `period` accepts "weekly" | "monthly" | "all" (anything else → all-time).
///
/// Cycle #84: SeaORM via `Statement::from_sql_and_values`. GROUP BY +
/// aggregates + LEFT JOIN — same reasoning as [`get_referrer_stats`].
pub(crate) async fn get_top_referrers(
    orm: &sea_orm::DatabaseConnection,
    period: &str,
    limit: i64,
) -> Result<Vec<TopReferrer>> {
    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    let sql = match period {
        "weekly" => {
            "SELECT
                re.referrer_id                              AS telegram_id,
                COALESCE(MAX(ul.first_name), 'Anonymous') AS first_name,
                COUNT(*)                                    AS referral_count,
                COALESCE(SUM(re.bonus_paid)::float8, 0)     AS total_bonus_earned
             FROM referral_events re
             LEFT JOIN user_languages ul ON re.referrer_id = ul.telegram_id
             WHERE (re.status = 'confirmed' OR re.status = 'paid')
             AND re.created_at >= NOW() - INTERVAL '7 days'
             GROUP BY re.referrer_id
             ORDER BY referral_count DESC, total_bonus_earned DESC
             LIMIT $1"
        }
        "monthly" => {
            "SELECT
                re.referrer_id                              AS telegram_id,
                COALESCE(MAX(ul.first_name), 'Anonymous') AS first_name,
                COUNT(*)                                  AS referral_count,
                COALESCE(SUM(re.bonus_paid)::float8, 0)   AS total_bonus_earned
             FROM referral_events re
             LEFT JOIN user_languages ul ON re.referrer_id = ul.telegram_id
             WHERE (re.status = 'confirmed' OR re.status = 'paid')
             AND re.created_at >= NOW() - INTERVAL '30 days'
             GROUP BY re.referrer_id
             ORDER BY referral_count DESC, total_bonus_earned DESC
             LIMIT $1"
        }
        _ => {
            "SELECT
                re.referrer_id                              AS telegram_id,
                COALESCE(MAX(ul.first_name), 'Anonymous') AS first_name,
                COUNT(*)                                  AS referral_count,
                COALESCE(SUM(re.bonus_paid)::float8, 0)   AS total_bonus_earned
             FROM referral_events re
             LEFT JOIN user_languages ul ON re.referrer_id = ul.telegram_id
             WHERE (re.status = 'confirmed' OR re.status = 'paid')
             GROUP BY re.referrer_id
             ORDER BY referral_count DESC, total_bonus_earned DESC
             LIMIT $1"
        }
    };
    let stmt = Statement::from_sql_and_values(DbBackend::Postgres, sql, [limit.into()]);
    let rows = orm.query_all(stmt).await.context("get_top_referrers")?;

    Ok(rows
        .iter()
        .map(|r| TopReferrer {
            telegram_id: r.try_get::<i64>("", "telegram_id").unwrap_or(0),
            first_name: r.try_get::<String>("", "first_name").ok(),
            referral_count: r.try_get::<i64>("", "referral_count").unwrap_or(0),
            total_bonus_earned: {
                let v: f64 = crate::try_get_warn!(r, "total_bonus_earned", 0.0);
                if v.is_finite() {
                    v.max(0.0)
                } else {
                    0.0
                }
            },
        })
        .collect())
}

// ──────────────────────────────────────────────────────────────────
// Unit tests (no DB required)
// ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_referral_code_length_and_alphabet() {
        let code = generate_referral_code();
        assert_eq!(code.len(), REFERRAL_CODE_LEN);
        assert!(code
            .chars()
            .all(|c| REFERRAL_CODE_ALPHABET.contains(&(c as u8))));
    }

    #[test]
    fn test_generate_referral_code_not_telegram_id() {
        // Cycle #next: codes are now opaque, not derived from telegram_id.
        let code = generate_referral_code();
        assert_ne!(code, "123456789");
        assert!(!code.chars().all(|c| c.is_ascii_digit()));
    }

    #[test]
    fn test_is_self_referral_detects_same_id() {
        assert!(is_self_referral(12345, 12345));
        assert!(is_self_referral(0, 0));
        assert!(is_self_referral(-1, -1));
    }

    #[test]
    fn test_is_self_referral_allows_distinct_ids() {
        assert!(!is_self_referral(12345, 67890));
        assert!(!is_self_referral(1, -1));
    }
}
