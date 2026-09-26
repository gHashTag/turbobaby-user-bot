//! The referral credit's THB ledger (migration 089; owner, 2026-09-26, R3).
//!
//! The owner's words and the rule are in `crate::trios::referral_credit`,
//! which also holds the arithmetic; this module only reads and writes the
//! three tables 089 creates: `referral_rentals` (a manager's record of a
//! completed rental), `referral_requests` (a customer's «Списать в счёт
//! аренды» or «Запросить выплату») and `referral_ledger` (every movement, one
//! row each). A person's balance is `SUM(amount_thb)` over their ledger rows;
//! there is no stored balance column, so none can drift. Loyalty points never
//! enter these tables, and nothing here moves money: a payout is made by a
//! manager by hand and only RECORDED here.
//!
//! One money-creating act: [`record_rental`]. Every other write is a hold
//! ([`open_request`]), its resolution ([`resolve_request`]) or a whole
//! reversal ([`reverse_rental`], and [`reverse_rental_for_order`] from the
//! bot's reject of a completed order).
//!
//! The global lock order is (1) the orders row, (2) ONE person advisory lock
//! ([`lock_person`], two-int4 keys, so it never meets `create_order`'s
//! one-bigint lock), (3) referral rows `FOR UPDATE`. A transaction takes the
//! person lock of the one person whose balance it debits or whose request it
//! opens or resolves; credits and reversals take none. A balance may go
//! negative after a reversal, and the only guards are on debits.
//!
//! Every read here fails loud: a column that cannot be read is an error, never
//! a zero. Every timestamp is written by SQL (`NOW()` or a column default),
//! never bound. `specs/turbobaby/referral_credit.t27` records the rules.

use anyhow::anyhow;
use chrono::{DateTime, SecondsFormat, Utc};
use sea_orm::{
    ConnectionTrait, DatabaseConnection, DbBackend, DbErr, QueryResult, Statement, TransactionTrait,
};

use crate::trios::referral_credit::{
    applied_redemption, available, credit_for_rental, creditable_inviter, order_holds_a_rental,
    AdminOverview, AdminRequestRow, BalanceRow, CreditOutcome, InviteeRow, NotCreditable,
    OpenRequestResponse, RecordRentalBody, RecordRentalResponse, RedemptionOutcome, ReferralCredit,
    ReferralRequest, RentalRow, ResolveResponse, ReverseResponse, LOCK_NAMESPACE,
};

/// The migration whose `_schema_migrations.applied_at` starts the programme:
/// an order created before it is not credited (no retroactive credit).
const PROGRAM_MIGRATION: &str = "089_referral_credit.sql";

/// The note an automatic reversal carries: the bot's reject of the order.
const BOT_REJECT_NOTE: &str = "order rejected in the bot";

const RENTAL_COLUMNS: &str = "id, customer_telegram_id, order_id, rental_amount_thb, \
     applied_thb, inviter_telegram_id, credit_thb, note, recorded_by, recorded_at, \
     reversed_at, reversed_by, reversal_note";

const REQUEST_COLUMNS: &str = "id, kind, amount_thb, status, created_at";

/// One admin-view request row, with the requester's names and current balance.
const ADMIN_REQUEST_SELECT: &str = "SELECT rq.id, rq.telegram_id, ul.first_name, ul.username, \
     rq.kind, rq.amount_thb, rq.status, rq.created_at, rq.resolved_at, rq.resolved_by, \
     rq.rental_id, rq.applied_thb, rq.admin_note, \
     (SELECT COALESCE(SUM(l.amount_thb), 0)::bigint FROM referral_ledger l \
       WHERE l.telegram_id = rq.telegram_id) AS balance_thb \
     FROM referral_requests rq LEFT JOIN user_languages ul ON ul.telegram_id = rq.telegram_id";

/// Why a credit write refused. The API turns each into its status and code.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Refusal {
    OrderNotFound,
    OrderOfAnotherCustomer,
    OrderHasNoRentalLine,
    OrderNotCompleted,
    OrderBeforeProgram,
    OrderAlreadyRecorded,
    IdempotencyKeyReused,
    RecorderIsInviter,
    NoReferralEffect(NotCreditable),
    RequestOpen(ReferralRequest),
    NothingAvailable,
    RentalNotFound,
    RequestNotFound,
    RequestNotOpen(String),
    RedeemCannotBePaid,
    BalanceBelowRequest,
}

/// A refusal, or a database failure (a 500).
#[derive(Debug)]
pub(crate) enum CreditError {
    Refused(Refusal),
    Db(anyhow::Error),
}

impl From<DbErr> for CreditError {
    fn from(e: DbErr) -> Self {
        CreditError::Db(e.into())
    }
}

impl From<anyhow::Error> for CreditError {
    fn from(e: anyhow::Error) -> Self {
        CreditError::Db(e)
    }
}

type CreditResult<T> = Result<T, CreditError>;

fn refuse<T>(refusal: Refusal) -> CreditResult<T> {
    Err(CreditError::Refused(refusal))
}

fn stmt(sql: &str, values: Vec<sea_orm::Value>) -> Statement {
    Statement::from_sql_and_values(DbBackend::Postgres, sql, values)
}

/// RFC 3339 UTC, whole seconds: `2026-09-27T10:00:00Z`.
fn stamp(at: DateTime<Utc>) -> String {
    at.to_rfc3339_opts(SecondsFormat::Secs, true)
}

