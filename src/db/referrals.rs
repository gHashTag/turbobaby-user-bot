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

/// Loop #20: a single invitee's visible progress, for the friends panel on the
/// referrals page.
///
/// `display_name` stays a single ready-to-print string so nothing that already
/// reads it breaks, but the parts are carried alongside it now: the screen
/// wants the handle in its own muted colour, and a client that only has the
/// joined string cannot get it back out.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Invitee {
    pub display_name: String,
    /// The Telegram handle without its `@`, when the person has one.
    pub username: Option<String>,
    /// True when nothing about this person was ever recorded, so the client
    /// can print "friend" in its own language instead of the English noun the
    /// old `COALESCE(first_name, 'Friend')` baked into the database layer.
    ///
    /// Derived from `suffix` rather than matched separately: the two say the
    /// same thing, and a boolean computed next to a string it must agree with
    /// is the shape that drifts.
    pub is_anonymous: bool,
    /// The stable `#NNNN` tail that keeps two unnamed friends apart, present
    /// only when there is no name to print.
    ///
    /// `display_name` already ends with it — glued behind the English word
    /// `Friend`, because this layer holds no language. Sending the tail on its
    /// own is what lets the client substitute its own noun without parsing the
    /// word back off a string the server formatted, which is a contract
    /// nobody wrote down and nothing would have caught breaking.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suffix: Option<String>,
    pub status: String,
    /// `streak` stood here: `COALESCE(MAX(gp.max_streak), 0)` over a
    /// `LEFT JOIN garden_plants`. Migration 083 drops that table, and Postgres
    /// does not shrug at a join onto a relation that is not there — it fails
    /// the whole statement. So this was not a list that had lost one column,
    /// it was a list that could not be produced at all: every call to
    /// `/api/referrals/{id}/invitees` answered 500. Removing the garden is D5,
    /// and a streak nobody can compute does not come back as `0` (D9).
    pub has_ordered: bool,
    pub source: Option<String>,
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

// `get_referrer_of` stood here: it read `loyalty_profiles.referred_by` for the
// milestone award, which stopped on 2026-09-26 (owner, R3: «Убрать, только
// скидка 10%»). The referral credit reads the edge from `referral_events`.

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
    // self-referral pending event. Until 2026-09-26 the confirmation paid
    // such an edge a bonus; since R3 it pays nothing, and the referral credit
    // refuses a self edge itself (`creditable_inviter`, `SelfEdge`).
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
    use sea_orm::{ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter};
    let id = Uuid::new_v4();

    // Check whether this referred user already has a referral event so we can
    // send the "friend joined" notification only on a fresh invite.
    let already_referred = RefEventEntity::find()
        .filter(RefEventCol::ReferredId.eq(referred_id))
        .one(orm)
        .await
        .context("check existing referral event")?
        .is_some();

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

    // Loop #21: notify the referrer only on a fresh invite (not a duplicate).
    if !already_referred {
        let name = crate::db::users::first_name_for(orm, referred_id)
            .await
            .unwrap_or_else(|_| "Friend".to_string());
        if let Err(e) =
            crate::db::notifications::enqueue_friend_joined(orm, referrer_id, &name).await
        {
            tracing::warn!("record_referral: failed to enqueue friend_joined: {}", e);
        }
    }

    Ok(id)
}

