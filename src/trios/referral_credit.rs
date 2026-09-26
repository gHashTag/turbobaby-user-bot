//! The referral credit: 10% of every completed rental of an invited friend,
//! in whole baht, credited to the friend's inviter.
//!
//! The owner decided it on 2026-09-26 (R3). His words, verbatim:
//!
//! * «Должно начисляться исключительно за то кто арендовал 10% скидка»
//!   ("it must be credited only for someone who rented; a 10% discount");
//! * «Пригласивший и может забрать скидкой за аренду или деньгами»
//!   ("the inviter can take it as a discount on a rental, or as money");
//! * on the referral bonus points: «Убрать, только скидка 10%»
//!   ("remove them; only the 10% discount");
//! * and he confirmed the rule this module computes: when an invited friend
//!   completes a rental, the inviter is credited 10% of that rental's amount,
//!   excluding the deposit and delivery, «с каждой аренды друга» ("for each
//!   rental of the friend"). The inviter may spend it on their own rental or
//!   ask for a payout, which a manager makes by hand. The friend gets nothing.
//!
//! The server never knows a rental's amount: a bike line is priced at 0 by the
//! checkout (D11), and no shipped client creates a rental order. So the credit
//! is born only when a manager RECORDS a completed rental with the charge he
//! took (`POST /api/admin/referral-credit/rentals`), and this module is the one
//! arithmetic the server, the admin screen and the customer page share (D15).
//!
//! The credit is a THB ledger of its own (migration 089), never loyalty points:
//! points cannot pay a rental, and a payout from the points balance would pay
//! out cashback. Nothing here moves money. `specs/turbobaby/referral_credit.t27`
//! (turbobaby/referral-credit) owns the rate, the rounding, the holds, the
//! settlement and the ledger's structure, and gate 3 binds the constants below
//! to it and to migration 089's CHECKs.

use serde::{Deserialize, Serialize};

/// Owner, 2026-09-26 (R3). Bound by gate 3 to referral_credit.t27 CREDIT_PERCENT and to 089's CHECK.
pub const REFERRAL_CREDIT_PERCENT: i64 = 10;
/// = MAX_ORDER_TOTAL in validate_create_order (gate 3 binds all three copies).
pub const MAX_RENTAL_AMOUNT_THB: i64 = 100_000_000;
/// The longest note a manager may attach to a record, a reversal or a resolution.
pub const NOTE_MAX_CHARS: usize = 200;
/// The ledger's row kinds, in migration 089's CHECK order.
pub const LEDGER_KINDS: [&str; 5] = [
    "accrual",
    "accrual_reversal",
    "redeem",
    "redeem_reversal",
    "payout",
];
/// What a customer may ask for: a payout by hand, or the balance spent on a rental.
pub const REQUEST_KINDS: [&str; 2] = ["payout", "redeem"];
/// A request's statuses, in migration 089's CHECK order.
pub const REQUEST_STATUSES: [&str; 4] = ["open", "applied", "paid", "declined"];
/// The first half of the two-int4 advisory-lock key a person's balance is
/// locked under (`pg_advisory_xact_lock(hashtext('referral_credit'), hashtext(tid))`).
/// The two-int4 key space never meets `create_order`'s one-bigint lock.
pub const LOCK_NAMESPACE: &str = "referral_credit";

/// A rental charge a manager may record: a whole number of baht, at least 1,
/// at most [`MAX_RENTAL_AMOUNT_THB`].
pub fn rental_amount_is_valid(thb: i64) -> bool {
    (1..=MAX_RENTAL_AMOUNT_THB).contains(&thb)
}

/// floor(PERCENT% of (rental - applied)); 0 when the amount is invalid or applied is outside 0..=rental.
///
/// `applied` is the referral balance the FRIEND spent on this same rental: the
/// base is the cash the shop took, so the credit never exceeds 10% of it.
/// Integer division of a non-negative number is the floor, so the credit is
/// rounded down to whole baht.
pub fn credit_for_rental(rental_amount_thb: i64, applied_thb: i64) -> i64 {
    if !rental_amount_is_valid(rental_amount_thb)
        || applied_thb < 0
        || applied_thb > rental_amount_thb
    {
        return 0;
    }
    (rental_amount_thb - applied_thb) * REFERRAL_CREDIT_PERCENT / 100
}