fn missing_row(what: &str) -> DbErr {
    DbErr::Custom(format!("referral credit: {what} returned no row"))
}

/// Take the transaction-scoped advisory lock of one person's referral balance.
///
/// `pg_advisory_xact_lock(hashtext('referral_credit'), hashtext(tid))`: the
/// two-int4 key space, which PostgreSQL keeps apart from the one-bigint key
/// space `create_order`'s `pg_advisory_xact_lock($tid)` uses, so the two can
/// never collide.
pub(crate) async fn lock_person<C: ConnectionTrait>(
    conn: &C,
    telegram_id: i64,
) -> Result<(), DbErr> {
    conn.execute(stmt(
        "SELECT pg_advisory_xact_lock(hashtext($1), hashtext($2::text))",
        vec![LOCK_NAMESPACE.into(), telegram_id.to_string().into()],
    ))
    .await?;
    Ok(())
}

/// A person's referral balance: the sum of their ledger rows, signed.
pub(crate) async fn balance_thb<C: ConnectionTrait>(
    conn: &C,
    telegram_id: i64,
) -> Result<i64, DbErr> {
    let row = conn
        .query_one(stmt(
            "SELECT COALESCE(SUM(amount_thb), 0)::bigint AS balance FROM referral_ledger \
             WHERE telegram_id = $1",
            vec![telegram_id.into()],
        ))
        .await?
        .ok_or_else(|| missing_row("the balance sum"))?;
    row.try_get::<i64>("", "balance")
}

fn request_of(row: &QueryResult) -> Result<ReferralRequest, DbErr> {
    Ok(ReferralRequest {
        id: row.try_get("", "id")?,
        kind: row.try_get("", "kind")?,
        amount_thb: row.try_get("", "amount_thb")?,
        status: row.try_get("", "status")?,
        created_at: stamp(row.try_get::<DateTime<Utc>>("", "created_at")?),
    })
}

fn rental_of(row: &QueryResult) -> Result<RentalRow, DbErr> {
    Ok(RentalRow {
        id: row.try_get("", "id")?,
        customer_telegram_id: row.try_get("", "customer_telegram_id")?,
        order_id: row.try_get("", "order_id")?,
        rental_amount_thb: row.try_get("", "rental_amount_thb")?,
        applied_thb: row.try_get("", "applied_thb")?,
        inviter_telegram_id: row.try_get("", "inviter_telegram_id")?,
        credit_thb: row.try_get("", "credit_thb")?,
        note: row.try_get("", "note")?,
        recorded_by: row.try_get("", "recorded_by")?,
        recorded_at: stamp(row.try_get::<DateTime<Utc>>("", "recorded_at")?),
        reversed_at: row
            .try_get::<Option<DateTime<Utc>>>("", "reversed_at")?
            .map(stamp),
        reversed_by: row.try_get("", "reversed_by")?,
        reversal_note: row.try_get("", "reversal_note")?,
    })
}

fn admin_request_of(row: &QueryResult) -> Result<AdminRequestRow, DbErr> {
    Ok(AdminRequestRow {
        id: row.try_get("", "id")?,
        telegram_id: row.try_get("", "telegram_id")?,
        first_name: row.try_get("", "first_name")?,
        username: row.try_get("", "username")?,
        kind: row.try_get("", "kind")?,
        amount_thb: row.try_get("", "amount_thb")?,
        status: row.try_get("", "status")?,
        created_at: stamp(row.try_get::<DateTime<Utc>>("", "created_at")?),
        resolved_at: row
            .try_get::<Option<DateTime<Utc>>>("", "resolved_at")?
            .map(stamp),
        resolved_by: row.try_get("", "resolved_by")?,
        rental_id: row.try_get("", "rental_id")?,
        applied_thb: row.try_get("", "applied_thb")?,
        admin_note: row.try_get("", "admin_note")?,
        balance_thb: row.try_get("", "balance_thb")?,
    })
}

/// What a customer is shown: the signed balance, the hold of their one open
/// request, and what is still available. No entry, friend or order.
fn summary(balance_thb: i64, open_request: Option<ReferralRequest>) -> ReferralCredit {
    let held_thb = open_request.as_ref().map_or(0, |r| r.amount_thb);
    ReferralCredit {
        balance_thb,
        held_thb,
        available_thb: available(balance_thb, held_thb),
        open_request,
    }
}

/// `GET /api/referral-credit/me/:telegram_id`: two SELECTs, no lock.
pub(crate) async fn credit_summary(
    orm: &DatabaseConnection,
    telegram_id: i64,
) -> Result<ReferralCredit, DbErr> {
    let balance = balance_thb(orm, telegram_id).await?;
    let open = orm
        .query_one(stmt(
            &format!(
                "SELECT {REQUEST_COLUMNS} FROM referral_requests \
                 WHERE telegram_id = $1 AND status = 'open'"
            ),
            vec![telegram_id.into()],
        ))
        .await?;
    let open_request = match open {
        Some(row) => Some(request_of(&row)?),
        None => None,
    };
    Ok(summary(balance, open_request))
}

/// A customer's first name and handle, for the admins' notice. Absent when
/// nothing was ever recorded about them.
pub(crate) async fn names_of(
    orm: &DatabaseConnection,
    telegram_id: i64,
) -> Result<(Option<String>, Option<String>), DbErr> {
    let row = orm
        .query_one(stmt(
            "SELECT first_name, username FROM user_languages WHERE telegram_id = $1",
            vec![telegram_id.into()],
        ))
        .await?;
    match row {
        Some(row) => Ok((row.try_get("", "first_name")?, row.try_get("", "username")?)),
        None => Ok((None, None)),
    }
}

