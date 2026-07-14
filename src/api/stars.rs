//! Stars (⭐) internal currency API.
//!
//! Game credits Stars; Telegram shop (Plot) spends them. Single server-side
//! balance keyed by telegram_id.

use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::api::auth::{check_not_blocked, check_owner, validate_telegram_id_param};
use crate::AppState;

pub(crate) fn routes() -> Router<AppState> {
    Router::new()
        .route("/stars/balance/:telegram_id", get(get_balance))
        .route("/stars/history/:telegram_id", get(get_history))
        .route("/stars/add", post(add_stars))
        .route("/stars/spend", post(spend_stars))
}

// ── Request / Response types ─────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub(crate) struct AddStarsRequest {
    pub telegram_id: i64,
    pub amount: i64,
    pub source: String,         // 'woodshop', 'garden', etc
    pub reason: String,         // 'level_complete', 'combo', etc
    pub external_tx_id: String, // game-generated UUID for idempotency
}

#[derive(Debug, Deserialize)]
pub(crate) struct SpendStarsRequest {
    pub telegram_id: i64,
    pub amount: i64,
    pub reason: String, // 'purchase', 'fee', etc
    pub related_order_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct BalanceQuery {
    // Reserved for future filters (currency_code, etc).
}

// ── Validation helpers ───────────────────────────────────────────────────────

const MAX_STARS_PER_TX: i64 = 1_000_000;

fn validate_amount(amount: i64) -> Result<(), StatusCode> {
    if amount <= 0 || amount > MAX_STARS_PER_TX {
        return Err(StatusCode::BAD_REQUEST);
    }
    Ok(())
}

fn validate_source_reason(source: &str, reason: &str) -> Result<(), StatusCode> {
    if source.is_empty() || source.len() > 50 || reason.is_empty() || reason.len() > 100 {
        return Err(StatusCode::BAD_REQUEST);
    }
    Ok(())
}

fn validate_external_tx_id(id: &str) -> Result<(), StatusCode> {
    if id.is_empty() || id.len() > 100 {
        return Err(StatusCode::BAD_REQUEST);
    }
    Ok(())
}

// ── Endpoints ────────────────────────────────────────────────────────────────

/// GetBalance(userId) — current Stars balance + short stats.
async fn get_balance(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(telegram_id): Path<i64>,
    Query(_q): Query<BalanceQuery>,
) -> Result<Json<Value>, StatusCode> {
    validate_telegram_id_param(telegram_id)?;
    check_owner(&headers, &state, telegram_id)?;
    check_not_blocked(&state, telegram_id).await?;

    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    let row = state
        .db
        .orm
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT COALESCE(balance, 0) AS balance FROM user_stars WHERE telegram_id = $1",
            [telegram_id.into()],
        ))
        .await
        .map_err(|e| {
            tracing::error!("stars get_balance: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let balance = row
        .map(|r| r.try_get::<i64>("", "balance").unwrap_or(0))
        .unwrap_or(0);

    Ok(Json(json!({ "telegram_id": telegram_id, "balance": balance })))
}

/// GetTransactionHistory(userId) — paginated append-only ledger.
async fn get_history(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(telegram_id): Path<i64>,
) -> Result<Json<Value>, StatusCode> {
    validate_telegram_id_param(telegram_id)?;
    check_owner(&headers, &state, telegram_id)?;
    check_not_blocked(&state, telegram_id).await?;

    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    let rows = state
        .db
        .orm
        .query_all(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT id, amount, balance_after, source, reason, external_tx_id, related_order_id, EXTRACT(EPOCH FROM created_at)::float8 AS created_at_epoch \
             FROM stars_transactions WHERE telegram_id = $1 ORDER BY created_at DESC LIMIT 500",
            [telegram_id.into()],
        ))
        .await
        .map_err(|e| {
            tracing::error!("stars get_history: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let txs: Vec<Value> = rows
        .iter()
        .map(|r| {
            json!({
                "id": r.try_get::<String>("", "id").unwrap_or_default(),
                "amount": r.try_get::<i64>("", "amount").unwrap_or(0),
                "balance_after": r.try_get::<i64>("", "balance_after").unwrap_or(0),
                "source": r.try_get::<String>("", "source").unwrap_or_default(),
                "reason": r.try_get::<String>("", "reason").unwrap_or_default(),
                "external_tx_id": r.try_get::<Option<String>>("", "external_tx_id").ok().flatten(),
                "related_order_id": r.try_get::<Option<String>>("", "related_order_id").ok().flatten(),
                "created_at_epoch": r.try_get::<f64>("", "created_at_epoch").unwrap_or(0.0),
            })
        })
        .collect();

    Ok(Json(json!({ "telegram_id": telegram_id, "transactions": txs })))
}

/// AddStars(userId, amount) — credits Stars from a game.
/// Authenticated by Telegram initData owner check so a user can only credit
/// themselves. Idempotency via external_tx_id prevents double-crediting.
async fn add_stars(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(req): Json<AddStarsRequest>,
) -> Result<Json<Value>, StatusCode> {
    validate_telegram_id_param(req.telegram_id)?;
    check_owner(&headers, &state, req.telegram_id)?;
    check_not_blocked(&state, req.telegram_id).await?;
    validate_amount(req.amount)?;
    validate_source_reason(&req.source, &req.reason)?;
    validate_external_tx_id(&req.external_tx_id)?;

    use crate::db::entities::{
        loyalty_profile::{ActiveModel as LpAm, Column as LpCol, Entity as LpEntity},
        stars_idempotency_key::{ActiveModel as IdemAm, Entity as IdemEntity},
        stars_transaction::{ActiveModel as TxAm, Entity as TxEntity},
        user_stars::{ActiveModel as UsAm, Column as UsCol, Entity as UsEntity},
    };
    use sea_orm::sea_query::OnConflict;
    use sea_orm::{
        ActiveValue::Set, ColumnTrait, ConnectionTrait, DbBackend, EntityTrait, QueryFilter,
        Statement, TransactionTrait,
    };

    let tx = state.db.orm.begin().await.map_err(|e| {
        tracing::error!("stars add tx.begin: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Idempotency: advisory-lock + check external_tx_id.
    tx.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "SELECT pg_advisory_xact_lock(hashtext($1)::bigint)",
        [req.external_tx_id.clone().into()],
    ))
    .await
    .map_err(|e| {
        tracing::error!("stars add idempotency lock: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    if IdemEntity::find_by_id(req.external_tx_id.clone())
        .one(&tx)
        .await
        .map_err(|e| {
            tracing::error!("stars add idempotency select: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .is_some()
    {
        tx.commit().await.ok();
        return Ok(Json(json!({
            "success": true,
            "idempotent_replay": true,
            "telegram_id": req.telegram_id,
            "amount": req.amount,
        })));
    }

    // Ensure loyalty_profile exists (FK target).
    let lp_am = LpAm {
        telegram_id: Set(req.telegram_id),
        bonus_balance: Set(Some(0.0)),
        total_spent: Set(Some(0.0)),
        ..Default::default()
    };
    LpEntity::insert(lp_am)
        .on_conflict(OnConflict::column(LpCol::TelegramId).do_nothing().to_owned())
        .do_nothing()
        .exec(&tx)
        .await
        .map_err(|e| {
            tracing::error!("stars add loyalty_profile upsert: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    // Upsert user_stars row; default balance 0 on conflict.
    let us_am = UsAm {
        telegram_id: Set(req.telegram_id),
        balance: Set(0),
        ..Default::default()
    };
    UsEntity::insert(us_am)
        .on_conflict(
            OnConflict::column(UsCol::TelegramId)
                .update_column(UsCol::UpdatedAt)
                .to_owned(),
        )
        .exec(&tx)
        .await
        .map_err(|e| {
            tracing::error!("stars add user_stars upsert: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    // Atomic balance bump and snapshot.
    let updated = UsEntity::update_many()
        .col_expr(
            UsCol::Balance,
            sea_orm::sea_query::Expr::cust_with_values("balance + $1", [req.amount]),
        )
        .filter(UsCol::TelegramId.eq(req.telegram_id))
        .exec(&tx)
        .await
        .map_err(|e| {
            tracing::error!("stars add balance bump: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    if updated.rows_affected == 0 {
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }

    // Read new balance for the ledger snapshot.
    let new_balance_row = UsEntity::find_by_id(req.telegram_id)
        .one(&tx)
        .await
        .map_err(|e| {
            tracing::error!("stars add balance read: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    let balance_after = new_balance_row.map(|r| r.balance).unwrap_or(req.amount);

    let tx_id = uuid::Uuid::new_v4().to_string();
    let tx_am = TxAm {
        id: Set(tx_id.clone()),
        telegram_id: Set(req.telegram_id),
        amount: Set(req.amount),
        balance_after: Set(balance_after),
        source: Set(req.source.clone()),
        reason: Set(req.reason.clone()),
        external_tx_id: Set(Some(req.external_tx_id.clone())),
        related_order_id: Set(None),
        ..Default::default()
    };
    TxEntity::insert(tx_am).exec(&tx).await.map_err(|e| {
        tracing::error!("stars add transaction insert: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let idem_am = IdemAm {
        external_tx_id: Set(req.external_tx_id.clone()),
        telegram_id: Set(req.telegram_id),
        amount: Set(req.amount),
        ..Default::default()
    };
    IdemEntity::insert(idem_am).exec(&tx).await.map_err(|e| {
        tracing::error!("stars add idempotency insert: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    tx.commit().await.map_err(|e| {
        tracing::error!("stars add commit: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(json!({
        "success": true,
        "tx_id": tx_id,
        "telegram_id": req.telegram_id,
        "amount": req.amount,
        "balance": balance_after,
    })))
}

/// SpendStars(userId, amount) — debits Stars (used by shop / Plot).
/// Requires owner auth via Telegram initData. Idempotency via X-Idempotency-Key.
async fn spend_stars(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(req): Json<SpendStarsRequest>,
) -> Result<Json<Value>, StatusCode> {
    validate_telegram_id_param(req.telegram_id)?;
    check_owner(&headers, &state, req.telegram_id)?;
    check_not_blocked(&state, req.telegram_id).await?;
    validate_amount(req.amount)?;
    if req.reason.is_empty() || req.reason.len() > 100 {
        return Err(StatusCode::BAD_REQUEST);
    }

    let idem_key: Option<String> = headers
        .get("x-idempotency-key")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    if let Some(ref k) = &idem_key {
        if !crate::api::orders::is_valid_idempotency_key(k) {
            return Err(StatusCode::BAD_REQUEST);
        }
    }

    use crate::db::entities::{
        order_idempotency_key::{ActiveModel as OrderIdemAm, Entity as OrderIdemEntity},
        stars_transaction::{ActiveModel as TxAm, Entity as TxEntity},
        user_stars::{ActiveModel as UsAm, Column as UsCol, Entity as UsEntity},
    };
    use sea_orm::sea_query::OnConflict;
    use sea_orm::{
        ActiveValue::Set, ColumnTrait, ConnectionTrait, DbBackend, EntityTrait, QueryFilter,
        Statement, TransactionTrait,
    };

    let tx = state.db.orm.begin().await.map_err(|e| {
        tracing::error!("stars spend tx.begin: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Idempotency via order_idempotency_keys (shared with orders).
    if let Some(ref k) = &idem_key {
        tx.execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT pg_advisory_xact_lock(hashtext($1)::bigint)",
            [k.clone().into()],
        ))
        .await
        .map_err(|e| {
            tracing::error!("stars spend idempotency lock: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

        if OrderIdemEntity::find_by_id(k.clone())
            .one(&tx)
            .await
            .map_err(|e| {
                tracing::error!("stars spend idempotency select: {:?}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?
            .is_some()
        {
            tx.commit().await.ok();
            return Ok(Json(json!({
                "success": true,
                "idempotent_replay": true,
                "telegram_id": req.telegram_id,
                "amount": req.amount,
            })));
        }
    }

    // Ensure row exists with 0 balance before trying to debit (better error).
    let us_am = UsAm {
        telegram_id: Set(req.telegram_id),
        balance: Set(0),
        ..Default::default()
    };
    UsEntity::insert(us_am)
        .on_conflict(
            OnConflict::column(UsCol::TelegramId)
                .update_column(UsCol::UpdatedAt)
                .to_owned(),
        )
        .exec(&tx)
        .await
        .map_err(|e| {
            tracing::error!("stars spend user_stars upsert: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    // Atomic debit with guard balance >= amount.
    let updated = UsEntity::update_many()
        .col_expr(
            UsCol::Balance,
            sea_orm::sea_query::Expr::cust_with_values("balance - $1", [req.amount]),
        )
        .filter(UsCol::TelegramId.eq(req.telegram_id))
        .filter(UsCol::Balance.gte(req.amount))
        .exec(&tx)
        .await
        .map_err(|e| {
            tracing::error!("stars spend debit: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    if updated.rows_affected == 0 {
        return Err(StatusCode::PAYMENT_REQUIRED); // Not enough Stars
    }

    let new_balance_row = UsEntity::find_by_id(req.telegram_id)
        .one(&tx)
        .await
        .map_err(|e| {
            tracing::error!("stars spend balance read: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    let balance_after = new_balance_row.map(|r| r.balance).unwrap_or(0);

    let tx_id = uuid::Uuid::new_v4().to_string();
    let tx_am = TxAm {
        id: Set(tx_id.clone()),
        telegram_id: Set(req.telegram_id),
        amount: Set(-req.amount),
        balance_after: Set(balance_after),
        source: Set("plot".to_string()),
        reason: Set(req.reason.clone()),
        external_tx_id: Set(None),
        related_order_id: Set(req.related_order_id.clone()),
        ..Default::default()
    };
    TxEntity::insert(tx_am).exec(&tx).await.map_err(|e| {
        tracing::error!("stars spend transaction insert: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    if let Some(ref k) = &idem_key {
        let idem_am = OrderIdemAm {
            key: Set(k.clone()),
            order_id: Set(format!("stars:{}", tx_id)),
            telegram_id: Set(Some(req.telegram_id)),
            ..Default::default()
        };
        OrderIdemEntity::insert(idem_am).exec(&tx).await.map_err(|e| {
            tracing::error!("stars spend idempotency insert: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    }

    tx.commit().await.map_err(|e| {
        tracing::error!("stars spend commit: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(json!({
        "success": true,
        "tx_id": tx_id,
        "telegram_id": req.telegram_id,
        "amount": req.amount,
        "balance": balance_after,
    })))
}