/// What a person may still ask for: the balance less what an open request
/// already holds, never below zero.
pub fn available(balance_thb: i64, held_thb: i64) -> i64 {
    (balance_thb - held_thb.max(0)).max(0)
}

/// The balance a customer is shown. A reversal can leave a balance negative,
/// and no approved sentence explains a debt, so a negative balance is shown as
/// zero; the API still returns the signed figure.
pub fn shown_balance(balance_thb: i64) -> i64 {
    balance_thb.max(0)
}

/// How much of an open «Списать в счёт аренды» request is applied to the
/// rental being recorded: the least of the hold, the rental charge and the
/// balance, never below zero.
pub fn applied_redemption(hold_thb: i64, rental_amount_thb: i64, balance_thb: i64) -> i64 {
    hold_thb.min(rental_amount_thb).min(balance_thb).max(0)
}

/// Some items[*].bike.deal.kind for which crate::trios::pricing::cart_kind_is_served is true ("bike_rental").
///
/// `items` is an order's stored `items` JSONB. A line of the previous shop has
/// no `bike`, and a `bike_sale` line is not a rental, so neither makes an order
/// one a rental can be recorded against.
pub fn order_holds_a_rental(items: &serde_json::Value) -> bool {
    let Some(lines) = items.as_array() else {
        return false;
    };
    lines.iter().any(|line| {
        line.get("bike")
            .and_then(|bike| bike.get("deal"))
            .and_then(|deal| deal.get("kind"))
            .and_then(|kind| kind.as_str())
            .is_some_and(crate::trios::pricing::cart_kind_is_served)
    })
}

/// Is `kind` one of [`REQUEST_KINDS`]?
pub fn is_request_kind(kind: &str) -> bool {
    REQUEST_KINDS.contains(&kind)
}

/// Why a recorded rental credits nobody.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotCreditable {
    /// The customer was invited by nobody (no `referral_events` row).
    NoEdge,
    /// The edge points at the customer (a migration-007 backfill self edge).
    SelfEdge,
    /// The edge was created after the linked order.
    EdgeAfterOrder,
    /// The customer bought or rented before the edge was created.
    ExistingCustomer,
}

impl NotCreditable {
    /// The reason code the API returns.
    pub fn code(self) -> &'static str {
        match self {
            NotCreditable::NoEdge => "no_edge",
            NotCreditable::SelfEdge => "self_edge",
            NotCreditable::EdgeAfterOrder => "edge_after_order",
            NotCreditable::ExistingCustomer => "existing_customer",
        }
    }
}

/// Checked in this order: NoEdge, SelfEdge, EdgeAfterOrder, ExistingCustomer.
///
/// Returns the inviter to credit. `edge_referrer` is the `referrer_id` of the
/// customer's `referral_events` row, any status; `edge_after_order` is true
/// only when a linked order predates the edge; `customer_before_edge` is true
/// when the customer bought or had a rental recorded before the edge.
pub fn creditable_inviter(
    customer: i64,
    edge_referrer: Option<i64>,
    edge_after_order: bool,
    customer_before_edge: bool,
) -> Result<i64, NotCreditable> {
    let Some(inviter) = edge_referrer else {
        return Err(NotCreditable::NoEdge);
    };
    if inviter == customer {
        return Err(NotCreditable::SelfEdge);
    }
    if edge_after_order {
        return Err(NotCreditable::EdgeAfterOrder);
    }
    if customer_before_edge {
        return Err(NotCreditable::ExistingCustomer);
    }
    Ok(inviter)
}