/// «Списать в счёт аренды» (`redeem`) or «Запросить выплату» (`payout`): hold
/// the whole available balance under one open request.
///
/// The same kind already open answers that request (`already_open`); the
/// other kind open is refused. Moves no money.
pub(crate) async fn open_request(
    orm: &DatabaseConnection,
    telegram_id: i64,
    kind: &str,
) -> CreditResult<OpenRequestResponse> {
    let tx = orm.begin().await?;
    lock_person(&tx, telegram_id).await?;

    let open = tx
        .query_one(stmt(
            &format!(
                "SELECT {REQUEST_COLUMNS} FROM referral_requests \
                 WHERE telegram_id = $1 AND status = 'open' FOR UPDATE"
            ),
            vec![telegram_id.into()],
        ))
        .await?;
    if let Some(row) = open {
        let request = request_of(&row)?;
        if request.kind != kind {
            return refuse(Refusal::RequestOpen(request));
        }
        let balance = balance_thb(&tx, telegram_id).await?;
        tx.commit().await?;
        return Ok(OpenRequestResponse {
            credit: summary(balance, Some(request.clone())),
            request,
            already_open: true,
        });
    }

    let balance = balance_thb(&tx, telegram_id).await?;
    let amount = available(balance, 0);
    if amount < 1 {
        return refuse(Refusal::NothingAvailable);
    }
    let row = tx
        .query_one(stmt(
            &format!(
                "INSERT INTO referral_requests (telegram_id, kind, amount_thb) \
                 VALUES ($1, $2, $3) RETURNING {REQUEST_COLUMNS}"
            ),
            vec![telegram_id.into(), kind.into(), amount.into()],
        ))
        .await?
        .ok_or_else(|| missing_row("the request insert"))?;
    let request = request_of(&row)?;
    tx.commit().await?;
    Ok(OpenRequestResponse {
        credit: summary(balance, Some(request.clone())),
        request,
        already_open: false,
    })
}

async fn insert_ledger<C: ConnectionTrait>(
    conn: &C,
    telegram_id: i64,
    kind: &str,
    amount_thb: i64,
    rental_id: Option<i64>,
    request_id: Option<i64>,
    created_by: i64,
) -> Result<(), DbErr> {
    conn.execute(stmt(
        "INSERT INTO referral_ledger (telegram_id, kind, amount_thb, rental_id, request_id, created_by) \
         VALUES ($1, $2, $3, $4, $5, $6)",
        vec![
            telegram_id.into(),
            kind.into(),
            amount_thb.into(),
            rental_id.into(),
            request_id.into(),
            created_by.into(),
        ],
    ))
    .await?;
    Ok(())
}

async fn rental_by_key<C: ConnectionTrait>(
    conn: &C,
    key: &str,
) -> Result<Option<RentalRow>, DbErr> {
    let row = conn
        .query_one(stmt(
            &format!("SELECT {RENTAL_COLUMNS} FROM referral_rentals WHERE idempotency_key = $1"),
            vec![key.into()],
        ))
        .await?;
    match row {
        Some(row) => Ok(Some(rental_of(&row)?)),
        None => Ok(None),
    }
}

/// The answer a stored record gives to a replay of its key.
async fn replay_of<C: ConnectionTrait>(
    conn: &C,
    stored: RentalRow,
) -> CreditResult<RecordRentalResponse> {
    let credit = stored.inviter_telegram_id.map(|inviter| CreditOutcome {
        inviter_telegram_id: inviter,
        credit_thb: stored.credit_thb,
    });
    let redemption = if stored.applied_thb > 0 {
        let row = conn
            .query_one(stmt(
                "SELECT id FROM referral_requests WHERE rental_id = $1",
                vec![stored.id.into()],
            ))
            .await?
            .ok_or_else(|| missing_row("the settled request of a replayed record"))?;
        Some(RedemptionOutcome {
            request_id: row.try_get("", "id")?,
            applied_thb: stored.applied_thb,
        })
    } else {
        None
    };
    Ok(RecordRentalResponse {
        rental: stored,
        credit,
        redemption,
        idempotent_replay: true,
    })
}

/// The order-link guards of a record, on the order row already locked.
async fn check_order<C: ConnectionTrait>(
    conn: &C,
    order: &QueryResult,
    order_id: &str,
    customer: i64,
) -> CreditResult<()> {
    let owner: Option<i64> = order.try_get("", "telegram_id")?;
    if owner != Some(customer) {
        return refuse(Refusal::OrderOfAnotherCustomer);
    }
    let items: serde_json::Value = order.try_get("", "items")?;
    if !order_holds_a_rental(&items) {
        return refuse(Refusal::OrderHasNoRentalLine);
    }
    let status: String = order.try_get("", "status")?;
    if status != "completed" {
        return refuse(Refusal::OrderNotCompleted);
    }
    let created_at: DateTime<Utc> = order.try_get("", "created_at")?;
    let program = conn
        .query_one(stmt(
            "SELECT applied_at FROM _schema_migrations WHERE name = $1",
            vec![PROGRAM_MIGRATION.into()],
        ))
        .await?
        .ok_or_else(|| anyhow!("{PROGRAM_MIGRATION} is not recorded in _schema_migrations"))?;
    let started: DateTime<Utc> = program.try_get("", "applied_at")?;
    if created_at < started {
        return refuse(Refusal::OrderBeforeProgram);
    }
    let live = conn
        .query_one(stmt(
            "SELECT id FROM referral_rentals WHERE order_id = $1 AND reversed_at IS NULL",
            vec![order_id.into()],
        ))
        .await?;
    if live.is_some() {
        return refuse(Refusal::OrderAlreadyRecorded);
    }
    Ok(())
}

