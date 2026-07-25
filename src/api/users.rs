use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
    Json, Router,
};
use chrono::{Datelike, NaiveDate};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::api::auth::{check_owner, validate_telegram_id_param};
use crate::db::entities::loyalty_profile::{
    ActiveModel as LpAm, Column as LpCol, Entity as LoyaltyProfileEntity,
};
use crate::AppState;
use sea_orm::{ActiveValue::Set, EntityTrait};

pub(crate) fn routes() -> Router<AppState> {
    Router::new()
        .route("/users/me/:telegram_id/verify-age", post(verify_age))
        .route("/users/me/:telegram_id", get(get_my_profile))
}

/// Thailand cannabis law: buyer must be at least 20 years old.
const MIN_AGE_YEARS: i32 = 20;

#[derive(Debug, Deserialize)]
pub(crate) struct VerifyAgeRequest {
    pub dob: String,
}

/// Parse an ISO date (YYYY-MM-DD) and validate it is a plausible DOB.
fn parse_dob(raw: &str) -> Result<NaiveDate, StatusCode> {
    if raw.len() > 20 {
        return Err(StatusCode::BAD_REQUEST);
    }
    let date = NaiveDate::parse_from_str(raw.trim(), "%Y-%m-%d").map_err(|e| {
        tracing::debug!("parse_dob failed: {}", e);
        StatusCode::BAD_REQUEST
    })?;
    // Sanity bounds: not in the future, not older than 120 years.
    let today = chrono::Local::now().naive_local().date();
    if date > today {
        return Err(StatusCode::BAD_REQUEST);
    }
    if today.year() - date.year() > 120 {
        return Err(StatusCode::BAD_REQUEST);
    }
    Ok(date)
}

/// Compute whether `dob` meets the minimum age as of today.
fn is_age_verified(dob: NaiveDate) -> bool {
    let today = chrono::Local::now().naive_local().date();
    let years = today.year() - dob.year();
    // Adjust if birthday hasn't occurred yet this calendar year.
    let had_birthday = (today.month(), today.day()) >= (dob.month(), dob.day());
    (if had_birthday { years } else { years - 1 }) >= MIN_AGE_YEARS
}

/// POST /api/users/me/:telegram_id/verify-age
/// Owner-only: records DOB and server-computed verification outcome.
async fn verify_age(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(telegram_id): Path<i64>,
    Json(req): Json<VerifyAgeRequest>,
) -> Result<Json<Value>, StatusCode> {
    validate_telegram_id_param(telegram_id)?;
    check_owner(&headers, &state, telegram_id)?;

    let dob = parse_dob(&req.dob)?;
    let verified = is_age_verified(dob);

    let update = LpAm {
        telegram_id: Set(telegram_id),
        date_of_birth: Set(Some(dob)),
        age_verified: Set(verified),
        ..Default::default()
    };
    LoyaltyProfileEntity::insert(update)
        .on_conflict(
            sea_orm::sea_query::OnConflict::column(LpCol::TelegramId)
                .update_columns([LpCol::DateOfBirth, LpCol::AgeVerified])
                .to_owned(),
        )
        .exec(&state.db.orm)
        .await
        .map_err(|e| {
            tracing::error!("verify_age upsert failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(json!({
        "age_verified": verified,
        "min_age_years": MIN_AGE_YEARS,
        "date_of_birth": dob.to_string(),
    })))
}

/// GET /api/users/me/:telegram_id
/// Owner-only: returns profile including age verification status.
async fn get_my_profile(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(telegram_id): Path<i64>,
) -> Result<Json<Value>, StatusCode> {
    validate_telegram_id_param(telegram_id)?;
    check_owner(&headers, &state, telegram_id)?;

    let profile = LoyaltyProfileEntity::find_by_id(telegram_id)
        .one(&state.db.orm)
        .await
        .map_err(|e| {
            tracing::error!("get_my_profile read failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let (verified, dob) = match profile {
        Some(p) => (p.age_verified, p.date_of_birth.map(|d| d.to_string())),
        None => (false, None),
    };

    Ok(Json(json!({
        "telegram_id": telegram_id,
        "age_verified": verified,
        "date_of_birth": dob,
        "min_age_years": MIN_AGE_YEARS,
    })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_age_verified_exactly_20_today() {
        let today = chrono::Local::now().naive_local().date();
        let dob = today.with_year(today.year() - 20).unwrap();
        assert!(is_age_verified(dob));
    }

    #[test]
    fn test_is_age_verified_one_day_short() {
        let today = chrono::Local::now().naive_local().date();
        // Birthday is tomorrow
        let dob = (today + chrono::Duration::days(1))
            .with_year(today.year() - 20)
            .unwrap();
        assert!(!is_age_verified(dob));
    }

    #[test]
    fn test_is_age_verified_underage() {
        let today = chrono::Local::now().naive_local().date();
        let dob = today.with_year(today.year() - 19).unwrap();
        assert!(!is_age_verified(dob));
    }

    #[test]
    fn test_parse_dob_valid() {
        assert_eq!(
            parse_dob("1990-05-17").unwrap(),
            NaiveDate::from_ymd_opt(1990, 5, 17).unwrap()
        );
    }

    #[test]
    fn test_parse_dob_invalid_format() {
        assert_eq!(
            parse_dob("17-05-1990").unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn test_parse_dob_future() {
        let future = (chrono::Local::now().naive_local().date() + chrono::Duration::days(1))
            .format("%Y-%m-%d")
            .to_string();
        assert_eq!(parse_dob(&future).unwrap_err(), StatusCode::BAD_REQUEST);
    }
}