/// Confirm a referral edge: the invited friend's first completed order, or a
/// creditable rental a manager records for them. Pays nothing.
///
/// - Sets `status = 'confirmed'` and `confirmed_at` on the pending event
/// - Seeds the referrer's loyalty profile and adds 1 to its `referral_count`
///
/// Until 2026-09-26 this was `confirm_referral`, and it also credited the
/// referrer a fixed `referral_bonus` in loyalty points (with `bonus_paid`
/// written on the event) and the friend a welcome credit. The owner stopped
/// both that day (R3: «Убрать, только скидка 10%»): referral money is now the
/// THB credit of `crate::db::referral_credit`, born only when a manager
/// records a rental. Points already credited stay where they are. The pending
/// row is still taken under an exclusive lock (`lock_exclusive`), so two
/// confirmations count the friend once. Returns the referrer when it flipped a
/// row, `None` when there was no pending event.
///
/// Runs on the caller's connection or transaction: `record_rental` confirms
/// inside its own transaction, and [`confirm_referral_edge`] wraps it in one.
pub(crate) async fn confirm_referral_edge_in<C: sea_orm::ConnectionTrait>(
    conn: &C,
    referred_id: i64,
) -> Result<Option<i64>> {
    use crate::db::entities::{
        loyalty_profile::{ActiveModel as LpAm, Column as LpCol, Entity as LoyaltyProfileEntity},
        referral_event::{
            ActiveModel as RefEventAm, Column as RefEventCol, Entity as RefEventEntity,
        },
    };
    use sea_orm::sea_query::OnConflict;
    use sea_orm::{ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter, QuerySelect};

    // 1. The pending event, under a row lock (`SELECT ... FOR UPDATE`).
    let Some(pending) = RefEventEntity::find()
        .filter(RefEventCol::ReferredId.eq(referred_id))
        .filter(RefEventCol::Status.eq("pending"))
        .lock_exclusive()
        .one(conn)
        .await
        .context("query pending referral_event")?
    else {
        return Ok(None);
    };
    let event_id = pending.id;
    let referrer_id = pending.referrer_id;

    // 2. Mark it confirmed. No amount is written.
    let mut event_am: RefEventAm = pending.into();
    event_am.status = Set("confirmed".to_string());
    event_am.confirmed_at = Set(Some(chrono::Utc::now().into()));
    RefEventEntity::update_many()
        .set(event_am)
        .filter(RefEventCol::Id.eq(event_id))
        .exec(conn)
        .await
        .context("update referral_event status")?;

    // 3. Ensure the referrer's loyalty profile row exists (idempotent upsert).
    let lp_seed = LpAm {
        telegram_id: Set(referrer_id),
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
        .exec(conn)
        .await
        .context("upsert referrer loyalty_profile")?;

    // 4. Count the friend.
    let updated = LoyaltyProfileEntity::update_many()
        .col_expr(
            LpCol::ReferralCount,
            sea_orm::sea_query::Expr::cust("referral_count + 1"),
        )
        .filter(LpCol::TelegramId.eq(referrer_id))
        .exec(conn)
        .await
        .context("count the confirmed referral")?;
    if updated.rows_affected == 0 {
        anyhow::bail!(
            "confirm_referral_edge: loyalty profile missing for referrer_id={} after upsert (race?)",
            referrer_id
        );
    }
    Ok(Some(referrer_id))
}

/// [`confirm_referral_edge_in`] in a transaction of its own: the completion
/// path's step 7 (`complete_order_and_update_loyalty` in src/db/orders.rs),
/// which runs after the order's own commit.
pub(crate) async fn confirm_referral_edge(
    orm: &sea_orm::DatabaseConnection,
    referred_id: i64,
) -> Result<Option<i64>> {
    use sea_orm::TransactionTrait;
    let tx = orm.begin().await.context("start SeaORM tx")?;
    let confirmed = confirm_referral_edge_in(&tx, referred_id).await?;
    tx.commit().await.context("commit SeaORM referral tx")?;
    Ok(confirmed)
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

/// Loop #20: list invitees for a referrer with their order status.
///
/// The name is decided by `crate::trios::person`, which has the three cases
/// written down and tested; this function only supplies what the database
/// knows. `telegram_id` is still not exposed — the handle is, because it is
/// the thing the owner of the list asked to see and these are people they
/// personally invited, but the numeric id stays out of the response.
///
/// Before this, the query said `COALESCE(first_name, 'Friend')` and the column
/// was empty for everybody, so the panel was a list of identical strangers.
pub(crate) async fn get_invitees(
    orm: &sea_orm::DatabaseConnection,
    referrer_id: i64,
) -> Result<Vec<Invitee>> {
    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    let sql = "
        SELECT
            re.referred_id                                        AS referred_id,
            re.status                                             AS status,
            re.source                                             AS source,
            MAX(ul.first_name)                                    AS first_name,
            MAX(ul.last_name)                                     AS last_name,
            MAX(ul.username)                                      AS username,
            MAX(CASE WHEN o.id IS NOT NULL THEN 1 ELSE 0 END)   AS has_ordered
        FROM referral_events re
        LEFT JOIN user_languages ul ON ul.telegram_id = re.referred_id
        LEFT JOIN orders o ON o.telegram_id = re.referred_id
        WHERE re.referrer_id = $1
        GROUP BY re.referred_id, re.status, re.source
        ORDER BY MAX(re.created_at) DESC
    ";
    let stmt = Statement::from_sql_and_values(DbBackend::Postgres, sql, [referrer_id.into()]);
    let rows = orm.query_all(stmt).await.context("get_invitees")?;
    Ok(rows
        .iter()
        .map(|r| {
            let referred_id: i64 = r.try_get::<i64>("", "referred_id").unwrap_or(0);
            let first_name: Option<String> =
                r.try_get::<Option<String>>("", "first_name").ok().flatten();
            let last_name: Option<String> =
                r.try_get::<Option<String>>("", "last_name").ok().flatten();
            let username: Option<String> =
                r.try_get::<Option<String>>("", "username").ok().flatten();

            // `person` decides this, not the query: the rule is exercised by
            // `cargo test`, and neither this module nor the component that
            // renders it is.
            let known = crate::trios::person::Known {
                first_name: first_name.as_deref(),
                last_name: last_name.as_deref(),
                username: username.as_deref(),
            };
            let naming = crate::trios::person::name_for(&known, referred_id);
            let suffix = match &naming {
                crate::trios::person::Naming::Anonymous { suffix } => Some(suffix.clone()),
                _ => None,
            };
            // "Friend" only ever reaches the wire as the last resort, and the
            // client is told so via `is_anonymous` and `suffix`, and may say it
            // its own way.
            let display_name = crate::trios::person::one_line(&known, referred_id, "Friend");

            Invitee {
                display_name,
                username: crate::trios::person::handle(username.as_deref()),
                is_anonymous: suffix.is_some(),
                suffix,
                status: r
                    .try_get::<String>("", "status")
                    .unwrap_or_else(|_| "pending".into()),
                has_ordered: r.try_get::<i32>("", "has_ordered").unwrap_or(0) == 1,
                source: r.try_get::<String>("", "source").ok(),
            }
        })
        .collect())
}

// The milestone ladder (`MILESTONE_THRESHOLDS`, `[1, 3, 5]`) and its compiled
// amounts (`MILESTONE_DEFAULT_BONUS`) stood here. Nothing awards a milestone
// since 2026-09-26 (owner, R3: «Убрать, только скидка 10%»), so nothing offers
// one: `/api/referrals/me/:id/milestones` serves empty `thresholds` and
// `bonuses`. The rows already awarded stay, and are still read below.

/// Loop #21: return achieved referral milestones and total confirmed count.
///
/// Each milestone comes back with the amount that was **actually credited**,
/// read from its `referral_milestones` row — not recomputed. The row is the
/// only record of what the shop paid: `loyalty_config` can be edited between
/// one award and the next, so a milestone reached in June may be worth a
/// different number today. The client used to print its own
/// `1 => 100, 3 => 300, 5 => 500`, which is right until the day somebody edits
/// the config and then silently wrong for ever.
pub(crate) async fn get_referral_milestones(
    orm: &sea_orm::DatabaseConnection,
    referrer_id: i64,
) -> Result<(Vec<(i32, f64)>, i64)> {
    use crate::db::entities::referral_milestone::{
        Column as MilestoneCol, Entity as MilestoneEntity,
    };
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};

    let achieved = MilestoneEntity::find()
        .filter(MilestoneCol::ReferrerId.eq(referrer_id))
        .order_by_asc(MilestoneCol::Milestone)
        .all(orm)
        .await
        .context("fetch referral_milestones")?;
    let confirmed = count_confirmed_referrals(orm, referrer_id).await?;
    Ok((
        achieved
            .into_iter()
            .map(|m| (m.milestone, m.bonus_amount))
            .collect(),
        confirmed,
    ))
}

async fn count_confirmed_referrals(
    orm: &sea_orm::DatabaseConnection,
    referrer_id: i64,
) -> Result<i64> {
    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    let row = orm
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT COUNT(*)::bigint AS cnt FROM referral_events \
             WHERE referrer_id = $1 AND (status = 'confirmed' OR status = 'paid')",
            [referrer_id.into()],
        ))
        .await
        .context("count confirmed referrals")?;
    Ok(row
        .and_then(|r| r.try_get::<i64>("", "cnt").ok())
        .unwrap_or(0))
}