/// Whether a record would credit the admin who records it
/// (`RECORDER_MAY_BE_THE_INVITER = false` in `referral_credit.t27`).
///
/// `check_admin` answers the recorder's own id when Telegram names him, and
/// 0 for the shared password token, which names nobody. A named recorder is
/// refused when he is the inviter. A password record is refused when the
/// inviter is on `admin_ids`, since any admin on the list may be the one
/// holding the password. Not caught (review of 2026-09-26, DECISIONS.md): a
/// person who knows the password and is not on the list, recording a friend
/// he invited; nothing on the request tells him apart from a manager.
fn recorder_is_the_inviter(recorder: i64, inviter: i64, admin_ids: &[i64]) -> bool {
    if recorder != 0 {
        recorder == inviter
    } else {
        admin_ids.contains(&inviter)
    }
}

/// A UNIQUE violation on the record's insert is a concurrent twin: the key
/// under another payload, or a second live record of one order.
fn insert_refusal(e: DbErr) -> CreditError {
    let text = e.to_string();
    if text.contains("referral_rentals_idempotency_key_key") {
        CreditError::Refused(Refusal::IdempotencyKeyReused)
    } else if text.contains("referral_rentals_one_live_per_order") {
        CreditError::Refused(Refusal::OrderAlreadyRecorded)
    } else {
        CreditError::Db(e.into())
    }
}

/// `POST /api/admin/referral-credit/rentals`: a manager records one completed
/// rental of a customer, with the charge he took (without deposit and
/// delivery). The one act that creates referral money.
///
/// In one transaction: an open «Списать в счёт аренды» of the customer is
/// settled against this rental (`applied`), and the customer's creditable
/// inviter is credited `floor(10% × (charge − applied))`. A record that
/// neither credits nor settles anything is refused and writes nothing, and
/// so is one that would credit its own recorder ([`recorder_is_the_inviter`]:
/// `admin_id` is `check_admin`'s answer, 0 for the password token, and
/// `admin_ids` the configured admin list).
pub(crate) async fn record_rental(
    orm: &DatabaseConnection,
    admin_id: i64,
    admin_ids: &[i64],
    body: &RecordRentalBody,
) -> CreditResult<RecordRentalResponse> {
    let customer = body.customer_telegram_id;
    let amount = body.rental_amount_thb;
    let tx = orm.begin().await?;

    // (1) The order row first, in the global lock order.
    let mut order_created_at: Option<DateTime<Utc>> = None;
    if let Some(order_id) = body.order_id.as_deref() {
        let Some(order) = tx
            .query_one(stmt(
                "SELECT telegram_id, status, items, created_at FROM orders WHERE id = $1 FOR UPDATE",
                vec![order_id.into()],
            ))
            .await?
        else {
            return refuse(Refusal::OrderNotFound);
        };
        order_created_at = Some(order.try_get::<DateTime<Utc>>("", "created_at")?);
        // A replay of a key already stored skips the guards its first call
        // passed: they are about the order as it was then, and the answer to
        // a retry is the stored record (or the key's refusal, below).
        if rental_by_key(&tx, &body.idempotency_key).await?.is_none() {
            check_order(&tx, &order, order_id, customer).await?;
        }
    }

    // (2) The customer's person lock: their balance may be debited below.
    lock_person(&tx, customer).await?;

    // (3) The key, compared with its payload.
    if let Some(stored) = rental_by_key(&tx, &body.idempotency_key).await? {
        if stored.customer_telegram_id == customer
            && stored.rental_amount_thb == amount
            && stored.order_id == body.order_id
        {
            let response = replay_of(&tx, stored).await?;
            tx.commit().await?;
            return Ok(response);
        }
        return refuse(Refusal::IdempotencyKeyReused);
    }

    // (4) The customer's open «Списать в счёт аренды», and what it applies.
    let redeem = tx
        .query_one(stmt(
            "SELECT id, amount_thb FROM referral_requests \
             WHERE telegram_id = $1 AND kind = 'redeem' AND status = 'open' FOR UPDATE",
            vec![customer.into()],
        ))
        .await?;
    let (redeem_id, applied) = match redeem {
        Some(row) => {
            let id: i64 = row.try_get("", "id")?;
            let hold: i64 = row.try_get("", "amount_thb")?;
            let balance = balance_thb(&tx, customer).await?;
            (Some(id), applied_redemption(hold, amount, balance))
        }
        None => (None, 0),
    };

    // (5) The edge, and whether the customer was one before it.
    let edge = tx
        .query_one(stmt(
            "SELECT re.referrer_id, re.created_at AS edge_at, \
                    (EXISTS (SELECT 1 FROM loyalty_profiles lp \
                              WHERE lp.telegram_id = re.referred_id \
                                AND lp.first_purchase_at < re.created_at) \
                     OR EXISTS (SELECT 1 FROM referral_rentals rr \
                                 WHERE rr.customer_telegram_id = re.referred_id \
                                   AND rr.recorded_at < re.created_at)) AS before_edge \
             FROM referral_events re WHERE re.referred_id = $1",
            vec![customer.into()],
        ))
        .await?;
    let (edge_referrer, edge_after_order, customer_before_edge) = match edge {
        Some(row) => {
            let referrer: i64 = row.try_get("", "referrer_id")?;
            let edge_at: DateTime<Utc> = row.try_get("", "edge_at")?;
            let before: bool = row.try_get("", "before_edge")?;
            let after_order = order_created_at.is_some_and(|ordered| edge_at > ordered);
            (Some(referrer), after_order, before)
        }
        None => (None, false, false),
    };
    let creditable = creditable_inviter(
        customer,
        edge_referrer,
        edge_after_order,
        customer_before_edge,
    );
    let inviter = match creditable {
        Ok(inviter) if recorder_is_the_inviter(admin_id, inviter, admin_ids) => {
            return refuse(Refusal::RecorderIsInviter);
        }
        Ok(inviter) => Some(inviter),
        Err(reason) if applied == 0 => return refuse(Refusal::NoReferralEffect(reason)),
        Err(_) => None,
    };
    let credit = match inviter {
        Some(_) => credit_for_rental(amount, applied),
        None => 0,
    };

    // (6) The record.
    let row = tx
        .query_one(stmt(
            &format!(
                "INSERT INTO referral_rentals (customer_telegram_id, order_id, rental_amount_thb, \
                 applied_thb, inviter_telegram_id, credit_thb, note, idempotency_key, recorded_by) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9) RETURNING {RENTAL_COLUMNS}"
            ),
            vec![
                customer.into(),
                body.order_id.clone().into(),
                amount.into(),
                applied.into(),
                inviter.into(),
                credit.into(),
                body.note.clone().into(),
                body.idempotency_key.clone().into(),
                admin_id.into(),
            ],
        ))
        .await
        .map_err(insert_refusal)?
        .ok_or_else(|| missing_row("the rental insert"))?;
    let rental = rental_of(&row)?;

    // (7) The redemption it settles.
    let mut redemption = None;
    if applied > 0 {
        let request_id =
            redeem_id.ok_or_else(|| anyhow!("an applied redemption without its request"))?;
        insert_ledger(
            &tx,
            customer,
            "redeem",
            -applied,
            Some(rental.id),
            Some(request_id),
            admin_id,
        )
        .await?;
        let settled = tx
            .execute(stmt(
                "UPDATE referral_requests SET status = 'applied', applied_thb = $1, rental_id = $2, \
                 resolved_at = NOW(), resolved_by = $3 WHERE id = $4 AND status = 'open'",
                vec![applied.into(), rental.id.into(), admin_id.into(), request_id.into()],
            ))
            .await?;
        if settled.rows_affected() != 1 {
            return Err(
                anyhow!("the redeem request {request_id} left 'open' under its lock").into(),
            );
        }
        redemption = Some(RedemptionOutcome {
            request_id,
            applied_thb: applied,
        });
    }

    // (8) The credit, and the edge confirmed (no money moves there).
    if let Some(inviter) = inviter {
        if credit > 0 {
            insert_ledger(
                &tx,
                inviter,
                "accrual",
                credit,
                Some(rental.id),
                None,
                admin_id,
            )
            .await?;
        }
        crate::db::referrals::confirm_referral_edge_in(&tx, customer).await?;
    }

    tx.commit().await?;
    Ok(RecordRentalResponse {
        rental,
        credit: inviter.map(|inviter| CreditOutcome {
            inviter_telegram_id: inviter,
            credit_thb: credit,
        }),
        redemption,
        idempotent_replay: false,
    })
}

