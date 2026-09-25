use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::api::auth::{check_admin, check_not_blocked, validate_telegram_id_param};
use crate::AppState;

#[derive(Debug, Deserialize)]
pub(crate) struct AddBonusRequest {
    pub amount: f64,
    pub tx_type: String,
    pub description: Option<String>,
    pub related_order_id: Option<String>,
}

pub(crate) fn routes() -> Router<AppState> {
    Router::new()
        .route("/loyalty/tiers", get(get_loyalty_tiers))
        .route("/loyalty/:telegram_id", get(get_profile))
        .route("/loyalty/:telegram_id/bonus", post(add_bonus))
        .route("/loyalty/:telegram_id/use-bonus", post(use_bonus))
        .route(
            "/loyalty/:telegram_id/bonus-history",
            get(get_bonus_history),
        )
        .route("/loyalty/leaderboard", get(get_leaderboard))
        .route("/loyalty/config", get(get_loyalty_config))
        .route("/loyalty/config", post(update_loyalty_config))
}

async fn get_loyalty_tiers(State(state): State<AppState>) -> Result<Json<Value>, StatusCode> {
    // Cycle #92: SeaORM via Statement. No `loyalty_tier` entity — single
    // call site, read-only, full-row shape inlined into JSON.
    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    let rows = state.db.orm.query_all(Statement::from_string(
        DbBackend::Postgres,
        "SELECT tier, name, min_points, discount_percent, points_multiplier::float8, icon, color \
         FROM loyalty_tiers ORDER BY min_points ASC LIMIT 500".to_string(),
    )).await.map_err(|e| { tracing::error!("loyalty_tiers: {e}"); StatusCode::INTERNAL_SERVER_ERROR })?;
    // Cycle #98: propagate `try_get` errors on the *financial* fields (min_points /
    // discount_percent / points_multiplier). Per TRY_GET_AUDIT, defaulting these to 0 on
    // schema drift would wipe all tier-based discounts — UI would show "every user gets 0%"
    // instead of failing loud. String/array fields keep default-on-missing since cosmetic
    // columns are NULL-able by intent. The filter and the empty `perks`: see `retired_row`.
    let tiers: Vec<Value> = rows
        .iter()
        .filter(|r| !retired_row(r))
        .map(|r| {
            let pm = r.try_get::<f64>("", "points_multiplier")?;
            let points_multiplier = if pm.is_finite() { pm.max(0.0) } else { 0.0 };
            Ok::<_, sea_orm::DbErr>(json!({
                "tier":             r.try_get::<String>("", "tier").unwrap_or_default(),
                "name":             r.try_get::<String>("", "name").unwrap_or_default(),
                "min_points":       r.try_get::<i32>("", "min_points")?,
                "discount_percent": r.try_get::<i32>("", "discount_percent")?,
                "points_multiplier": points_multiplier,
                "perks":            Vec::<String>::new(),
                "icon":             r.try_get::<String>("", "icon").unwrap_or_default(),
                "color":            r.try_get::<String>("", "color").unwrap_or_default(),
            }))
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| {
            tracing::error!("loyalty_tiers parse: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    Ok(Json(json!({ "tiers": tiers })))
}

// The standing loyalty policy and the reader for it live in
// `crate::trios::loyalty`. They were declared here first — and, for a few
// hours, only here, which is how the third and fourth copies in
// `db/orders.rs` and `api/orders.rs` went on disagreeing with them.
use crate::trios::loyalty::{config_f64, defaults as default_loyalty_config};

async fn get_profile(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(telegram_id): Path<i64>,
) -> Result<Json<Value>, StatusCode> {
    validate_telegram_id_param(telegram_id)?;
    crate::api::auth::check_owner(&headers, &state, telegram_id)?;
    check_not_blocked(&state, telegram_id).await?;
    // SeaORM-версия: sqlx безопасно читает NUMERIC в f64.
    use sea_orm::{ConnectionTrait, DbBackend, EntityTrait, Statement};

    // Loop #14: read the single loyalty_config row so the UI shows the same
    // thresholds/cashback/max-usage the backend uses to complete orders.
    let config: serde_json::Value = {
        use crate::db::entities::loyalty_config::Entity as LcEntity;
        LcEntity::find_by_id(1)
            .one(&state.db.orm)
            .await
            .map_err(|e| {
                tracing::error!("get_profile loyalty_config: {e}");
                StatusCode::INTERNAL_SERVER_ERROR
            })?
            .map(|m| m.config)
            .unwrap_or_else(default_loyalty_config)
    };

    let stmt = Statement::from_sql_and_values(
        DbBackend::Postgres,
        // `referral_count` counts friends whose order COMPLETED: it is written
        // only by `confirm_referral`, which only `complete_order` calls. The
        // profile screen was printing it under "Приглашено друзей", so somebody
        // who invited ten friends who all arrived and browsed saw zero. The
        // sentence and the number were about different things.
        //
        // `invited_count` is the number the label promises — everyone who
        // followed the link, whatever they did next. It is a subquery rather
        // than another denormalised column because a count that is stored is a
        // count that can drift from the rows it counts, which is the defect
        // being fixed.
        "SELECT lp.telegram_id, lp.total_spent::float8 AS total_spent, lp.bonus_balance::float8 AS bonus_balance, lp.tier, lp.referral_code, lp.referred_by, lp.referral_count, \
                (SELECT COUNT(*)::int4 FROM referral_events re WHERE re.referrer_id = lp.telegram_id) AS invited_count, \
                lp.first_purchase_at, lp.manager_telegram_id, lp.is_blocked, COUNT(o.id)::int4 AS orders_count FROM loyalty_profiles lp LEFT JOIN orders o ON o.telegram_id = lp.telegram_id WHERE lp.telegram_id = $1 GROUP BY lp.telegram_id",
        [telegram_id.into()],
    );
    let row = state.db.orm.query_one(stmt).await.map_err(|e| {
        tracing::error!("get_profile sea-orm: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    match row {
        Some(r) => {
            let total_spent = r
                .try_get::<Option<f64>>("", "total_spent")
                .ok()
                .flatten()
                .filter(|v| v.is_finite());
            let bonus_balance = r
                .try_get::<Option<f64>>("", "bonus_balance")
                .ok()
                .flatten()
                .filter(|v| v.is_finite())
                .unwrap_or(0.0)
                .max(0.0);
            let tier = r.try_get::<String>("", "tier").unwrap_or_default();
            let cashback_pct = crate::db::orders::cashback_pct_for_tier(&config, &tier);
            let max_bonus_usage_pct = crate::trios::loyalty::max_bonus_usage_pct(&config);
            // What the shop pays for a friend who orders. On the wire because
            // the profile screen prints it and must not guess — see
            // `default_loyalty_config`.
            let referral_bonus = config_f64(&config, "referral_bonus").max(0.0);

            let thresholds: Vec<(&str, f64)> = vec![
                ("bronze", config_f64(&config, "bronze_threshold")),
                ("silver", config_f64(&config, "silver_threshold")),
                ("gold", config_f64(&config, "gold_threshold")),
            ];
            let spent = total_spent.unwrap_or(0.0);
            let (next_tier, next_threshold) = thresholds
                .iter()
                .filter(|(_, t)| *t > spent)
                .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
                .map(|(name, t)| (*name, *t))
                .unwrap_or(("max", spent));

            let profile = json!({
                "telegram_id": r.try_get::<i64>("", "telegram_id").unwrap_or(0),
                "total_spent": total_spent,
                "bonus_balance": bonus_balance,
                "tier": tier,
                "referral_code": r.try_get::<Option<String>>("", "referral_code").ok().flatten(),
                "referred_by": r.try_get::<Option<i64>>("", "referred_by").ok().flatten(),
                "referral_count": r.try_get::<i32>("", "referral_count").unwrap_or(0),
                "invited_count": r.try_get::<i32>("", "invited_count").unwrap_or(0),
                "first_purchase_at": r.try_get::<Option<chrono::DateTime<chrono::Utc>>>("", "first_purchase_at").ok().flatten(),
                "manager_telegram_id": r.try_get::<Option<i64>>("", "manager_telegram_id").ok().flatten(),
                "is_blocked": r.try_get::<bool>("", "is_blocked").unwrap_or(false),
                "orders_count": r.try_get::<i32>("", "orders_count").unwrap_or(0),
            });
            Ok(Json(json!({
                "profile": profile,
                "config": {
                    "cashback_pct": cashback_pct,
                    "max_bonus_usage_pct": max_bonus_usage_pct,
                    "next_tier": next_tier,
                    "next_threshold": next_threshold,
                    "referral_bonus": referral_bonus,
                }
            })))
        }
        None => Err(StatusCode::NOT_FOUND),
    }
}

/// Cycle #151: upper bound for grant amount. Originally only `< 0`
/// was rejected, so an admin typo of "100000000" instead of "100"
/// would credit the user an absurd balance — irreversible without
/// a second admin issuing a counter-grant. 1_000_000 ฿ is the price
/// ceiling for any single product in the catalog (per
/// `validate_accessory_request` etc.), so capping single bonus grants
/// at the same number lets large legitimate refunds through while
/// catching the "extra zeros" class of mistake.
const ADD_BONUS_MAX_AMOUNT: f64 = 1_000_000.0;

fn validate_add_bonus_request(req: &AddBonusRequest) -> Result<(), StatusCode> {
    if req.tx_type.len() > 50 {
        return Err(StatusCode::BAD_REQUEST);
    }
    if let Some(ref d) = req.description {
        if d.len() > 1000 {
            return Err(StatusCode::BAD_REQUEST);
        }
    }
    if let Some(ref r) = req.related_order_id {
        if r.len() > 200 {
            return Err(StatusCode::BAD_REQUEST);
        }
    }
    if !req.amount.is_finite() || req.amount < 0.0 || req.amount > ADD_BONUS_MAX_AMOUNT {
        return Err(StatusCode::BAD_REQUEST);
    }
    Ok(())
}

async fn add_bonus(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(telegram_id): Path<i64>,
    Json(req): Json<AddBonusRequest>,
) -> Result<Json<Value>, StatusCode> {
    validate_telegram_id_param(telegram_id)?;
    check_admin(&headers, &state)?;
    validate_add_bonus_request(&req)?;

    // Cycle #159: X-Idempotency-Key (phase 2 of #157B; schema landed
    // #158). Optional — clients without the header keep working —
    // but when present, two POSTs with the same key produce one
    // bonus transaction and the second call returns the cached
    // tx_id. Mirrors `create_order`'s cycle-#57 implementation.
    let idem_key: Option<String> = headers
        .get("x-idempotency-key")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    if let Some(ref k) = idem_key {
        if !crate::api::orders::is_valid_idempotency_key(k) {
            return Err(StatusCode::BAD_REQUEST);
        }
    }

    let tx_id = uuid::Uuid::new_v4().to_string();
    // Cycle #83: full SeaORM transaction. Three statements that must
    // commit atomically:
    //   1. Ensure loyalty_profile exists (upsert no-op on conflict).
    //   2. Append to bonus_transactions ledger.
    //   3. Bump the balance.
    // `DatabaseTransaction` implements `ConnectionTrait` so the same
    // entity API works as on `DatabaseConnection`; pass `&tx` to every
    // `.exec(...)`. Errors before commit auto-rollback when `tx` drops,
    // so we don't write an explicit `.rollback()` arm — only commit on
    // the happy path.
    use crate::db::entities::{
        bonus_transaction::{ActiveModel as BonusTxAm, Entity as BonusTxEntity},
        loyalty_idempotency_key::{ActiveModel as LoyaltyIdemAm, Entity as LoyaltyIdemEntity},
        loyalty_profile::{ActiveModel as LpAm, Column as LpCol, Entity as LoyaltyProfileEntity},
    };
    use sea_orm::sea_query::OnConflict;
    use sea_orm::{
        ActiveValue::Set, ColumnTrait, ConnectionTrait, DbBackend, EntityTrait, QueryFilter,
        Statement, TransactionTrait,
    };

    let tx = state.db.orm.begin().await.map_err(|e| {
        tracing::error!("add_bonus tx.begin error: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Cycle #159: idempotency check. Advisory-lock the key, look it
    // up in `loyalty_idempotency_keys`. Existing row → return cached
    // tx_id; absent → fall through and INSERT the key at commit time.
    if let Some(ref k) = idem_key {
        tx.execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT pg_advisory_xact_lock(hashtext($1)::bigint)",
            [k.clone().into()],
        ))
        .await
        .map_err(|e| {
            tracing::error!("add_bonus idempotency lock: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
        let existing = LoyaltyIdemEntity::find_by_id(k.clone())
            .one(&tx)
            .await
            .map_err(|e| {
                tracing::error!("add_bonus idempotency SELECT: {:?}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
        if let Some(row) = existing {
            let cached_tx_id = row.tx_id;
            if let Err(e) = tx.commit().await {
                tracing::error!("add_bonus idempotency replay commit: {:?}", e);
            }
            tracing::info!(tx_id = %cached_tx_id, "add_bonus: idempotent replay");
            return Ok(Json(json!({
                "success": true,
                "tx_id": cached_tx_id,
                "idempotent_replay": true,
            })));
        }
    }

    // 1. Upsert loyalty_profile (no-op if it exists).
    let lp_am = LpAm {
        telegram_id: Set(telegram_id),
        bonus_balance: Set(Some(0.0)),
        total_spent: Set(Some(0.0)),
        ..Default::default()
    };
    LoyaltyProfileEntity::insert(lp_am)
        .on_conflict(
            OnConflict::column(LpCol::TelegramId)
                .do_nothing()
                .to_owned(),
        )
        .do_nothing()
        .exec(&tx)
        .await
        .map_err(|e| {
            tracing::error!("add_bonus loyalty_profile upsert: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    // 2. Append bonus_transactions ledger row.
    let bt_am = BonusTxAm {
        id: Set(tx_id.clone()),
        telegram_id: Set(telegram_id),
        amount: Set(req.amount),
        tx_type: Set(req.tx_type.clone()),
        description: Set(req.description.clone()),
        related_order_id: Set(req.related_order_id.clone()),
        ..Default::default()
    };
    BonusTxEntity::insert(bt_am).exec(&tx).await.map_err(|e| {
        tracing::error!("add_bonus bonus_transaction insert: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // 3. Bump bonus_balance with column-expr update (filter ensures the
    // row exists; step 1 made sure of that, so 0-row would be a serious
    // race).
    let updated = LoyaltyProfileEntity::update_many()
        .col_expr(
            LpCol::BonusBalance,
            sea_orm::sea_query::Expr::cust_with_values("bonus_balance + $1", [req.amount]),
        )
        .filter(LpCol::TelegramId.eq(telegram_id))
        .exec(&tx)
        .await
        .map_err(|e| {
            tracing::error!("add_bonus balance bump: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    if updated.rows_affected == 0 {
        tracing::error!(
            "add_bonus: loyalty profile missing for telegram_id={} after upsert (race?)",
            telegram_id
        );
        // tx drops → auto-rollback.
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }

    // Cycle #159: record the idempotency key inside the same tx so
    // retries after this commit replay the cached tx_id. The earlier
    // advisory lock guarantees no concurrent tx holds a different
    // (key, tx_id) pair.
    if let Some(ref k) = idem_key {
        let idem_am = LoyaltyIdemAm {
            key: Set(k.clone()),
            tx_id: Set(tx_id.clone()),
            telegram_id: Set(telegram_id),
            ..Default::default()
        };
        LoyaltyIdemEntity::insert(idem_am)
            .exec(&tx)
            .await
            .map_err(|e| {
                tracing::error!("add_bonus idempotency INSERT: {:?}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
    }

    tx.commit().await.map_err(|e| {
        tracing::error!("add_bonus commit: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    Ok(Json(json!({ "success": true, "tx_id": tx_id })))
}

pub(crate) fn validate_use_bonus_amount(amount: f64) -> Result<(), StatusCode> {
    // Cycle #151: same ceiling as add_bonus. Use-bonus is naturally
    // bounded by the customer's actual balance (the SQL `WHERE
    // bonus_balance >= $1` no-ops on insufficient funds), so this
    // upper bound is mainly defensive against an inflated `amount`
    // landing in audit logs or fraud detection heuristics with values
    // like 1e308 that would skew tier-recompute downstream.
    if !amount.is_finite() || amount <= 0.0 || amount > ADD_BONUS_MAX_AMOUNT {
        return Err(StatusCode::BAD_REQUEST);
    }
    Ok(())
}

async fn use_bonus(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(telegram_id): Path<i64>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
    validate_telegram_id_param(telegram_id)?;
    check_admin(&headers, &state)?;
    let amount = body["amount"].as_f64().unwrap_or(0.0);
    validate_use_bonus_amount(amount)?;

    // Cycle #160 (phase 2b of cycle #157B). Mirror of the cycle-#159
    // add_bonus idempotency wiring. Use-bonus has built-in SQL
    // atomicity via `WHERE bonus_balance >= $1` (concurrent retries
    // can't double-deduct — only one succeeds), but a network-blip
    // retry of an already-successful deduction returned 400 (not
    // 200), confusing clients about whether the original call took.
    // With X-Idempotency-Key, the retry now replays the original
    // 200 OK.
    let idem_key: Option<String> = headers
        .get("x-idempotency-key")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    if let Some(ref k) = idem_key {
        if !crate::api::orders::is_valid_idempotency_key(k) {
            return Err(StatusCode::BAD_REQUEST);
        }
    }
    // Synthetic tx_id — use_bonus doesn't write a ledger row today,
    // so this exists purely as a stable receipt for the caller and
    // the value cached in `loyalty_idempotency_keys.tx_id`. If a
    // future cycle adds a deduction ledger, this becomes the row id.
    let tx_id = uuid::Uuid::new_v4().to_string();

    // Cycle #83: SeaORM. The UPDATE has *two* clauses that need to
    // survive intact: the `GREATEST(0, bonus_balance - $1)` clamp and
    // the `WHERE bonus_balance >= $1` guard. The guard makes the whole
    // statement atomically idempotent — a concurrent caller that drained
    // the balance leaves us with `rows_affected == 0` and we return 400.
    //
    // SeaORM patterns used:
    //   - `Expr::cust_with_values("GREATEST(0, bonus_balance - $1)", [amount])`
    //     for the column update with a raw expr (no native clamp helper).
    //   - `.filter(Column.eq(...).and(Column.gte(...)))` for the
    //     concurrent-safe guard.
    use crate::db::entities::{
        loyalty_idempotency_key::{ActiveModel as LoyaltyIdemAm, Entity as LoyaltyIdemEntity},
        loyalty_profile::{Column as LpCol, Entity as LoyaltyProfileEntity},
    };
    use sea_orm::{
        ActiveValue::Set, ColumnTrait, ConnectionTrait, DbBackend, EntityTrait, QueryFilter,
        Statement, TransactionTrait,
    };

    let tx = state.db.orm.begin().await.map_err(|e| {
        tracing::error!("use_bonus tx.begin error: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Cycle #160: idempotency check — advisory-lock the key, look it
    // up. Existing row → return cached tx_id; absent → fall through.
    if let Some(ref k) = idem_key {
        tx.execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT pg_advisory_xact_lock(hashtext($1)::bigint)",
            [k.clone().into()],
        ))
        .await
        .map_err(|e| {
            tracing::error!("use_bonus idempotency lock: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
        let existing = LoyaltyIdemEntity::find_by_id(k.clone())
            .one(&tx)
            .await
            .map_err(|e| {
                tracing::error!("use_bonus idempotency SELECT: {:?}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
        if let Some(row) = existing {
            let cached_tx_id = row.tx_id;
            if let Err(e) = tx.commit().await {
                tracing::error!("use_bonus idempotency replay commit: {:?}", e);
            }
            tracing::info!(tx_id = %cached_tx_id, "use_bonus: idempotent replay");
            return Ok(Json(json!({
                "success": true,
                "tx_id": cached_tx_id,
                "idempotent_replay": true,
            })));
        }
    }

    let result = LoyaltyProfileEntity::update_many()
        .col_expr(
            LpCol::BonusBalance,
            sea_orm::sea_query::Expr::cust_with_values("GREATEST(0, bonus_balance - $1)", [amount]),
        )
        .filter(LpCol::TelegramId.eq(telegram_id))
        .filter(LpCol::BonusBalance.gte(amount))
        .exec(&tx)
        .await
        .map_err(|e| {
            tracing::error!("use_bonus SeaORM error: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    if result.rows_affected == 0 {
        // Insufficient funds (or concurrent retry that hit the WHERE
        // guard after another caller drained it). tx drops →
        // auto-rollback; no idempotency record written, so the
        // caller's next retry will see the same 400 — correct.
        return Err(StatusCode::BAD_REQUEST);
    }

    // Cycle #161: deduction ledger row. Pre-cycle the
    // `bonus_transactions` ledger was grant-only — `add_bonus` wrote
    // rows with positive amounts, but `use_bonus` just decremented
    // `loyalty_profiles.bonus_balance` with no audit trail. That
    // made the bookkeeping equation `SUM(amount) WHERE telegram_id=N
    // == bonus_balance` violate as soon as anyone spent bonus, and
    // fraud-investigation queries like "show me every credit/debit
    // for this user" could only see half the story.
    //
    // The cycle-#160 synthetic `tx_id` UUID was forward-designed for
    // exactly this — use it as the ledger row's id so the
    // idempotency-replay path and the audit log share the same
    // identifier. `amount` is stored negative (the entity's schema
    // doc-comment explicitly says "positive = credit, negative =
    // debit"); `tx_type = "admin_deduction"` distinguishes from
    // existing values like "referral_bonus" and from admin grants.
    use crate::db::entities::bonus_transaction::{
        ActiveModel as BonusTxAm, Entity as BonusTxEntity,
    };
    let bt_am = BonusTxAm {
        id: Set(tx_id.clone()),
        telegram_id: Set(telegram_id),
        amount: Set(-amount),
        tx_type: Set("admin_deduction".to_string()),
        description: Set(Some("use_bonus by admin".to_string())),
        related_order_id: Set(None),
        ..Default::default()
    };
    BonusTxEntity::insert(bt_am).exec(&tx).await.map_err(|e| {
        tracing::error!("use_bonus deduction ledger insert: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Cycle #160: record the idempotency key inside the same tx, so
    // retries after this commit replay the cached tx_id.
    if let Some(ref k) = idem_key {
        let idem_am = LoyaltyIdemAm {
            key: Set(k.clone()),
            tx_id: Set(tx_id.clone()),
            telegram_id: Set(telegram_id),
            ..Default::default()
        };
        LoyaltyIdemEntity::insert(idem_am)
            .exec(&tx)
            .await
            .map_err(|e| {
                tracing::error!("use_bonus idempotency INSERT: {:?}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
    }

    tx.commit().await.map_err(|e| {
        tracing::error!("use_bonus commit: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    Ok(Json(json!({ "success": true, "tx_id": tx_id })))
}

#[derive(Debug, Deserialize)]
pub(crate) struct BonusHistoryQuery {
    #[serde(default = "default_bonus_history_limit")]
    limit: i64,
    #[serde(default)]
    offset: i64,
}

fn default_bonus_history_limit() -> i64 {
    50
}

const BONUS_HISTORY_MAX_LIMIT: i64 = 100;

async fn get_bonus_history(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(telegram_id): Path<i64>,
    Query(query): Query<BonusHistoryQuery>,
) -> Result<Json<Value>, StatusCode> {
    validate_telegram_id_param(telegram_id)?;
    crate::api::auth::check_owner(&headers, &state, telegram_id)?;
    check_not_blocked(&state, telegram_id).await?;

    let limit = query.limit.clamp(1, BONUS_HISTORY_MAX_LIMIT);
    let offset = query.offset.max(0);

    use crate::db::entities::bonus_transaction::{Column as BtCol, Entity as BonusTxEntity};
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};

    let rows = BonusTxEntity::find()
        .filter(BtCol::TelegramId.eq(telegram_id))
        .order_by_desc(BtCol::CreatedAt)
        .limit(Some(limit as u64))
        .offset(Some(offset as u64))
        .all(&state.db.orm)
        .await
        .map_err(|e| {
            tracing::error!("get_bonus_history: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let transactions: Vec<Value> = rows
        .into_iter()
        .map(|m| {
            json!({
                "id": m.id,
                "amount": m.amount,
                "tx_type": m.tx_type,
                "description": m.description,
                "related_order_id": m.related_order_id,
                "created_at": m.created_at,
            })
        })
        .collect();

    Ok(Json(json!({ "transactions": transactions })))
}

async fn get_leaderboard(State(state): State<AppState>) -> Result<Json<Value>, StatusCode> {
    // SeaORM-версия: обходит все проблемы с NUMERIC ↔ f64,
    // потому что sqlx из коробки умеет читать numeric в f64.
    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    let stmt = Statement::from_string(
        DbBackend::Postgres,
        "SELECT lp.telegram_id, lp.total_spent::float8 AS total_spent, lp.tier, ul.first_name FROM loyalty_profiles lp LEFT JOIN user_languages ul ON lp.telegram_id = ul.telegram_id ORDER BY lp.total_spent DESC NULLS LAST LIMIT 20".to_string(),
    );
    let rows = state.db.orm.query_all(stmt).await.map_err(|e| {
        tracing::error!("get_leaderboard sea-orm: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    let leaderboard: Vec<Value> = rows
        .iter()
        .map(|r| {
            let total_spent = r
                .try_get::<Option<f64>>("", "total_spent")
                .ok()
                .flatten()
                .filter(|v| v.is_finite())
                .unwrap_or(0.0)
                .max(0.0);
            let tier: String = r.try_get::<String>("", "tier").unwrap_or_default();
            let first_name: Option<String> = r.try_get::<String>("", "first_name").ok();
            let display_name = first_name.unwrap_or_else(|| "Anonymous".to_string());
            json!({
                "first_name": display_name,
                "total_spent": total_spent,
                "tier": tier,
            })
        })
        .collect();
    Ok(Json(json!({ "leaderboard": leaderboard })))
}

async fn get_loyalty_config(State(state): State<AppState>) -> Result<Json<Value>, StatusCode> {
    // Cycle #92: SeaORM via the existing `loyalty_config` entity
    // (cycle #82). `find_by_id(1)` for the singleton row.
    use crate::db::entities::loyalty_config::Entity as LoyaltyConfigEntity;
    use sea_orm::EntityTrait;
    let model = LoyaltyConfigEntity::find_by_id(1)
        .one(&state.db.orm)
        .await
        .map_err(|e| {
            tracing::error!("get_loyalty_config: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    match model {
        Some(m) => Ok(Json(json!({ "config": m.config }))),
        None => Ok(Json(json!({ "config": null }))),
    }
}

fn validate_loyalty_config_body(body: &Value) -> Result<(), StatusCode> {
    let required = [
        "gold_threshold",
        "silver_threshold",
        "bronze_threshold",
        "referral_bonus",
    ];
    for key in required {
        if let Some(v) = body.get(key).and_then(|v| v.as_f64()) {
            if v < 0.0 || !v.is_finite() || v > 1_000_000_000.0 {
                return Err(StatusCode::BAD_REQUEST);
            }
        } else {
            return Err(StatusCode::BAD_REQUEST);
        }
    }

    // Cycle #153: tier-threshold ordering invariant. The
    // tier-recompute SQL in `complete_order_and_update_loyalty`
    // evaluates `CASE WHEN total_spent >= gold THEN 'gold' WHEN
    // total_spent >= silver THEN 'silver' WHEN total_spent >= bronze
    // THEN 'bronze' ELSE 'none' END` top-down, so the thresholds
    // MUST satisfy `gold > silver > bronze`. Without this guard, an
    // admin typo `gold=100, silver=1000` would give the 'gold' tier
    // to every user who spent ≥100 ฿ — accidental mass tier
    // inflation that's silently irreversible (the SQL just keeps
    // running with the broken ordering until somebody fixes it).
    //
    // Allow equality between bronze and 0 — a config with bronze=0
    // means "any spend qualifies for bronze," a legitimate setting.
    // Equality between tiers is rejected: it implies an empty band
    // (e.g. silver==gold means nobody can ever be silver).
    let gold = body["gold_threshold"].as_f64().unwrap_or(0.0);
    let silver = body["silver_threshold"].as_f64().unwrap_or(0.0);
    let bronze = body["bronze_threshold"].as_f64().unwrap_or(0.0);
    if !(gold > silver && silver > bronze) {
        return Err(StatusCode::BAD_REQUEST);
    }
    Ok(())
}

async fn update_loyalty_config(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
    check_admin(&headers, &state)?;
    validate_loyalty_config_body(&body)?;
    // Cycle #92: SeaORM upsert via the `loyalty_config` entity. Pattern
    // #8 (`OnConflict::column.update_columns`).
    use crate::db::entities::loyalty_config::{
        ActiveModel as LcAm, Column as LcCol, Entity as LcEntity,
    };
    use sea_orm::sea_query::OnConflict;
    use sea_orm::{ActiveValue::Set, EntityTrait};
    let am = LcAm {
        id: Set(1),
        config: Set(body),
        // Cycle #136: leave marketing_badges_hidden unset on the
        // loyalty-config PUT path — it has its own dedicated
        // endpoint (PUT /api/admin/marketing-display).
        ..Default::default()
    };
    LcEntity::insert(am)
        .on_conflict(
            OnConflict::column(LcCol::Id)
                .update_column(LcCol::Config)
                .to_owned(),
        )
        .exec(&state.db.orm)
        .await
        .map_err(|e| {
            tracing::error!("update_loyalty_config: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    Ok(Json(json!({ "success": true })))
}

#[cfg(test)]
mod tests {
    use super::{
        validate_add_bonus_request, validate_loyalty_config_body, validate_use_bonus_amount,
        AddBonusRequest,
    };
    use axum::http::StatusCode;
    use serde_json::json;

    fn valid_bonus_req() -> AddBonusRequest {
        AddBonusRequest {
            amount: 10.0,
            tx_type: "manual".into(),
            description: Some("test".into()),
            related_order_id: None,
        }
    }

    #[test]
    fn test_validate_add_bonus_ok() {
        assert!(validate_add_bonus_request(&valid_bonus_req()).is_ok());
    }

    #[test]
    fn test_validate_add_bonus_tx_type_too_long() {
        let mut req = valid_bonus_req();
        req.tx_type = "a".repeat(51);
        assert_eq!(
            validate_add_bonus_request(&req).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn test_validate_add_bonus_description_too_long() {
        let mut req = valid_bonus_req();
        req.description = Some("a".repeat(1001));
        assert_eq!(
            validate_add_bonus_request(&req).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn test_validate_add_bonus_related_order_id_too_long() {
        let mut req = valid_bonus_req();
        req.related_order_id = Some("a".repeat(201));
        assert_eq!(
            validate_add_bonus_request(&req).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn test_validate_add_bonus_amount_negative() {
        let mut req = valid_bonus_req();
        req.amount = -1.0;
        assert_eq!(
            validate_add_bonus_request(&req).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn test_validate_add_bonus_amount_nan() {
        let mut req = valid_bonus_req();
        req.amount = f64::NAN;
        assert_eq!(
            validate_add_bonus_request(&req).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    // Cycle #151: upper bound. The "extra zeros" typo case.
    #[test]
    fn test_validate_add_bonus_amount_too_large() {
        let mut req = valid_bonus_req();
        req.amount = 1_000_000.01;
        assert_eq!(
            validate_add_bonus_request(&req).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn test_validate_add_bonus_amount_at_max_ok() {
        // The boundary is inclusive — exactly 1_000_000 ฿ allowed
        // because that's the same ceiling as a single catalog item.
        let mut req = valid_bonus_req();
        req.amount = 1_000_000.0;
        assert!(validate_add_bonus_request(&req).is_ok());
    }

    #[test]
    fn test_validate_add_bonus_amount_infinity() {
        let mut req = valid_bonus_req();
        req.amount = f64::INFINITY;
        assert_eq!(
            validate_add_bonus_request(&req).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn test_validate_loyalty_config_ok() {
        let body = json!({
            "gold_threshold": 1000.0,
            "silver_threshold": 500.0,
            "bronze_threshold": 100.0,
            "referral_bonus": 50.0
        });
        assert!(validate_loyalty_config_body(&body).is_ok());
    }

    #[test]
    fn test_validate_loyalty_config_missing_field() {
        let body = json!({"gold_threshold": 100.0});
        assert_eq!(
            validate_loyalty_config_body(&body).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn test_validate_loyalty_config_negative_value() {
        let body = json!({
            "gold_threshold": -1.0,
            "silver_threshold": 500.0,
            "bronze_threshold": 100.0,
            "referral_bonus": 50.0
        });
        assert_eq!(
            validate_loyalty_config_body(&body).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    // Cycle #153: tier ordering invariant tests. gold > silver > bronze
    // must hold or the CASE-WHEN tier-recompute SQL assigns wrong tiers.

    #[test]
    fn test_validate_loyalty_config_rejects_silver_above_gold() {
        // The motivator: admin types thresholds in the wrong order.
        // Pre-cycle-#153 this passed, then every user >= 100 ฿ got 'gold'.
        let body = json!({
            "gold_threshold": 100.0,
            "silver_threshold": 1000.0,
            "bronze_threshold": 50.0,
            "referral_bonus": 50.0
        });
        assert_eq!(
            validate_loyalty_config_body(&body).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn test_validate_loyalty_config_rejects_bronze_above_silver() {
        let body = json!({
            "gold_threshold": 1000.0,
            "silver_threshold": 100.0,
            "bronze_threshold": 500.0,
            "referral_bonus": 50.0
        });
        assert_eq!(
            validate_loyalty_config_body(&body).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn test_validate_loyalty_config_rejects_silver_equals_gold() {
        // Equal bands collapse — nobody can ever be in the lower tier.
        let body = json!({
            "gold_threshold": 500.0,
            "silver_threshold": 500.0,
            "bronze_threshold": 100.0,
            "referral_bonus": 50.0
        });
        assert_eq!(
            validate_loyalty_config_body(&body).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn test_validate_loyalty_config_rejects_bronze_equals_silver() {
        let body = json!({
            "gold_threshold": 1000.0,
            "silver_threshold": 100.0,
            "bronze_threshold": 100.0,
            "referral_bonus": 50.0
        });
        assert_eq!(
            validate_loyalty_config_body(&body).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn test_validate_loyalty_config_accepts_bronze_zero() {
        // bronze=0 is a legitimate "anyone qualifies for bronze" setting.
        let body = json!({
            "gold_threshold": 1000.0,
            "silver_threshold": 500.0,
            "bronze_threshold": 0.0,
            "referral_bonus": 50.0
        });
        assert!(validate_loyalty_config_body(&body).is_ok());
    }

    #[test]
    fn test_validate_loyalty_config_nan_value() {
        let body = json!({
            "gold_threshold": f64::NAN,
            "silver_threshold": 500.0,
            "bronze_threshold": 100.0,
            "referral_bonus": 50.0
        });
        assert_eq!(
            validate_loyalty_config_body(&body).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn test_validate_loyalty_config_too_high() {
        let body = json!({
            "gold_threshold": 2_000_000_000.0,
            "silver_threshold": 500.0,
            "bronze_threshold": 100.0,
            "referral_bonus": 50.0
        });
        assert_eq!(
            validate_loyalty_config_body(&body).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn test_validate_use_bonus_amount_ok() {
        assert!(validate_use_bonus_amount(10.0).is_ok());
    }

    #[test]
    fn test_validate_use_bonus_amount_zero() {
        assert_eq!(
            validate_use_bonus_amount(0.0).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn test_validate_use_bonus_amount_negative() {
        assert_eq!(
            validate_use_bonus_amount(-1.0).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn test_validate_use_bonus_amount_nan() {
        assert_eq!(
            validate_use_bonus_amount(f64::NAN).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    // Cycle #151: upper bound for use-bonus too.
    #[test]
    fn test_validate_use_bonus_amount_too_large() {
        assert_eq!(
            validate_use_bonus_amount(1_000_000.01).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn test_validate_use_bonus_amount_at_max_ok() {
        assert!(validate_use_bonus_amount(1_000_000.0).is_ok());
    }

    #[test]
    fn test_validate_use_bonus_amount_infinity() {
        assert_eq!(
            validate_use_bonus_amount(f64::INFINITY).unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }
}

/// What `GET /api/loyalty/tiers` stopped serving on 2026-09-25, when the owner
/// ruled that nothing cannabis-related may appear anywhere. Two things, both
/// stopped here in code; no row is written or deleted:
///
/// * every `perks` list is served empty, and the column is no longer read.
///   The stored perks are the old shop's promises (migration 009 seeded them:
///   its catalogue, its founder, its events) and no TurboBaby perk copy has
///   been approved to stand in their place. The key stays, so a reader of the
///   old shape still parses.
/// * the tier named after the old shop is not served: this function, over
///   `retired_tier`. Nothing assigns that tier: the recompute in `db::orders`
///   only ever writes gold, silver, bronze or none.
///
/// Kept at the end of the file so that no line the contracts cite above moves.
fn retired_row(r: &sea_orm::QueryResult) -> bool {
    let tier = r.try_get::<String>("", "tier").unwrap_or_default();
    let name = r.try_get::<String>("", "name").unwrap_or_default();
    retired_tier(&tier, &name)
}

/// Whether a `loyalty_tiers` row is the tier named after the old shop, which
/// `GET /api/loyalty/tiers` no longer serves (owner, 2026-09-25: nothing
/// cannabis-related anywhere).
///
/// Read from the row rather than from a list of seeded keys, so a renamed or
/// re-keyed copy of the same tier in production is caught too: the key or the
/// display name carrying the old shop's name, in any case.
fn retired_tier(tier: &str, name: &str) -> bool {
    const OLD_SHOP_NAME: &str = "woody";
    tier.to_lowercase().contains(OLD_SHOP_NAME) || name.to_lowercase().contains(OLD_SHOP_NAME)
}

#[cfg(test)]
mod retired_tier_tests {
    use super::retired_tier;

    #[test]
    fn the_old_shops_tier_is_retired_by_key_or_by_name() {
        // The row migration 009 seeds, and the two ways a copy of it could
        // differ in production.
        assert!(retired_tier("woody", "Woody Elite"));
        assert!(retired_tier("WOODY", "Elite"));
        assert!(retired_tier("elite", "Woody Elite"));
    }

    #[test]
    fn every_other_seeded_tier_is_served() {
        for (tier, name) in [
            ("bronze", "Бронза"),
            ("silver", "Серебро"),
            ("gold", "Золото"),
            ("platinum", "Платина"),
            ("diamond", "Бриллиант"),
        ] {
            assert!(!retired_tier(tier, name), "{tier} must still be served");
        }
    }
}