// ── Wire types ─────────────────────────────────────────────────────────────
// Timestamps are RFC 3339 UTC strings ("2026-09-27T10:00:00Z"). Money is whole
// THB. No number field is defaulted: an absent figure stays absent (D9), and a
// client that cannot parse a response hides the block.

/// One request of a customer, as the customer sees it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReferralRequest {
    pub id: i64,
    pub kind: String,
    pub amount_thb: i64,
    pub status: String,
    pub created_at: String,
}

/// A customer's referral balance. No entries, no friend and no order are
/// named: a customer sees figures only.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReferralCredit {
    pub balance_thb: i64,
    pub held_thb: i64,
    pub available_thb: i64,
    pub open_request: Option<ReferralRequest>,
}

/// `POST /api/referral-credit/me/:telegram_id/requests`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OpenRequestBody {
    pub kind: String,
}

/// The answer to an opened (or already open) request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OpenRequestResponse {
    pub request: ReferralRequest,
    pub already_open: bool,
    pub credit: ReferralCredit,
}

/// Every refusal body of the credit routes that carries one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ApiError {
    pub error: String,
    pub reason: Option<String>,
    pub status: Option<String>,
    pub open_request: Option<ReferralRequest>,
}

/// `POST /api/admin/referral-credit/rentals`: a manager records a completed
/// rental. `rental_amount_thb` is the rental charge he took, without deposit
/// and delivery, before any referral balance is applied.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecordRentalBody {
    pub customer_telegram_id: i64,
    pub rental_amount_thb: i64,
    pub order_id: Option<String>,
    pub note: Option<String>,
    pub idempotency_key: String,
}

/// One recorded rental, as the admin sees it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RentalRow {
    pub id: i64,
    pub customer_telegram_id: i64,
    pub order_id: Option<String>,
    pub rental_amount_thb: i64,
    pub applied_thb: i64,
    pub inviter_telegram_id: Option<i64>,
    pub credit_thb: i64,
    pub note: Option<String>,
    pub recorded_by: i64,
    pub recorded_at: String,
    pub reversed_at: Option<String>,
    pub reversed_by: Option<i64>,
    pub reversal_note: Option<String>,
}

/// Who a record credited, and how much.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreditOutcome {
    pub inviter_telegram_id: i64,
    pub credit_thb: i64,
}

/// Which open redeem request a record settled, and how much it applied.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RedemptionOutcome {
    pub request_id: i64,
    pub applied_thb: i64,
}

/// The answer to a record (or to its replay under the same key).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecordRentalResponse {
    pub rental: RentalRow,
    pub credit: Option<CreditOutcome>,
    pub redemption: Option<RedemptionOutcome>,
    pub idempotent_replay: bool,
}

/// `POST /api/admin/referral-credit/rentals/:id/reverse`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReverseBody {
    pub note: Option<String>,
}

/// The answer to a reversal. A reversal is always whole.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReverseResponse {
    pub rental: RentalRow,
    pub accrual_reversed_thb: i64,
    pub redemption_returned_thb: i64,
    pub already_reversed: bool,
}

/// `POST /api/admin/referral-credit/requests/:id/resolve`; `action` is
/// `"paid"` or `"declined"`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResolveBody {
    pub action: String,
    pub note: Option<String>,
}

/// One request, as the admin sees it, with the requester's current balance.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AdminRequestRow {
    pub id: i64,
    pub telegram_id: i64,
    pub first_name: Option<String>,
    pub username: Option<String>,
    pub kind: String,
    pub amount_thb: i64,
    pub status: String,
    pub created_at: String,
    pub resolved_at: Option<String>,
    pub resolved_by: Option<i64>,
    pub rental_id: Option<i64>,
    pub applied_thb: Option<i64>,
    pub admin_note: Option<String>,
    pub balance_thb: i64,
}

/// The answer to a resolution.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResolveResponse {
    pub request: AdminRequestRow,
    pub balance_thb: i64,
    pub idempotent_replay: bool,
}