/// Reverse one locked record whole: take back the inviter's credit, return
/// the customer's applied balance, and mark the row. Idempotent.
async fn reverse_locked<C: ConnectionTrait>(
    conn: &C,
    rental: RentalRow,
    by: i64,
    note: Option<&str>,
) -> Result<ReverseResponse, DbErr> {
    let accrual = match rental.inviter_telegram_id {
        Some(_) => rental.credit_thb,
        None => 0,
    };
    let returned = rental.applied_thb;
    if rental.reversed_at.is_some() {
        return Ok(ReverseResponse {
            rental,
            accrual_reversed_thb: accrual,
            redemption_returned_thb: returned,
            already_reversed: true,
        });
    }
    if let Some(inviter) = rental.inviter_telegram_id {
        if accrual > 0 {
            insert_ledger(
                conn,
                inviter,
                "accrual_reversal",
                -accrual,
                Some(rental.id),
                None,
                by,
            )
            .await?;
        }
    }
    if returned > 0 {
        insert_ledger(
            conn,
            rental.customer_telegram_id,
            "redeem_reversal",
            returned,
            Some(rental.id),
            None,
            by,
        )
        .await?;
    }
    let row = conn
        .query_one(stmt(
            &format!(
                "UPDATE referral_rentals SET reversed_at = NOW(), reversed_by = $1, reversal_note = $2 \
                 WHERE id = $3 AND reversed_at IS NULL RETURNING {RENTAL_COLUMNS}"
            ),
            vec![by.into(), note.map(str::to_string).into(), rental.id.into()],
        ))
        .await?
        .ok_or_else(|| missing_row("the reversal of a locked record"))?;
    Ok(ReverseResponse {
        rental: rental_of(&row)?,
        accrual_reversed_thb: accrual,
        redemption_returned_thb: returned,
        already_reversed: false,
    })
}

