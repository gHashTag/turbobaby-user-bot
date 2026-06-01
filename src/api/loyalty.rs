use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::api::auth::{check_admin, check_not_blocked, validate_telegram_id_param};
use crate::AppState;

#[derive(Debug, Deserialize)]
pub struct AddBonusRequest {
    pub amount: f64,
    pub tx_type: String,
    pub description: Option<String>,
    pub related_order_id: Option<String>,
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/loyalty/tiers", get(get_loyalty_tiers))
        .route("/loyalty/:telegram_id", get(get_profile))
        .route("/loyalty/:telegram_id/bonus", post(add_bonus))
        .route("/loyalty/:telegram_id/use-bonus", post(use_bonus))
        .route("/loyalty/leaderboard", get(get_leaderboard))
        .route("/loyalty/config", get(get_loyalty_config))
        .route("/loyalty/config", post(update_loyalty_config))
}

async fn get_loyalty_tiers(State(state): State<AppState>) -> Result<Json<Value>, StatusCode> {
    let client = state.db.pool.get().await.map_err(|e| {
        tracing::error!("DB error: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    let rows = client.query(
        "SELECT tier, name, min_points, discount_percent, points_multiplier::float8, perks, icon, color \
         FROM loyalty_tiers ORDER BY min_points ASC LIMIT 500",
        &[],
    ).await.map_err(|e| { tracing::error!("DB error: {:?}", e); StatusCode::INTERNAL_SERVER_ERROR })?;
    let tiers: Vec<Value> = rows
        .iter()
        .map(|r| {
            let pm = r.try_get::<_, f64>("points_multiplier").unwrap_or(0.0);
            let points_multiplier = if pm.is_finite() { pm.max(0.0) } else { 0.0 };
            json!({
                "tier":             r.try_get::<_, String>("tier").unwrap_or_default(),
                "name":             r.try_get::<_, String>("name").unwrap_or_default(),
                "min_points":       r.try_get::<_, i32>("min_points").unwrap_or(0),
                "discount_percent": r.try_get::<_, i32>("discount_percent").unwrap_or(0),
                "points_multiplier": points_multiplier,
                "perks":            r.try_get::<_, Vec<String>>("perks").unwrap_or_default(),
                "icon":             r.try_get::<_, String>("icon").unwrap_or_default(),
                "color":            r.try_get::<_, String>("color").unwrap_or_default(),
            })
        })
        .collect();
    Ok(Json(json!({ "tiers": tiers })))
}

async fn get_profile(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(telegram_id): Path<i64>,
) -> Result<Json<Value>, StatusCode> {
    validate_telegram_id_param(telegram_id)?;
    crate::api::auth::check_owner(&headers, &state, telegram_id)?;
    check_not_blocked(&state, telegram_id).await?;
    // SeaORM-версия: sqlx безопасно читает NUMERIC в f64.
    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    let stmt = Statement::from_sql_and_values(
        DbBackend::Postgres,
        "SELECT telegram_id, total_spent::float8 AS total_spent, bonus_balance::float8 AS bonus_balance, tier, referral_code, referred_by, referral_count, first_purchase_at, manager_telegram_id, is_blocked FROM loyalty_profiles WHERE telegram_id = $1",
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
            let profile = json!({
                "telegram_id": r.try_get::<i64>("", "telegram_id").unwrap_or(0),
                "total_spent": total_spent,
                "bonus_balance": bonus_balance,
                "tier": r.try_get::<String>("", "tier").unwrap_or_default(),
                "referral_code": r.try_get::<Option<String>>("", "referral_code").ok().flatten(),
                "referred_by": r.try_get::<Option<i64>>("", "referred_by").ok().flatten(),
                "referral_count": r.try_get::<i32>("", "referral_count").unwrap_or(0),
                "first_purchase_at": r.try_get::<Option<chrono::DateTime<chrono::Utc>>>("", "first_purchase_at").ok().flatten(),
                "manager_telegram_id": r.try_get::<Option<i64>>("", "manager_telegram_id").ok().flatten(),
                "is_blocked": r.try_get::<bool>("", "is_blocked").unwrap_or(false),
            });
            Ok(Json(json!({ "profile": profile })))
        }
        None => Err(StatusCode::NOT_FOUND),
    }
}

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
    if !req.amount.is_finite() || req.amount < 0.0 {
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
        loyalty_profile::{ActiveModel as LpAm, Column as LpCol, Entity as LoyaltyProfileEntity},
    };
    use sea_orm::sea_query::OnConflict;
    use sea_orm::{ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter, TransactionTrait};

    let tx = state.db.orm.begin().await.map_err(|e| {
        tracing::error!("add_bonus tx.begin error: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

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

    tx.commit().await.map_err(|e| {
        tracing::error!("add_bonus commit: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    Ok(Json(json!({ "success": true, "tx_id": tx_id })))
}

pub(crate) fn validate_use_bonus_amount(amount: f64) -> Result<(), StatusCode> {
    if !amount.is_finite() || amount <= 0.0 {
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
    use crate::db::entities::loyalty_profile::{Column as LpCol, Entity as LoyaltyProfileEntity};
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
    let result = LoyaltyProfileEntity::update_many()
        .col_expr(
            LpCol::BonusBalance,
            sea_orm::sea_query::Expr::cust_with_values("GREATEST(0, bonus_balance - $1)", [amount]),
        )
        .filter(LpCol::TelegramId.eq(telegram_id))
        .filter(LpCol::BonusBalance.gte(amount))
        .exec(&state.db.orm)
        .await
        .map_err(|e| {
            tracing::error!("use_bonus SeaORM error: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    if result.rows_affected == 0 {
        return Err(StatusCode::BAD_REQUEST);
    }
    Ok(Json(json!({ "success": true })))
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
    let client = state.db.pool.get().await.map_err(|e| {
        tracing::error!("DB error: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    let row = client
        .query_opt("SELECT config FROM loyalty_config WHERE id = 1", &[])
        .await
        .map_err(|e| {
            tracing::error!("DB error: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    match row {
        Some(r) => Ok(Json(
            json!({ "config": r.try_get::<_, Value>("config").unwrap_or(Value::Null) }),
        )),
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
    Ok(())
}

async fn update_loyalty_config(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
    check_admin(&headers, &state)?;
    validate_loyalty_config_body(&body)?;
    let client = state.db.pool.get().await.map_err(|e| {
        tracing::error!("DB error: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    client.execute(
        "INSERT INTO loyalty_config (id, config) VALUES (1, $1) ON CONFLICT (id) DO UPDATE SET config = $1",
        &[&body],
    ).await.map_err(|e| { tracing::error!("DB error: {:?}", e); StatusCode::INTERNAL_SERVER_ERROR })?;
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
}
