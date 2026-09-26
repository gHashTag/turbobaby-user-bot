//! The referral credit's six routes (owner, 2026-09-26, R3).
//!
//! The owner's words and the arithmetic live in `crate::trios::referral_credit`;
//! the ledger lives in `crate::db::referral_credit`. This module is the door:
//! the gates, the validation of what a caller sends, and the answer each
//! refusal gets.
//!
//! * Two customer routes, each behind `validate_telegram_id_param`, the owner
//!   check and the blocked check, in that order and before any database read:
//!   the balance (`GET /api/referral-credit/me/:telegram_id`), and a request
//!   «Списать в счёт аренды» (`redeem`) or «Запросить выплату» (`payout`)
//!   (`POST /api/referral-credit/me/:telegram_id/requests`). A request holds
//!   the balance and moves no money; the admins are told of a new one.
//! * Four admin routes, each behind `check_admin`: the overview, a recorded
//!   rental (the one act that creates referral money), a whole reversal of a
//!   record, and a request resolved as paid (by hand) or declined.
//!
//! A customer's answer names no friend, no order and no ledger entry. Nothing
//! here moves money: a payout is made by a manager by hand and recorded.
//! `specs/turbobaby/referral_credit.t27` records the routes and their gates.

use axum::{
    body::Bytes,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde_json::Value;

use crate::api::auth::{check_admin, check_not_blocked, check_owner, validate_telegram_id_param};
use crate::db::referral_credit::{CreditError, Refusal};
use crate::trios::referral_credit::{
    is_request_kind, rental_amount_is_valid, ApiError, RecordRentalBody, ReferralCredit,
    ReferralRequest, NOTE_MAX_CHARS,
};
use crate::AppState;

/// The longest order id a record may link to (`orders.id` is `VARCHAR(36)`).
const ORDER_ID_MAX_CHARS: usize = 36;

pub(crate) fn routes() -> Router<AppState> {
    Router::new()
        .route("/referral-credit/me/:telegram_id", get(get_my_credit))
        .route(
            "/referral-credit/me/:telegram_id/requests",
            post(post_my_request),
        )
        .route("/admin/referral-credit/overview", get(get_overview))
        .route("/admin/referral-credit/rentals", post(post_rental))
        .route(
            "/admin/referral-credit/rentals/:id/reverse",
            post(post_reversal),
        )
        .route(
            "/admin/referral-credit/requests/:id/resolve",
            post(post_resolution),
        )
}

// ── Answers ─────────────────────────────────────────────────────────────────

fn plain(status: StatusCode) -> Response {
    status.into_response()
}

fn refused(status: StatusCode, error: &str) -> Response {
    error_body(status, error, None, None, None)
}

fn error_body(
    status: StatusCode,
    error: &str,
    reason: Option<&str>,
    detail: Option<String>,
    open_request: Option<ReferralRequest>,
) -> Response {
    let body = ApiError {
        error: error.to_string(),
        reason: reason.map(str::to_string),
        status: detail,
        open_request,
    };
    (status, Json(body)).into_response()
}

/// The status and body each refusal of the ledger is answered with.
fn refusal_response(refusal: Refusal) -> Response {
    use Refusal::*;
    let conflict = StatusCode::CONFLICT;
    let unprocessable = StatusCode::UNPROCESSABLE_ENTITY;
    let missing = StatusCode::NOT_FOUND;
    match refusal {
        OrderNotFound => refused(missing, "order_not_found"),
        OrderOfAnotherCustomer => refused(unprocessable, "order_of_another_customer"),
        OrderHasNoRentalLine => refused(unprocessable, "order_has_no_rental_line"),
        OrderNotCompleted => refused(conflict, "order_not_completed"),
        OrderBeforeProgram => refused(conflict, "order_before_program"),
        OrderAlreadyRecorded => refused(conflict, "order_already_recorded"),
        IdempotencyKeyReused => refused(conflict, "idempotency_key_reused"),
        RecorderIsInviter => refused(unprocessable, "recorder_is_inviter"),
        NoReferralEffect(reason) => error_body(
            unprocessable,
            "no_referral_effect",
            Some(reason.code()),
            None,
            None,
        ),
        RequestOpen(open) => error_body(conflict, "request_open", None, None, Some(open)),
        NothingAvailable => refused(unprocessable, "nothing_available"),
        RentalNotFound => refused(missing, "rental_not_found"),
        RequestNotFound => refused(missing, "request_not_found"),
        RequestNotOpen(status) => {
            error_body(conflict, "request_not_open", None, Some(status), None)
        }
        RedeemCannotBePaid => refused(unprocessable, "redeem_cannot_be_paid"),
        BalanceBelowRequest => refused(conflict, "balance_below_request"),
    }
}

fn credit_error(what: &str, error: CreditError) -> Response {
    match error {
        CreditError::Refused(refusal) => refusal_response(refusal),
        CreditError::Db(e) => {
            tracing::error!("referral credit {what}: {e:#}");
            plain(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

// ── What a caller may send ─────────────────────────────────────────────────

fn json_of(body: &[u8]) -> Option<Value> {
    if body.iter().all(u8::is_ascii_whitespace) {
        return Some(Value::Object(serde_json::Map::new()));
    }
    serde_json::from_slice(body).ok()
}

/// An optional text field: absent, `null` or blank is `None`; a string longer
/// than `max_chars` characters, or anything that is not a string, is refused.
fn optional_text(value: Option<&Value>, max_chars: usize) -> Result<Option<String>, ()> {
    match value {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(text)) => {
            let text = text.trim();
            if text.is_empty() {
                Ok(None)
            } else if text.chars().count() > max_chars {
                Err(())
            } else {
                Ok(Some(text.to_string()))
            }
        }
        Some(_) => Err(()),
    }
}

/// `POST /api/admin/referral-credit/rentals`'s body, field by field, each
/// refusal named for the first field that fails.
fn parse_record_body(body: &[u8]) -> Result<RecordRentalBody, &'static str> {
    let value = json_of(body).ok_or("invalid_customer")?;
    let customer_telegram_id = value
        .get("customer_telegram_id")
        .and_then(Value::as_i64)
        .filter(|id| validate_telegram_id_param(*id).is_ok())
        .ok_or("invalid_customer")?;
    let rental_amount_thb = value
        .get("rental_amount_thb")
        .and_then(Value::as_i64)
        .filter(|thb| rental_amount_is_valid(*thb))
        .ok_or("invalid_rental_amount")?;
    let order_id = optional_text(value.get("order_id"), ORDER_ID_MAX_CHARS)
        .map_err(|()| "invalid_order_id")?;
    let note = optional_text(value.get("note"), NOTE_MAX_CHARS).map_err(|()| "invalid_note")?;
    let idempotency_key = value
        .get("idempotency_key")
        .and_then(Value::as_str)
        .filter(|key| crate::api::orders::is_valid_idempotency_key(key))
        .ok_or("invalid_idempotency_key")?
        .to_string();
    Ok(RecordRentalBody {
        customer_telegram_id,
        rental_amount_thb,
        order_id,
        note,
        idempotency_key,
    })
}

/// `POST .../rentals/:id/reverse`'s body: an optional note.
fn parse_reverse_body(body: &[u8]) -> Result<Option<String>, &'static str> {
    let value = json_of(body).ok_or("invalid_note")?;
    optional_text(value.get("note"), NOTE_MAX_CHARS).map_err(|()| "invalid_note")
}

/// `POST .../requests/:id/resolve`'s body: `paid` or `declined`, and a note.
fn parse_resolve_body(body: &[u8]) -> Result<(&'static str, Option<String>), &'static str> {
    let value = json_of(body).ok_or("invalid_action")?;
    let action = match value.get("action").and_then(Value::as_str) {
        Some("paid") => "paid",
        Some("declined") => "declined",
        _ => return Err("invalid_action"),
    };
    let note = optional_text(value.get("note"), NOTE_MAX_CHARS).map_err(|()| "invalid_note")?;
    Ok((action, note))
}

/// `POST /api/referral-credit/me/:telegram_id/requests`'s body: the kind.
fn parse_request_kind(body: &[u8]) -> Option<String> {
    json_of(body)?
        .get("kind")
        .and_then(Value::as_str)
        .filter(|kind| is_request_kind(kind))
        .map(str::to_string)
}

/// The admins' notice of a new request. Admin-facing English written by the
/// lane (listed in DECISIONS.md for rewording); every user field is escaped.
fn request_notice(
    kind: &str,
    telegram_id: i64,
    first_name: Option<&str>,
    username: Option<&str>,
    amount_thb: i64,
    request_id: i64,
) -> String {
    use crate::util::html_escape;
    let head = if kind == "payout" {
        "💸 <b>Referral payout request</b>"
    } else {
        "🏍 <b>Referral balance to apply to a rental</b>"
    };
    let mut who = Vec::new();
    if let Some(name) = first_name {
        who.push(html_escape(name));
    }
    if let Some(handle) = username {
        who.push(format!("@{}", html_escape(handle)));
    }
    who.push(format!("id {telegram_id}"));
    format!(
        "{head}\n{}\n{}\n#R{request_id}\nAdmin → Лояльность → Рефералы",
        who.join(" "),
        crate::trios::pricing::format_baht(amount_thb as f64),
    )
}

// ── Customer routes ────────────────────────────────────────────────────────

/// `GET /api/referral-credit/me/:telegram_id`: the balance, the hold and what
/// is available, and the one open request. Errors carry an empty body.
async fn get_my_credit(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(telegram_id): Path<i64>,
) -> Result<Json<ReferralCredit>, StatusCode> {
    validate_telegram_id_param(telegram_id)?;
    check_owner(&headers, &state, telegram_id)?;
    check_not_blocked(&state, telegram_id).await?;
    crate::db::referral_credit::credit_summary(&state.db.orm, telegram_id)
        .await
        .map(Json)
        .map_err(|e| {
            tracing::error!("referral credit summary telegram_id={telegram_id}: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })
}

/// `POST /api/referral-credit/me/:telegram_id/requests` with
/// `{"kind":"redeem"|"payout"}`: hold the whole available balance.
async fn post_my_request(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(telegram_id): Path<i64>,
    body: Bytes,
) -> Response {
    if let Err(code) = validate_telegram_id_param(telegram_id) {
        return plain(code);
    }
    if let Err(code) = check_owner(&headers, &state, telegram_id) {
        return plain(code);
    }
    if let Err(code) = check_not_blocked(&state, telegram_id).await {
        return plain(code);
    }
    let Some(kind) = parse_request_kind(&body) else {
        return refused(StatusCode::BAD_REQUEST, "invalid_kind");
    };
    let opened =
        match crate::db::referral_credit::open_request(&state.db.orm, telegram_id, &kind).await {
            Ok(opened) => opened,
            Err(e) => return credit_error("request", e),
        };
    if !opened.already_open {
        // After the commit, and never on the request's path: a failed send is
        // logged, and the overview's open requests are the source of truth.
        let (first_name, username) =
            match crate::db::referral_credit::names_of(&state.db.orm, telegram_id).await {
                Ok(names) => names,
                Err(e) => {
                    tracing::warn!("referral request notice: names of {telegram_id}: {e}");
                    (None, None)
                }
            };
        let text = request_notice(
            &opened.request.kind,
            telegram_id,
            first_name.as_deref(),
            username.as_deref(),
            opened.request.amount_thb,
            opened.request.id,
        );
        let (bot, config) = (state.bot.clone(), state.config.clone());
        tokio::spawn(async move { crate::notify::notify_admins(&bot, &config, &text).await });
    }
    Json(opened).into_response()
}

// ── Admin routes ───────────────────────────────────────────────────────────

/// `GET /api/admin/referral-credit/overview`.
async fn get_overview(headers: HeaderMap, State(state): State<AppState>) -> Response {
    if let Err(code) = check_admin(&headers, &state) {
        return plain(code);
    }
    match crate::db::referral_credit::overview(&state.db.orm).await {
        Ok(overview) => Json(overview).into_response(),
        Err(e) => {
            tracing::error!("referral credit overview: {e}");
            plain(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// `POST /api/admin/referral-credit/rentals`: record one completed rental.
async fn post_rental(headers: HeaderMap, State(state): State<AppState>, body: Bytes) -> Response {
    let admin_id = match check_admin(&headers, &state) {
        Ok(admin_id) => admin_id,
        Err(code) => return plain(code),
    };
    let record = match parse_record_body(&body) {
        Ok(record) => record,
        Err(code) => return refused(StatusCode::BAD_REQUEST, code),
    };
    match crate::db::referral_credit::record_rental(&state.db.orm, admin_id, &record).await {
        Ok(recorded) => Json(recorded).into_response(),
        Err(e) => credit_error("record", e),
    }
}

/// `POST /api/admin/referral-credit/rentals/:id/reverse`: reverse a record
/// whole.
async fn post_reversal(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(rental_id): Path<i64>,
    body: Bytes,
) -> Response {
    let admin_id = match check_admin(&headers, &state) {
        Ok(admin_id) => admin_id,
        Err(code) => return plain(code),
    };
    let note = match parse_reverse_body(&body) {
        Ok(note) => note,
        Err(code) => return refused(StatusCode::BAD_REQUEST, code),
    };
    match crate::db::referral_credit::reverse_rental(
        &state.db.orm,
        rental_id,
        admin_id,
        note.as_deref(),
    )
    .await
    {
        Ok(reversed) => Json(reversed).into_response(),
        Err(e) => credit_error("reversal", e),
    }
}

/// `POST /api/admin/referral-credit/requests/:id/resolve`: `paid` (a payout
/// the manager made by hand) or `declined`.
async fn post_resolution(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(request_id): Path<i64>,
    body: Bytes,
) -> Response {
    let admin_id = match check_admin(&headers, &state) {
        Ok(admin_id) => admin_id,
        Err(code) => return plain(code),
    };
    let (action, note) = match parse_resolve_body(&body) {
        Ok(parsed) => parsed,
        Err(code) => return refused(StatusCode::BAD_REQUEST, code),
    };
    match crate::db::referral_credit::resolve_request(
        &state.db.orm,
        request_id,
        admin_id,
        action,
        note.as_deref(),
    )
    .await
    {
        Ok(resolved) => Json(resolved).into_response(),
        Err(e) => credit_error("resolution", e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trios::referral_credit::{NotCreditable, MAX_RENTAL_AMOUNT_THB};
    use serde_json::json;

    fn record(value: Value) -> Result<RecordRentalBody, &'static str> {
        parse_record_body(value.to_string().as_bytes())
    }

    fn base() -> Value {
        json!({
            "customer_telegram_id": 20,
            "rental_amount_thb": 1239,
            "idempotency_key": "3f1c9b1e-0a4b-4a5e-9d2f-7c1e2a3b4c5d",
        })
    }

    fn with(key: &str, value: Value) -> Value {
        let mut body = base();
        body[key] = value;
        body
    }

    #[test]
    fn a_record_body_is_read_field_by_field() {
        let parsed = record(base()).expect("the base body is valid");
        assert_eq!(parsed.customer_telegram_id, 20);
        assert_eq!(parsed.rental_amount_thb, 1239);
        assert_eq!(parsed.order_id, None);
        assert_eq!(parsed.note, None);
    }

    #[test]
    fn the_rental_amount_is_a_whole_number_of_baht_within_the_order_ceiling() {
        assert_eq!(
            record(with("rental_amount_thb", json!(0))).unwrap_err(),
            "invalid_rental_amount"
        );
        assert!(record(with("rental_amount_thb", json!(1))).is_ok());
        assert!(record(with("rental_amount_thb", json!(MAX_RENTAL_AMOUNT_THB))).is_ok());
        assert_eq!(
            record(with("rental_amount_thb", json!(MAX_RENTAL_AMOUNT_THB + 1))).unwrap_err(),
            "invalid_rental_amount"
        );
        assert_eq!(
            record(with("rental_amount_thb", json!(-5))).unwrap_err(),
            "invalid_rental_amount"
        );
        assert_eq!(
            record(with("rental_amount_thb", json!(1000.5))).unwrap_err(),
            "invalid_rental_amount"
        );
        assert_eq!(
            record(with("rental_amount_thb", json!("1000"))).unwrap_err(),
            "invalid_rental_amount"
        );
    }

    #[test]
    fn the_customer_note_order_id_and_key_are_bounded() {
        assert_eq!(
            record(with("customer_telegram_id", json!(0))).unwrap_err(),
            "invalid_customer"
        );
        assert_eq!(record(json!({})).unwrap_err(), "invalid_customer");
        assert_eq!(
            parse_record_body(b"not json").unwrap_err(),
            "invalid_customer"
        );

        assert!(record(with("note", json!("n".repeat(200)))).is_ok());
        assert_eq!(
            record(with("note", json!("n".repeat(201)))).unwrap_err(),
            "invalid_note"
        );
        assert_eq!(
            record(with("note", json!("ю".repeat(200)))).map(|b| b.note.map(|n| n.chars().count())),
            Ok(Some(200))
        );
        assert_eq!(record(with("note", json!(5))).unwrap_err(), "invalid_note");
        assert_eq!(record(with("note", json!("  "))).map(|b| b.note), Ok(None));

        assert!(record(with("order_id", json!("o".repeat(36)))).is_ok());
        assert_eq!(
            record(with("order_id", json!("o".repeat(37)))).unwrap_err(),
            "invalid_order_id"
        );
        assert_eq!(
            record(with("order_id", json!(""))).map(|b| b.order_id),
            Ok(None)
        );

        assert_eq!(
            record(with("idempotency_key", json!(""))).unwrap_err(),
            "invalid_idempotency_key"
        );
        assert_eq!(
            record(with("idempotency_key", json!("has space"))).unwrap_err(),
            "invalid_idempotency_key"
        );
        assert_eq!(
            record(with("idempotency_key", json!("k".repeat(101)))).unwrap_err(),
            "invalid_idempotency_key"
        );
        let mut missing = base();
        missing
            .as_object_mut()
            .expect("an object")
            .remove("idempotency_key");
        assert_eq!(record(missing).unwrap_err(), "invalid_idempotency_key");
    }

    #[test]
    fn the_request_kind_and_the_resolve_action_are_closed_lists() {
        assert_eq!(
            parse_request_kind(br#"{"kind":"payout"}"#).as_deref(),
            Some("payout")
        );
        assert_eq!(
            parse_request_kind(br#"{"kind":"redeem"}"#).as_deref(),
            Some("redeem")
        );
        for bad in [
            &br#"{"kind":"accrual"}"#[..],
            br#"{"kind":""}"#,
            br#"{}"#,
            b"",
            b"[1]",
            b"{",
        ] {
            assert_eq!(
                parse_request_kind(bad),
                None,
                "{}",
                String::from_utf8_lossy(bad)
            );
        }
        assert_eq!(
            parse_resolve_body(br#"{"action":"paid","note":"cash"}"#),
            Ok(("paid", Some("cash".to_string())))
        );
        assert_eq!(
            parse_resolve_body(br#"{"action":"declined"}"#),
            Ok(("declined", None))
        );
        for bad in [
            &br#"{"action":"applied"}"#[..],
            br#"{"action":"open"}"#,
            br#"{}"#,
            b"{",
        ] {
            assert_eq!(parse_resolve_body(bad), Err("invalid_action"));
        }
        let long_note = json!({"action": "paid", "note": "n".repeat(201)}).to_string();
        assert_eq!(
            parse_resolve_body(long_note.as_bytes()),
            Err("invalid_note")
        );
        assert_eq!(parse_reverse_body(b""), Ok(None));
        assert_eq!(
            parse_reverse_body(br#"{"note":"wrong amount"}"#),
            Ok(Some("wrong amount".to_string()))
        );
        assert_eq!(
            parse_reverse_body(json!({"note": "n".repeat(201)}).to_string().as_bytes()),
            Err("invalid_note")
        );
    }

    async fn answer(refusal: Refusal) -> (StatusCode, Value) {
        let response = refusal_response(refusal);
        let status = response.status();
        let body = axum::body::to_bytes(response.into_body(), 1 << 16)
            .await
            .expect("a body");
        (status, serde_json::from_slice(&body).expect("a JSON body"))
    }

    #[tokio::test]
    async fn every_refusal_has_its_status_and_code() {
        use Refusal::*;
        let open = ReferralRequest {
            id: 17,
            kind: "payout".into(),
            amount_thb: 450,
            status: "open".into(),
            created_at: "2026-09-27T10:00:00Z".into(),
        };
        for (refusal, status, code) in [
            (OrderNotFound, 404, "order_not_found"),
            (OrderOfAnotherCustomer, 422, "order_of_another_customer"),
            (OrderHasNoRentalLine, 422, "order_has_no_rental_line"),
            (OrderNotCompleted, 409, "order_not_completed"),
            (OrderBeforeProgram, 409, "order_before_program"),
            (OrderAlreadyRecorded, 409, "order_already_recorded"),
            (IdempotencyKeyReused, 409, "idempotency_key_reused"),
            (RecorderIsInviter, 422, "recorder_is_inviter"),
            (
                NoReferralEffect(NotCreditable::SelfEdge),
                422,
                "no_referral_effect",
            ),
            (RequestOpen(open.clone()), 409, "request_open"),
            (NothingAvailable, 422, "nothing_available"),
            (RentalNotFound, 404, "rental_not_found"),
            (RequestNotFound, 404, "request_not_found"),
            (RequestNotOpen("paid".into()), 409, "request_not_open"),
            (RedeemCannotBePaid, 422, "redeem_cannot_be_paid"),
            (BalanceBelowRequest, 409, "balance_below_request"),
        ] {
            let (got_status, body) = answer(refusal).await;
            assert_eq!(got_status.as_u16(), status, "{code}");
            assert_eq!(body["error"], code);
        }
        assert_eq!(
            answer(NoReferralEffect(NotCreditable::ExistingCustomer))
                .await
                .1["reason"],
            "existing_customer"
        );
        assert_eq!(
            answer(RequestNotOpen("declined".into())).await.1["status"],
            "declined"
        );
        assert_eq!(answer(RequestOpen(open)).await.1["open_request"]["id"], 17);
    }

    #[test]
    fn the_admins_notice_escapes_what_a_customer_typed() {
        let text = request_notice("payout", 123, Some("<b>Ann</b>"), Some("ann&co"), 450, 17);
        assert!(
            text.starts_with("💸 <b>Referral payout request</b>\n"),
            "{text}"
        );
        assert!(
            text.contains("&lt;b&gt;Ann&lt;/b&gt; @ann&amp;co id 123"),
            "{text}"
        );
        assert!(text.contains("#R17"), "{text}");
        assert!(text.ends_with("Admin → Лояльность → Рефералы"), "{text}");
        let redeem = request_notice("redeem", 123, None, None, 450, 18);
        assert!(
            redeem.starts_with("🏍 <b>Referral balance to apply to a rental</b>\nid 123\n"),
            "{redeem}"
        );
    }
}