// `maybe_award_referral_milestones` and `milestone_bonus_amounts` stood here:
// the award credited loyalty points per rung reached, and queued the
// `milestone` notice; the amounts came from `loyalty_config` or the compiled
// defaults. Both stopped on 2026-09-26 (owner, R3: «Убрать, только скидка
// 10%»). Points already credited stay in the ledger, and the stored
// `loyalty_config.milestone_bonus_N` keys are inert.

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

#[cfg(test)]
mod chain_tests {
    //! The referral chain, walked the way production walks it.
    //!
    //! `tests/referrals.rs` tests a *copy* of the code generator declared in
    //! the test file, and `tests/integration_invitees.rs` seeds
    //! `referral_events` with raw SQL. Neither has ever run the path a shared
    //! link actually takes: mint a code, put it in `ref_<code>`, strip it the
    //! way the bot does, look the referrer up, record, and count. Every link in
    //! that chain is `pub(crate)`, which is why the test has to live here.
    //!
    //! Run with:
    //! ```sh
    //! DATABASE_URL=postgres://…/woody_test cargo test --features backend --lib \
    //!   chain_tests -- --ignored --test-threads=1
    //! ```
    use super::*;

    async fn db() -> Option<sea_orm::DatabaseConnection> {
        let url = std::env::var("DATABASE_URL").ok()?;
        assert!(
            url.contains("localhost") || url.contains("127.0.0.1") || url.contains("test"),
            "refusing to run against {url:?}: this test writes rows"
        );
        // Migrate first, the way `tests/common`'s harness does. Connecting raw
        // meant this test ran against whatever schema the database happened to
        // be left at: adding `user_languages.username` in migration 073 broke
        // it with `column ul.last_name does not exist`, which is a stale test
        // database and not a defect in the code under test. A test that reads
        // the schema has to be the thing that establishes it.
        let db = crate::db::Database::connect(&url).await.ok()?;
        db.run_migrations().await.ok()?;
        Some(db.orm)
    }