/// `POST /api/admin/referral-credit/rentals/:id/reverse`: a manager reverses
/// one record whole. A correction is a reversal plus a new record.
pub(crate) async fn reverse_rental(
    orm: &DatabaseConnection,
    rental_id: i64,
    by: i64,
    note: Option<&str>,
) -> CreditResult<ReverseResponse> {
    let tx = orm.begin().await?;
    let Some(row) = tx
        .query_one(stmt(
            &format!("SELECT {RENTAL_COLUMNS} FROM referral_rentals WHERE id = $1 FOR UPDATE"),
            vec![rental_id.into()],
        ))
        .await?
    else {
        return refuse(Refusal::RentalNotFound);
    };
    let response = reverse_locked(&tx, rental_of(&row)?, by, note).await?;
    tx.commit().await?;
    Ok(response)
}

/// The bot's reject of an order reverses every live record of that order, in
/// the reject's own transaction (the order row is already locked there). A
/// no-op without a live record. Returns how many it reversed.
pub(crate) async fn reverse_rental_for_order<C: ConnectionTrait>(
    conn: &C,
    order_id: &str,
    by: i64,
) -> Result<u64, DbErr> {
    let rows = conn
        .query_all(stmt(
            &format!(
                "SELECT {RENTAL_COLUMNS} FROM referral_rentals \
                 WHERE order_id = $1 AND reversed_at IS NULL FOR UPDATE"
            ),
            vec![order_id.into()],
        ))
        .await?;
    let mut reversed = 0u64;
    for row in rows {
        reverse_locked(conn, rental_of(&row)?, by, Some(BOT_REJECT_NOTE)).await?;
        reversed += 1;
    }
    Ok(reversed)
}

async fn admin_request<C: ConnectionTrait>(
    conn: &C,
    request_id: i64,
) -> Result<AdminRequestRow, DbErr> {
    let row = conn
        .query_one(stmt(
            &format!("{ADMIN_REQUEST_SELECT} WHERE rq.id = $1"),
            vec![request_id.into()],
        ))
        .await?
        .ok_or_else(|| missing_row("the resolved request"))?;
    admin_request_of(&row)
}

/// `POST /api/admin/referral-credit/requests/:id/resolve`: `paid` records a
/// payout the manager made by hand (payout requests only, never partial, and
/// never above the balance); `declined` frees the hold of either kind.
pub(crate) async fn resolve_request(
    orm: &DatabaseConnection,
    request_id: i64,
    admin_id: i64,
    action: &str,
    note: Option<&str>,
) -> CreditResult<ResolveResponse> {
    let Some(owner) = orm
        .query_one(stmt(
            "SELECT telegram_id FROM referral_requests WHERE id = $1",
            vec![request_id.into()],
        ))
        .await?
    else {
        return refuse(Refusal::RequestNotFound);
    };
    let telegram_id: i64 = owner.try_get("", "telegram_id")?;

    let tx = orm.begin().await?;
    lock_person(&tx, telegram_id).await?;
    let row = tx
        .query_one(stmt(
            "SELECT kind, amount_thb, status FROM referral_requests WHERE id = $1 FOR UPDATE",
            vec![request_id.into()],
        ))
        .await?
        .ok_or_else(|| missing_row("the locked request"))?;
    let kind: String = row.try_get("", "kind")?;
    let amount: i64 = row.try_get("", "amount_thb")?;
    let status: String = row.try_get("", "status")?;

    if status == action {
        let request = admin_request(&tx, request_id).await?;
        let balance = balance_thb(&tx, telegram_id).await?;
        tx.commit().await?;
        return Ok(ResolveResponse {
            request,
            balance_thb: balance,
            idempotent_replay: true,
        });
    }
    if status != "open" {
        return refuse(Refusal::RequestNotOpen(status));
    }
    if action == "paid" {
        if kind != "payout" {
            return refuse(Refusal::RedeemCannotBePaid);
        }
        if balance_thb(&tx, telegram_id).await? < amount {
            return refuse(Refusal::BalanceBelowRequest);
        }
        insert_ledger(
            &tx,
            telegram_id,
            "payout",
            -amount,
            None,
            Some(request_id),
            admin_id,
        )
        .await?;
    }
    let resolved = tx
        .execute(stmt(
            "UPDATE referral_requests SET status = $1, resolved_at = NOW(), resolved_by = $2, \
             admin_note = $3 WHERE id = $4 AND status = 'open'",
            vec![
                action.into(),
                admin_id.into(),
                note.map(str::to_string).into(),
                request_id.into(),
            ],
        ))
        .await?;
    if resolved.rows_affected() != 1 {
        return Err(anyhow!("request {request_id} left 'open' under its lock").into());
    }
    let request = admin_request(&tx, request_id).await?;
    let balance = balance_thb(&tx, telegram_id).await?;
    tx.commit().await?;
    Ok(ResolveResponse {
        request,
        balance_thb: balance,
        idempotent_replay: false,
    })
}