/// One invited friend, as the admin sees them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InviteeRow {
    pub telegram_id: i64,
    pub first_name: Option<String>,
    pub username: Option<String>,
    pub inviter_telegram_id: i64,
    pub inviter_first_name: Option<String>,
    pub inviter_username: Option<String>,
    pub invited_at: String,
    pub edge_status: String,
    pub rentals_recorded: i64,
    pub credit_thb: i64,
    pub creditable: bool,
    pub not_creditable_reason: Option<String>,
}

/// One person's non-zero balance.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BalanceRow {
    pub telegram_id: i64,
    pub first_name: Option<String>,
    pub username: Option<String>,
    pub balance_thb: i64,
}

/// `GET /api/admin/referral-credit/overview`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AdminOverview {
    pub open_requests: Vec<AdminRequestRow>,
    pub invitees: Vec<InviteeRow>,
    pub rentals: Vec<RentalRow>,
    pub balances: Vec<BalanceRow>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn credit_rounds_down_to_whole_baht() {
        assert_eq!(credit_for_rental(1239, 0), 123);
        assert_eq!(credit_for_rental(1234, 0), 123);
        assert_eq!(credit_for_rental(10, 0), 1);
        assert_eq!(credit_for_rental(9, 0), 0);
        assert_eq!(credit_for_rental(1000, 300), 70);
        assert_eq!(credit_for_rental(1005, 5), 100);
        assert_eq!(credit_for_rental(MAX_RENTAL_AMOUNT_THB, 0), 10_000_000);
        assert_eq!(credit_for_rental(0, 0), 0);
        assert_eq!(credit_for_rental(-5, 0), 0);
        assert_eq!(credit_for_rental(MAX_RENTAL_AMOUNT_THB + 1, 0), 0);
        assert_eq!(credit_for_rental(100, 101), 0);
        assert_eq!(credit_for_rental(100, -1), 0);
    }

    #[test]
    fn credit_never_exceeds_the_owners_rate() {
        for r in 1..=5000_i64 {
            for a in [0, r / 2, r] {
                let credit = credit_for_rental(r, a);
                assert!(credit * 100 <= (r - a) * 10, "r={r} a={a} credit={credit}");
                assert!(
                    (r - a) * 10 - credit * 100 < 100,
                    "r={r} a={a} credit={credit}"
                );
            }
        }
    }

    #[test]
    fn available_and_shown_balance_are_never_negative() {
        assert_eq!(available(450, 0), 450);
        assert_eq!(available(450, 450), 0);
        assert_eq!(available(450, 200), 250);
        assert_eq!(available(-120, 0), 0);
        assert_eq!(available(100, 300), 0);
        assert_eq!(available(100, -50), 100);
        assert_eq!(shown_balance(450), 450);
        assert_eq!(shown_balance(0), 0);
        assert_eq!(shown_balance(-120), 0);
    }

    #[test]
    fn applied_redemption_is_the_least_of_hold_rental_and_balance() {
        assert_eq!(applied_redemption(200, 1000, 450), 200);
        assert_eq!(applied_redemption(500, 300, 500), 300);
        assert_eq!(applied_redemption(500, 1000, 120), 120);
        assert_eq!(applied_redemption(500, 1000, -40), 0);
        assert_eq!(applied_redemption(0, 1000, 500), 0);
    }

    #[test]
    fn creditable_inviter_refuses_in_order() {
        assert_eq!(creditable_inviter(2, Some(1), false, false), Ok(1));
        assert_eq!(
            creditable_inviter(2, None, true, true),
            Err(NotCreditable::NoEdge)
        );
        assert_eq!(
            creditable_inviter(2, Some(2), true, true),
            Err(NotCreditable::SelfEdge)
        );
        assert_eq!(
            creditable_inviter(2, Some(1), true, true),
            Err(NotCreditable::EdgeAfterOrder)
        );
        assert_eq!(
            creditable_inviter(2, Some(1), false, true),
            Err(NotCreditable::ExistingCustomer)
        );
        assert_eq!(NotCreditable::NoEdge.code(), "no_edge");
        assert_eq!(NotCreditable::SelfEdge.code(), "self_edge");
        assert_eq!(NotCreditable::EdgeAfterOrder.code(), "edge_after_order");
        assert_eq!(NotCreditable::ExistingCustomer.code(), "existing_customer");
    }

    #[test]
    fn an_order_holds_a_rental_only_through_a_bike_rental_line() {
        // The shape `BikeDeal::BikeRental` serializes to (`src/db/orders.rs`,
        // `#[serde(tag = "kind", rename_all = "snake_case")]`); a test there
        // serializes the real enum and asks this predicate.
        let rental = json!([{
            "quantity": 1.0,
            "bike": {"bike_key": "nmax-155", "bike_name": null, "deal": {
                "kind": "bike_rental", "rental_start": "2026-09-27",
                "rental_end": "2026-09-29", "rate_thb_day": null, "deposit": null}}
        }]);
        assert!(order_holds_a_rental(&rental));
        let sale = json!([{"quantity": 1.0, "bike": {"bike_key": "nmax-155",
            "deal": {"kind": "bike_sale", "price_thb": null}}}]);
        assert!(!order_holds_a_rental(&sale));
        let legacy = json!([{"quantity": 2.0, "unit_price": 100.0}]);
        assert!(!order_holds_a_rental(&legacy));
        assert!(!order_holds_a_rental(&json!([])));
        assert!(!order_holds_a_rental(
            &json!({"bike": {"deal": {"kind": "bike_rental"}}})
        ));
        assert!(!order_holds_a_rental(&serde_json::Value::Null));
        let mixed = json!([{"quantity": 1.0}, rental[0].clone()]);
        assert!(order_holds_a_rental(&mixed));
    }

    #[test]
    fn the_kind_lists_are_the_migrations() {
        assert_eq!(
            LEDGER_KINDS,
            [
                "accrual",
                "accrual_reversal",
                "redeem",
                "redeem_reversal",
                "payout"
            ]
        );
        assert_eq!(REQUEST_KINDS, ["payout", "redeem"]);
        assert_eq!(REQUEST_STATUSES, ["open", "applied", "paid", "declined"]);
        assert!(is_request_kind("payout"));
        assert!(is_request_kind("redeem"));
        assert!(!is_request_kind("accrual"));
        assert!(!is_request_kind(""));
        assert_eq!(REFERRAL_CREDIT_PERCENT, 10);
        assert_eq!(LOCK_NAMESPACE, "referral_credit");
        assert!(rental_amount_is_valid(1));
        assert!(rental_amount_is_valid(MAX_RENTAL_AMOUNT_THB));
        assert!(!rental_amount_is_valid(0));
        assert!(!rental_amount_is_valid(MAX_RENTAL_AMOUNT_THB + 1));
    }

    fn round_trip<T>(value: &T)
    where
        T: Serialize + for<'de> Deserialize<'de> + PartialEq + std::fmt::Debug,
    {
        let text = serde_json::to_string(value).expect("serialize");
        let back: T = serde_json::from_str(&text).expect("deserialize");
        assert_eq!(&back, value, "{text}");
    }

    fn keys(value: &serde_json::Value) -> Vec<String> {
        let mut out: Vec<String> = value
            .as_object()
            .expect("an object")
            .keys()
            .cloned()
            .collect();
        out.sort();
        out
    }

    #[test]
    fn every_wire_type_round_trips() {
        let request = ReferralRequest {
            id: 17,
            kind: "payout".into(),
            amount_thb: 450,
            status: "open".into(),
            created_at: "2026-09-27T10:00:00Z".into(),
        };
        let credit = ReferralCredit {
            balance_thb: 450,
            held_thb: 450,
            available_thb: 0,
            open_request: Some(request.clone()),
        };
        let rental = RentalRow {
            id: 3,
            customer_telegram_id: 20,
            order_id: Some("o-1".into()),
            rental_amount_thb: 1239,
            applied_thb: 0,
            inviter_telegram_id: Some(10),
            credit_thb: 123,
            note: None,
            recorded_by: 0,
            recorded_at: "2026-09-27T10:00:00Z".into(),
            reversed_at: None,
            reversed_by: None,
            reversal_note: None,
        };
        let admin_request = AdminRequestRow {
            id: 17,
            telegram_id: 10,
            first_name: Some("A".into()),
            username: None,
            kind: "redeem".into(),
            amount_thb: 200,
            status: "applied".into(),
            created_at: "2026-09-27T10:00:00Z".into(),
            resolved_at: Some("2026-09-28T10:00:00Z".into()),
            resolved_by: Some(0),
            rental_id: Some(3),
            applied_thb: Some(200),
            admin_note: None,
            balance_thb: 250,
        };
        let invitee = InviteeRow {
            telegram_id: 20,
            first_name: None,
            username: Some("friend".into()),
            inviter_telegram_id: 10,
            inviter_first_name: Some("A".into()),
            inviter_username: None,
            invited_at: "2026-09-26T10:00:00Z".into(),
            edge_status: "confirmed".into(),
            rentals_recorded: 2,
            credit_thb: 246,
            creditable: true,
            not_creditable_reason: None,
        };
        let balance = BalanceRow {
            telegram_id: 10,
            first_name: None,
            username: None,
            balance_thb: -40,
        };
        round_trip(&request);
        round_trip(&credit);
        round_trip(&OpenRequestBody {
            kind: "redeem".into(),
        });
        round_trip(&OpenRequestResponse {
            request: request.clone(),
            already_open: true,
            credit: credit.clone(),
        });
        round_trip(&ApiError {
            error: "request_open".into(),
            reason: None,
            status: None,
            open_request: Some(request.clone()),
        });
        round_trip(&RecordRentalBody {
            customer_telegram_id: 20,
            rental_amount_thb: 1239,
            order_id: None,
            note: Some("cash".into()),
            idempotency_key: "k-1".into(),
        });
        round_trip(&rental);
        round_trip(&RecordRentalResponse {
            rental: rental.clone(),
            credit: Some(CreditOutcome {
                inviter_telegram_id: 10,
                credit_thb: 123,
            }),
            redemption: Some(RedemptionOutcome {
                request_id: 17,
                applied_thb: 200,
            }),
            idempotent_replay: false,
        });
        round_trip(&ReverseBody { note: None });
        round_trip(&ReverseResponse {
            rental: rental.clone(),
            accrual_reversed_thb: 123,
            redemption_returned_thb: 0,
            already_reversed: false,
        });
        round_trip(&ResolveBody {
            action: "paid".into(),
            note: Some("cash".into()),
        });
        round_trip(&admin_request);
        round_trip(&ResolveResponse {
            request: admin_request.clone(),
            balance_thb: 250,
            idempotent_replay: true,
        });
        round_trip(&invitee);
        round_trip(&balance);
        round_trip(&AdminOverview {
            open_requests: vec![admin_request],
            invitees: vec![invitee],
            rentals: vec![rental],
            balances: vec![balance],
        });

        // What a customer is sent, key by key: figures, and nothing that names
        // a friend or an order.
        let wire = serde_json::to_value(&credit).expect("json");
        assert_eq!(
            keys(&wire),
            ["available_thb", "balance_thb", "held_thb", "open_request"]
        );
        assert_eq!(
            keys(&wire["open_request"]),
            ["amount_thb", "created_at", "id", "kind", "status"]
        );
        let empty = ReferralCredit {
            balance_thb: 450,
            held_thb: 0,
            available_thb: 450,
            open_request: None,
        };
        assert_eq!(
            serde_json::to_string(&empty).expect("json"),
            r#"{"balance_thb":450,"held_thb":0,"available_thb":450,"open_request":null}"#
        );
        // An absent figure stays absent: no number field is defaulted (D9).
        assert!(
            serde_json::from_str::<ReferralCredit>(r#"{"held_thb":0,"available_thb":0}"#).is_err()
        );
    }
}