    /// How the bot turns `/start ref_<code>` back into a code.
    fn code_from_payload(payload: &str) -> &str {
        payload.trim_start_matches("ref_")
    }

    #[tokio::test]
    #[ignore]
    async fn a_shared_referral_link_reaches_the_counter() {
        let Some(orm) = db().await else {
            eprintln!("DATABASE_URL unset — skipping");
            return;
        };
        use sea_orm::{ConnectionTrait, DbBackend, Statement};

        let referrer = 990_001i64;
        let invitee = 990_002i64;
        for id in [referrer, invitee] {
            let _ = orm
                .execute(Statement::from_sql_and_values(
                    DbBackend::Postgres,
                    "DELETE FROM referral_events WHERE referrer_id = $1 OR referred_id = $1",
                    [id.into()],
                ))
                .await;
            let _ = orm
                .execute(Statement::from_sql_and_values(
                    DbBackend::Postgres,
                    "DELETE FROM loyalty_profiles WHERE telegram_id = $1",
                    [id.into()],
                ))
                .await;
        }

        // 1. The referrer opens the bot and is minted a code.
        let code = get_or_create_referral_code(&orm, referrer)
            .await
            .expect("mint a code");

        // 2. That code goes into a link, and the bot strips it back out.
        let payload = format!("ref_{code}");
        assert_eq!(
            code_from_payload(&payload),
            code,
            "the bot must recover the code it shared"
        );

        // 3. The bot calls `/start` for the referrer again before the invitee
        //    ever taps — every `/start` does this. If it re-mints, every link
        //    already sent stops resolving and the counter never moves.
        let again = get_or_create_referral_code(&orm, referrer)
            .await
            .expect("second /start");
        assert_eq!(
            again, code,
            "the code changed between two /start calls: every link already shared is dead"
        );

        // 4. The invitee taps. The bot looks the referrer up by the code.
        let found = find_referrer_by_code(&orm, code_from_payload(&payload))
            .await
            .expect("lookup");
        assert_eq!(
            found,
            Some(referrer),
            "the shared code did not resolve to its owner"
        );

        // 5. And records the referral.
        record_referral(
            &orm,
            referrer,
            invitee,
            code_from_payload(&payload),
            Some("telegram_start"),
        )
        .await
        .expect("record");

        // 6. The counter the profile screen reads.
        let stats = get_referrer_stats(&orm, referrer).await.expect("stats");
        assert_eq!(stats.total_invited, 1, "the invite counter did not move");

        let invitees = get_invitees(&orm, referrer).await.expect("invitees");
        assert_eq!(invitees.len(), 1, "the invitee list is empty");
    }