/// `GET /api/admin/referral-credit/overview`: every open request (oldest
/// first), the newest 200 invited friends, the newest 100 records and every
/// non-zero balance (the 200 largest).
pub(crate) async fn overview(orm: &DatabaseConnection) -> Result<AdminOverview, DbErr> {
    let open_requests = orm
        .query_all(stmt(
            &format!("{ADMIN_REQUEST_SELECT} WHERE rq.status = 'open' ORDER BY rq.created_at ASC, rq.id ASC"),
            vec![],
        ))
        .await?
        .iter()
        .map(admin_request_of)
        .collect::<Result<Vec<_>, _>>()?;

    let invitee_rows = orm
        .query_all(stmt(
            "SELECT re.referred_id, ul.first_name, ul.username, re.referrer_id, \
                    ur.first_name AS inviter_first_name, ur.username AS inviter_username, \
                    re.created_at AS invited_at, re.status AS edge_status, \
                    (EXISTS (SELECT 1 FROM loyalty_profiles lp \
                              WHERE lp.telegram_id = re.referred_id \
                                AND lp.first_purchase_at < re.created_at) \
                     OR EXISTS (SELECT 1 FROM referral_rentals rr \
                                 WHERE rr.customer_telegram_id = re.referred_id \
                                   AND rr.recorded_at < re.created_at)) AS before_edge \
             FROM referral_events re \
             LEFT JOIN user_languages ul ON ul.telegram_id = re.referred_id \
             LEFT JOIN user_languages ur ON ur.telegram_id = re.referrer_id \
             ORDER BY re.created_at DESC LIMIT 200",
            vec![],
        ))
        .await?;
    // What each friend's live records add up to, read apart from the
    // friends so the two statements stay flat.
    let mut live: std::collections::HashMap<i64, (i64, i64)> = std::collections::HashMap::new();
    for row in orm
        .query_all(stmt(
            "SELECT customer_telegram_id, COUNT(*)::bigint AS rentals, \
                    COALESCE(SUM(credit_thb), 0)::bigint AS credited \
             FROM referral_rentals WHERE reversed_at IS NULL GROUP BY customer_telegram_id",
            vec![],
        ))
        .await?
    {
        live.insert(
            row.try_get("", "customer_telegram_id")?,
            (row.try_get("", "rentals")?, row.try_get("", "credited")?),
        );
    }
    let mut invitees = Vec::with_capacity(invitee_rows.len());
    for row in &invitee_rows {
        let telegram_id: i64 = row.try_get("", "referred_id")?;
        let inviter: i64 = row.try_get("", "referrer_id")?;
        let before_edge: bool = row.try_get("", "before_edge")?;
        // `edge_after_order` is a question about one order, so it is asked
        // when a rental is recorded against one, never here.
        let verdict = creditable_inviter(telegram_id, Some(inviter), false, before_edge);
        invitees.push(InviteeRow {
            telegram_id,
            first_name: row.try_get("", "first_name")?,
            username: row.try_get("", "username")?,
            inviter_telegram_id: inviter,
            inviter_first_name: row.try_get("", "inviter_first_name")?,
            inviter_username: row.try_get("", "inviter_username")?,
            invited_at: stamp(row.try_get::<DateTime<Utc>>("", "invited_at")?),
            edge_status: row.try_get("", "edge_status")?,
            rentals_recorded: live.get(&telegram_id).map_or(0, |(rentals, _)| *rentals),
            credit_thb: live.get(&telegram_id).map_or(0, |(_, credited)| *credited),
            creditable: verdict.is_ok(),
            not_creditable_reason: verdict.err().map(|reason| reason.code().to_string()),
        });
    }

    let rentals = orm
        .query_all(stmt(
            &format!(
                "SELECT {RENTAL_COLUMNS} FROM referral_rentals ORDER BY recorded_at DESC, id DESC LIMIT 100"
            ),
            vec![],
        ))
        .await?
        .iter()
        .map(rental_of)
        .collect::<Result<Vec<_>, _>>()?;

    let balance_rows = orm
        .query_all(stmt(
            "SELECT l.telegram_id, ul.first_name, ul.username, SUM(l.amount_thb)::bigint AS balance_thb \
             FROM referral_ledger l LEFT JOIN user_languages ul ON ul.telegram_id = l.telegram_id \
             GROUP BY l.telegram_id, ul.first_name, ul.username \
             HAVING SUM(l.amount_thb) <> 0 \
             ORDER BY balance_thb DESC, l.telegram_id ASC LIMIT 200",
            vec![],
        ))
        .await?;
    let mut balances = Vec::with_capacity(balance_rows.len());
    for row in &balance_rows {
        balances.push(BalanceRow {
            telegram_id: row.try_get("", "telegram_id")?,
            first_name: row.try_get("", "first_name")?,
            username: row.try_get("", "username")?,
            balance_thb: row.try_get("", "balance_thb")?,
        });
    }

    Ok(AdminOverview {
        open_requests,
        invitees,
        rentals,
        balances,
    })
}

#[cfg(test)]
mod tests {
    //! On a throwaway PostgreSQL only (`#[ignore]`), the way `tests/common`
    //! migrates one. The bot's reject path lives here rather than under
    //! `tests/` because this module is `pub(crate)`. Run with:
    //! ```sh
    //! DATABASE_URL=postgres://…/<throwaway db> cargo test --features backend --lib \
    //!   db::referral_credit::tests -- --ignored --test-threads=1
    //! ```
    use super::*;
    // Imported under another name so that the gate-3 count of this function's
    // call sites (AUTOMATIC_REVERSAL_SITES, over src/) keeps meaning "the
    // bot's reject", which is the only production caller.
    use super::reverse_rental_for_order as reverse_for_a_rejected_order;

    async fn db() -> Option<DatabaseConnection> {
        let url = std::env::var("DATABASE_URL").ok()?;
        assert!(
            url.contains("localhost") || url.contains("127.0.0.1") || url.contains("test"),
            "refusing to run against {url:?}: this test writes rows"
        );
        let db = crate::db::Database::connect(&url)
            .await
            .expect("connect to the DATABASE_URL this test was pointed at");
        db.run_migrations()
            .await
            .expect("migrate the test database");
        Some(db.orm)
    }

    fn unique_id() -> i64 {
        8_000_000_000_i64 + (uuid::Uuid::new_v4().as_u128() % 1_000_000_000) as i64
    }

    async fn exec(orm: &DatabaseConnection, sql: &str, values: Vec<sea_orm::Value>) {
        orm.execute(stmt(sql, values)).await.expect(sql);
    }

    /// The reversal hook of `CallbackAction::RejectOrder` (src/bot/callbacks.rs)
    /// runs inside the reject's transaction: committed, the record is reversed
    /// and the order rejected together; rolled back, neither happened.
    #[tokio::test]
    #[ignore]
    async fn the_bot_reject_path_reverses_inside_its_transaction() {
        let Some(orm) = db().await else {
            eprintln!("DATABASE_URL unset — skipping");
            return;
        };
        let inviter = unique_id();
        let friend = unique_id();
        exec(
            &orm,
            "INSERT INTO referral_events (referrer_id, referred_id, code, status) \
             VALUES ($1, $2, 'TESTCODE', 'pending')",
            vec![inviter.into(), friend.into()],
        )
        .await;
        for _ in 0..2 {
            let order_id = uuid::Uuid::new_v4().to_string();
            exec(
                &orm,
                "INSERT INTO orders (id, telegram_id, items, status) VALUES ($1, $2, $3, 'completed')",
                vec![
                    order_id.clone().into(),
                    friend.into(),
                    serde_json::json!([{"quantity": 1.0, "bike": {"bike_key": "nmax-155",
                        "deal": {"kind": "bike_rental", "rental_start": "2026-09-27",
                                 "rental_end": "2026-09-28"}}}])
                    .into(),
                ],
            )
            .await;
            let body = RecordRentalBody {
                customer_telegram_id: friend,
                rental_amount_thb: 1239,
                order_id: Some(order_id.clone()),
                note: None,
                idempotency_key: uuid::Uuid::new_v4().to_string(),
            };
            let recorded = record_rental(&orm, 0, &[], &body).await.expect("record");
            assert_eq!(recorded.credit.as_ref().map(|c| c.credit_thb), Some(123));
            assert_eq!(balance_thb(&orm, inviter).await.expect("balance"), 123);

            // A reject that rolls back leaves the record and the order alone.
            {
                let tx = orm.begin().await.expect("begin");
                let n = reverse_for_a_rejected_order(&tx, &order_id, 42)
                    .await
                    .expect("reverse");
                assert_eq!(n, 1);
                tx.execute(stmt(
                    "UPDATE orders SET status = 'rejected' WHERE id = $1",
                    vec![order_id.clone().into()],
                ))
                .await
                .expect("flip");
                tx.rollback().await.expect("rollback");
            }
            assert_eq!(balance_thb(&orm, inviter).await.expect("balance"), 123);

            // The reject that commits reverses the record with the flip.
            let tx = orm.begin().await.expect("begin");
            let n = reverse_for_a_rejected_order(&tx, &order_id, 42)
                .await
                .expect("reverse");
            assert_eq!(n, 1);
            tx.execute(stmt(
                "UPDATE orders SET status = 'rejected' WHERE id = $1",
                vec![order_id.clone().into()],
            ))
            .await
            .expect("flip");
            tx.commit().await.expect("commit");
            assert_eq!(balance_thb(&orm, inviter).await.expect("balance"), 0);

            // A second reject click finds nothing live.
            let tx = orm.begin().await.expect("begin");
            let n = reverse_for_a_rejected_order(&tx, &order_id, 42)
                .await
                .expect("reverse");
            assert_eq!(n, 0);
            tx.commit().await.expect("commit");
            assert_eq!(balance_thb(&orm, inviter).await.expect("balance"), 0);
        }
        let reversed = orm
            .query_one(stmt(
                "SELECT COUNT(*)::bigint AS n FROM referral_rentals \
                 WHERE customer_telegram_id = $1 AND reversed_at IS NOT NULL AND reversed_by = 42",
                vec![friend.into()],
            ))
            .await
            .expect("count")
            .expect("a row");
        assert_eq!(reversed.try_get::<i64>("", "n").expect("n"), 2);
    }
}

#[cfg(test)]
mod recorder_tests {
    //! The recorder guard with no database (review of 2026-09-26): both
    //! identities `check_admin` answers with, and the gap that stays open.
    use super::recorder_is_the_inviter;

    #[test]
    fn the_recorder_guard_reads_both_admin_identities() {
        let admins = [42, 43];
        // An admin named by Telegram is refused only as the inviter himself.
        assert!(recorder_is_the_inviter(42, 42, &admins));
        assert!(!recorder_is_the_inviter(42, 43, &admins));
        assert!(!recorder_is_the_inviter(42, 7, &admins));
        // The password token (0) names nobody, so every listed admin's
        // friend is refused to it.
        assert!(recorder_is_the_inviter(0, 42, &admins));
        assert!(recorder_is_the_inviter(0, 43, &admins));
        // A friend of someone off the list is recorded: the case a password
        // holder off the list cannot be told apart from.
        assert!(!recorder_is_the_inviter(0, 7, &admins));
        assert!(!recorder_is_the_inviter(0, 42, &[]));
    }
}