    /// A second tap by the same person must not inflate the count.
    #[tokio::test]
    #[ignore]
    async fn tapping_the_same_link_twice_counts_once() {
        let Some(orm) = db().await else { return };
        use sea_orm::{ConnectionTrait, DbBackend, Statement};
        let referrer = 990_011i64;
        let invitee = 990_012i64;
        for id in [referrer, invitee] {
            let _ = orm
                .execute(Statement::from_sql_and_values(
                    DbBackend::Postgres,
                    "DELETE FROM referral_events WHERE referrer_id = $1 OR referred_id = $1",
                    [id.into()],
                ))
                .await;
            let _ = orm
                .execute(Statement::from_sql_and_values(
                    DbBackend::Postgres,
                    "DELETE FROM loyalty_profiles WHERE telegram_id = $1",
                    [id.into()],
                ))
                .await;
        }
        let code = get_or_create_referral_code(&orm, referrer)
            .await
            .expect("code");
        for _ in 0..3 {
            let _ = record_referral(&orm, referrer, invitee, &code, Some("telegram_start")).await;
        }
        let stats = get_referrer_stats(&orm, referrer).await.expect("stats");
        assert_eq!(
            stats.total_invited, 1,
            "one friend counted {} times",
            stats.total_invited
        );
    }

    /// Since 2026-09-26 (owner, R3: «Убрать, только скидка 10%») confirming a
    /// referral pays nobody: the edge is confirmed and counted once, and no
    /// loyalty point moves for either person -- no referral credit to the
    /// inviter, no welcome credit to the friend, no `bonus_paid`.
    #[tokio::test]
    #[ignore]
    async fn a_confirmed_referral_credits_nobody() {
        let Some(orm) = db().await else { return };
        use sea_orm::{ConnectionTrait, DbBackend, Statement};
        let referrer = 990_021i64;
        let invitee = 990_022i64;
        for id in [referrer, invitee] {
            for sql in [
                "DELETE FROM referral_events WHERE referrer_id = $1 OR referred_id = $1",
                "DELETE FROM bonus_transactions WHERE telegram_id = $1",
                "DELETE FROM loyalty_profiles WHERE telegram_id = $1",
            ] {
                let _ = orm
                    .execute(Statement::from_sql_and_values(
                        DbBackend::Postgres,
                        sql,
                        [id.into()],
                    ))
                    .await;
            }
        }
        let code = get_or_create_referral_code(&orm, referrer)
            .await
            .expect("code");
        record_referral(&orm, referrer, invitee, &code, Some("telegram_start"))
            .await
            .expect("record");
        let points = |id: i64| {
            let orm = orm.clone();
            async move {
                let row = orm
                    .query_one(Statement::from_sql_and_values(
                        DbBackend::Postgres,
                        "SELECT COALESCE((SELECT bonus_balance::float8 FROM loyalty_profiles \
                                           WHERE telegram_id = $1), 0) AS balance, \
                                (SELECT COUNT(*)::bigint FROM bonus_transactions \
                                  WHERE telegram_id = $1) AS rows",
                        [id.into()],
                    ))
                    .await
                    .expect("query")
                    .expect("a row");
                (
                    row.try_get::<f64>("", "balance").expect("balance"),
                    row.try_get::<i64>("", "rows").expect("rows"),
                )
            }
        };
        let before = (points(referrer).await, points(invitee).await);

        assert_eq!(
            confirm_referral_edge(&orm, invitee).await.expect("confirm"),
            Some(referrer),
            "the pending edge was not flipped"
        );
        assert_eq!(
            confirm_referral_edge(&orm, invitee)
                .await
                .expect("confirm again"),
            None,
            "a confirmed edge was confirmed twice"
        );

        let row = orm
            .query_one(Statement::from_sql_and_values(
                DbBackend::Postgres,
                "SELECT re.status, re.bonus_paid::float8 AS bonus_paid, lp.referral_count \
                 FROM referral_events re JOIN loyalty_profiles lp ON lp.telegram_id = re.referrer_id \
                 WHERE re.referred_id = $1",
                [invitee.into()],
            ))
            .await
            .expect("query")
            .expect("the edge");
        assert_eq!(
            row.try_get::<String>("", "status").expect("status"),
            "confirmed"
        );
        assert_eq!(
            row.try_get::<f64>("", "bonus_paid").expect("bonus_paid"),
            0.0
        );
        assert_eq!(row.try_get::<i32>("", "referral_count").expect("count"), 1);
        assert_eq!(
            (points(referrer).await, points(invitee).await),
            before,
            "a confirmation moved loyalty points"
        );
    }
}
