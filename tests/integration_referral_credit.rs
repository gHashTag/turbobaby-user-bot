//! The referral credit of 2026-09-26 (R3) on a real PostgreSQL, through the
//! router the way a client and an admin reach it.
//!
//! The owner decided, verbatim: «Должно начисляться исключительно за то кто
//! арендовал 10% скидка», «Пригласивший и может забрать скидкой за аренду или
//! деньгами», of the bonus points «Убрать, только скидка 10%», and he confirmed
//! the rule «с каждой аренды друга»: 10% of every completed rental of an invited
//! friend, without deposit and delivery, to the inviter, spent on a rental or
//! paid out by hand; the friend gets nothing.
//!
//! Every test makes its own people (fresh ids), so the tests share one
//! database and never clean up. `#[ignore]`d: run only against a private,
//! throwaway PostgreSQL (the harness refuses a non-local URL):
//! ```sh
//! DATABASE_URL=postgres://postgres@127.0.0.1:<port>/<fresh db> \
//!   cargo test --features backend --test integration_referral_credit -- --ignored --test-threads=1
//! ```

#![cfg(feature = "backend")]
#![allow(clippy::panic, clippy::expect_used)]

mod common;

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use common::{make_app_with_db, make_init_data};
use sea_orm::{ConnectionTrait, DbBackend, Statement};
use serde_json::{json, Value};
use tower::ServiceExt;
use turbobaby_bot::db::Database;

const BOT_TOKEN: &str = "dummy_test_token";
/// `tests/common`'s `admin_ids`.
const ADMIN_ID: i64 = 42;

struct Shop {
    app: axum::Router,
    db: Arc<Database>,
}

async fn shop() -> Option<Shop> {
    let (app, db) = make_app_with_db().await?;
    Some(Shop { app, db })
}

/// A person nobody else in this database is.
fn person() -> i64 {
    6_000_000_000 + (uuid::Uuid::new_v4().as_u128() % 1_000_000_000) as i64
}

fn key() -> String {
    uuid::Uuid::new_v4().to_string()
}

impl Shop {
    async fn call(
        &self,
        method: Method,
        uri: &str,
        headers: &[(&str, String)],
        body: Option<Value>,
    ) -> (StatusCode, Value) {
        let mut req = Request::builder().method(method).uri(uri);
        for (name, value) in headers {
            req = req.header(*name, value);
        }
        let req = match body {
            Some(body) => req
                .header("content-type", "application/json")
                .body(Body::from(body.to_string())),
            None => req.body(Body::empty()),
        }
        .expect("request");
        let resp = self.app.clone().oneshot(req).await.expect("response");
        let status = resp.status();
        let bytes = axum::body::to_bytes(resp.into_body(), 1 << 20)
            .await
            .expect("body");
        let value = if bytes.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&bytes)
                .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&bytes).into()))
        };
        (status, value)
    }

    async fn admin(&self, method: Method, uri: &str, body: Option<Value>) -> (StatusCode, Value) {
        let token = turbobaby_bot::api::auth::generate_admin_token("test_password", BOT_TOKEN);
        self.call(method, uri, &[("X-Admin-Token", token)], body)
            .await
    }

    async fn customer(
        &self,
        who: i64,
        method: Method,
        uri: &str,
        body: Option<Value>,
    ) -> (StatusCode, Value) {
        self.call(
            method,
            uri,
            &[("X-Telegram-Init-Data", make_init_data(who, BOT_TOKEN))],
            body,
        )
        .await
    }

    async fn sql(&self, sql: &str, values: Vec<sea_orm::Value>) {
        self.db
            .orm
            .execute(Statement::from_sql_and_values(
                DbBackend::Postgres,
                sql,
                values,
            ))
            .await
            .unwrap_or_else(|e| panic!("{sql}: {e}"));
    }

    async fn int(&self, sql: &str, values: Vec<sea_orm::Value>) -> i64 {
        let row = self
            .db
            .orm
            .query_one(Statement::from_sql_and_values(
                DbBackend::Postgres,
                sql,
                values,
            ))
            .await
            .unwrap_or_else(|e| panic!("{sql}: {e}"))
            .expect("a row");
        row.try_get::<i64>("", "n").expect("n")
    }

    /// `inviter` invited `friend`: the pending edge `/start ref_<code>` writes.
    async fn edge(&self, inviter: i64, friend: i64) {
        self.sql(
            "INSERT INTO referral_events (id, referrer_id, referred_id, code, status, source, created_at) \
             VALUES (gen_random_uuid(), $1, $2, 'TESTCODE', 'pending', 'telegram_start', NOW())",
            vec![inviter.into(), friend.into()],
        )
        .await;
    }

    /// A rental order of `customer` in `status`, created now.
    async fn rental_order(&self, customer: i64, status: &str) -> String {
        let id = key();
        self.sql(
            "INSERT INTO orders (id, telegram_id, items, status) VALUES ($1, $2, $3, $4)",
            vec![
                id.clone().into(),
                customer.into(),
                rental_items().into(),
                status.into(),
            ],
        )
        .await;
        id
    }

    async fn record(
        &self,
        customer: i64,
        amount: i64,
        order_id: Option<&str>,
    ) -> (StatusCode, Value) {
        self.record_keyed(customer, amount, order_id, &key()).await
    }

    async fn record_keyed(
        &self,
        customer: i64,
        amount: i64,
        order_id: Option<&str>,
        key: &str,
    ) -> (StatusCode, Value) {
        let mut body = json!({
            "customer_telegram_id": customer,
            "rental_amount_thb": amount,
            "idempotency_key": key,
        });
        if let Some(order_id) = order_id {
            body["order_id"] = json!(order_id);
        }
        self.admin(
            Method::POST,
            "/api/admin/referral-credit/rentals",
            Some(body),
        )
        .await
    }

    async fn summary(&self, who: i64) -> Value {
        let (status, body) = self
            .customer(
                who,
                Method::GET,
                &format!("/api/referral-credit/me/{who}"),
                None,
            )
            .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        body
    }

    async fn balance(&self, who: i64) -> i64 {
        self.int(
            "SELECT COALESCE(SUM(amount_thb), 0)::bigint AS n FROM referral_ledger WHERE telegram_id = $1",
            vec![who.into()],
        )
        .await
    }

    async fn request(&self, who: i64, kind: &str) -> (StatusCode, Value) {
        self.customer(
            who,
            Method::POST,
            &format!("/api/referral-credit/me/{who}/requests"),
            Some(json!({ "kind": kind })),
        )
        .await
    }

    async fn resolve(&self, request_id: i64, action: &str) -> (StatusCode, Value) {
        self.admin(
            Method::POST,
            &format!("/api/admin/referral-credit/requests/{request_id}/resolve"),
            Some(json!({ "action": action, "note": "by hand" })),
        )
        .await
    }

    async fn reverse(&self, rental_id: i64) -> (StatusCode, Value) {
        self.admin(
            Method::POST,
            &format!("/api/admin/referral-credit/rentals/{rental_id}/reverse"),
            Some(json!({ "note": "wrong amount" })),
        )
        .await
    }

    /// A person with `thb` of referral balance: they invited a fresh friend,
    /// whose rental of `thb × 10` was recorded.
    async fn funded(&self, thb: i64) -> (i64, i64) {
        let inviter = person();
        let friend = person();
        self.edge(inviter, friend).await;
        let (status, body) = self.record(friend, thb * 10, None).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["credit"]["credit_thb"], thb);
        (inviter, body["rental"]["id"].as_i64().expect("a rental id"))
    }
}

fn rental_items() -> Value {
    json!([{
        "quantity": 1.0,
        "fulfillment": "pickup",
        "bike": {"bike_key": "nmax-155", "bike_name": null, "deal": {
            "kind": "bike_rental", "rental_start": "2026-09-27", "rental_end": "2026-09-29",
            "rate_thb_day": null, "deposit": null}}
    }])
}

macro_rules! shop_or_skip {
    () => {
        match shop().await {
            Some(shop) => shop,
            None => {
                eprintln!("DATABASE_URL unset — skipping");
                return;
            }
        }
    };
}

/// 1. The friend's first completed order confirms the edge and counts the
/// friend, and credits nobody: no referral point row, no milestone, no
/// `friend_ordered` or `milestone` notice, no balance moved -- although the
/// harness's config still says `referral_welcome_bonus: 50.0`.
#[tokio::test]
#[ignore]
async fn a_first_completed_order_confirms_the_edge_and_credits_no_points() {
    let shop = shop_or_skip!();
    let (inviter, friend) = (person(), person());
    for id in [inviter, friend] {
        shop.sql(
            "INSERT INTO loyalty_profiles (telegram_id, bonus_balance) VALUES ($1, 7)",
            vec![id.into()],
        )
        .await;
    }
    shop.edge(inviter, friend).await;
    let order = shop.rental_order(friend, "pending").await;

    let completion =
        turbobaby_bot::db::orders::complete_order_and_update_loyalty(&shop.db.orm, &order)
            .await
            .expect("complete")
            .expect("newly completed");
    assert!(completion.is_first_order);

    let n = |sql: &'static str, id: i64| {
        let shop = &shop;
        async move { shop.int(sql, vec![id.into()]).await }
    };
    assert_eq!(
        n("SELECT COUNT(*)::bigint AS n FROM referral_events WHERE referred_id = $1 AND status = 'confirmed'", friend).await,
        1,
        "the edge was not confirmed"
    );
    assert_eq!(
        n(
            "SELECT referral_count::bigint AS n FROM loyalty_profiles WHERE telegram_id = $1",
            inviter
        )
        .await,
        1
    );
    for id in [inviter, friend] {
        assert_eq!(
            n("SELECT COUNT(*)::bigint AS n FROM bonus_transactions WHERE telegram_id = $1 AND tx_type LIKE 'referral%'", id).await,
            0,
            "a referral point row was written for {id}"
        );
        assert_eq!(
            n(
                "SELECT bonus_balance::bigint AS n FROM loyalty_profiles WHERE telegram_id = $1",
                id
            )
            .await,
            7,
            "a points balance moved for {id}"
        );
        assert_eq!(shop.balance(id).await, 0);
    }
    assert_eq!(
        n(
            "SELECT COUNT(*)::bigint AS n FROM referral_milestones WHERE referrer_id = $1",
            inviter
        )
        .await,
        0
    );
    assert_eq!(
        n("SELECT COUNT(*)::bigint AS n FROM notification_queue WHERE telegram_id = $1 AND kind IN ('friend_ordered', 'milestone')", inviter).await,
        0
    );
    assert_eq!(
        n(
            "SELECT bonus_paid::bigint AS n FROM referral_events WHERE referred_id = $1",
            friend
        )
        .await,
        0
    );
}

/// 2. 1239 ฿ credits the inviter 123 ฿ (10%, rounded down), and the friend
/// nothing.
#[tokio::test]
#[ignore]
async fn a_recorded_rental_credits_ten_percent_rounded_down_to_the_inviter_only() {
    let shop = shop_or_skip!();
    let (inviter, friend) = (person(), person());
    shop.edge(inviter, friend).await;
    let (status, body) = shop.record(friend, 1239, None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body["credit"],
        json!({"inviter_telegram_id": inviter, "credit_thb": 123})
    );
    assert_eq!(body["redemption"], Value::Null);
    assert_eq!(body["idempotent_replay"], false);
    assert_eq!(body["rental"]["rental_amount_thb"], 1239);
    assert_eq!(
        body["rental"]["recorded_by"], 0,
        "the password token records as 0"
    );

    let summary = shop.summary(inviter).await;
    assert_eq!(summary["balance_thb"], 123);
    assert_eq!(summary["available_thb"], 123);
    assert_eq!(shop.balance(friend).await, 0);
    assert_eq!(
        shop.int(
            "SELECT COUNT(*)::bigint AS n FROM referral_ledger WHERE telegram_id = $1",
            vec![friend.into()]
        )
        .await,
        0,
        "the friend was credited"
    );
    // The edge is confirmed by a creditable record, with no money of its own.
    assert_eq!(
        shop.int("SELECT COUNT(*)::bigint AS n FROM referral_events WHERE referred_id = $1 AND status = 'confirmed'", vec![friend.into()]).await,
        1
    );
}

/// 3. «с каждой аренды друга»: every recorded rental credits again.
#[tokio::test]
#[ignore]
async fn every_recorded_rental_credits_again() {
    let shop = shop_or_skip!();
    let (inviter, friend) = (person(), person());
    shop.edge(inviter, friend).await;
    for (amount, credit) in [(1000, 100), (2005, 200), (9, 0)] {
        let (status, body) = shop.record(friend, amount, None).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["credit"]["credit_thb"], credit);
    }
    assert_eq!(shop.balance(inviter).await, 300);
    // The 9 ฿ record is kept, with no ledger row.
    assert_eq!(
        shop.int(
            "SELECT COUNT(*)::bigint AS n FROM referral_rentals WHERE customer_telegram_id = $1",
            vec![friend.into()]
        )
        .await,
        3
    );
    assert_eq!(
        shop.int(
            "SELECT COUNT(*)::bigint AS n FROM referral_ledger WHERE telegram_id = $1",
            vec![inviter.into()]
        )
        .await,
        2
    );
}

/// 4. A replayed key answers the stored record; the key under another
/// payload is refused; a missing or malformed key is refused before anything.
#[tokio::test]
#[ignore]
async fn a_replayed_key_returns_the_record_and_a_changed_payload_conflicts() {
    let shop = shop_or_skip!();
    let (inviter, friend) = (person(), person());
    shop.edge(inviter, friend).await;
    let k = key();
    let (status, first) = shop.record_keyed(friend, 1239, None, &k).await;
    assert_eq!(status, StatusCode::OK, "{first}");
    let (status, again) = shop.record_keyed(friend, 1239, None, &k).await;
    assert_eq!(status, StatusCode::OK, "{again}");
    assert_eq!(again["idempotent_replay"], true);
    assert_eq!(again["rental"], first["rental"]);
    assert_eq!(again["credit"], first["credit"]);
    assert_eq!(shop.balance(inviter).await, 123, "a replay credited twice");

    let (status, body) = shop.record_keyed(friend, 1240, None, &k).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body["error"], "idempotency_key_reused");
    let (status, body) = shop.record_keyed(person(), 1239, None, &k).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body["error"], "idempotency_key_reused");

    let (status, body) = shop
        .admin(
            Method::POST,
            "/api/admin/referral-credit/rentals",
            Some(json!({"customer_telegram_id": friend, "rental_amount_thb": 1000})),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"], "invalid_idempotency_key");
    let (status, body) = shop.record_keyed(friend, 1000, None, "not a key").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"], "invalid_idempotency_key");
    assert_eq!(shop.balance(inviter).await, 123);

    // No admin, no record.
    let (status, _) = shop
        .call(
            Method::POST,
            "/api/admin/referral-credit/rentals",
            &[("X-Forwarded-For", "10.77.0.1".to_string())],
            Some(json!({"customer_telegram_id": friend, "rental_amount_thb": 1000, "idempotency_key": key()})),
        )
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

/// 5. A record linked to an order: the order is the customer's, completed,
/// holds a rental line, is newer than the programme and is recorded once
/// while live.
#[tokio::test]
#[ignore]
async fn order_link_guards() {
    let shop = shop_or_skip!();
    let (inviter, friend) = (person(), person());
    shop.edge(inviter, friend).await;

    let (status, body) = shop.record(friend, 1000, Some("no-such-order")).await;
    assert_eq!(
        (status, body["error"].clone()),
        (StatusCode::NOT_FOUND, json!("order_not_found"))
    );

    let pending = shop.rental_order(friend, "pending").await;
    let (status, body) = shop.record(friend, 1000, Some(&pending)).await;
    assert_eq!(
        (status, body["error"].clone()),
        (StatusCode::CONFLICT, json!("order_not_completed"))
    );

    let someone_elses = shop.rental_order(person(), "completed").await;
    let (status, body) = shop.record(friend, 1000, Some(&someone_elses)).await;
    assert_eq!(
        (status, body["error"].clone()),
        (
            StatusCode::UNPROCESSABLE_ENTITY,
            json!("order_of_another_customer")
        )
    );

    let no_rental = key();
    shop.sql(
        "INSERT INTO orders (id, telegram_id, items, status) VALUES ($1, $2, $3, 'completed')",
        vec![
            no_rental.clone().into(),
            friend.into(),
            json!([{"quantity": 2.0, "unit_price": 100.0}]).into(),
        ],
    )
    .await;
    let (status, body) = shop.record(friend, 1000, Some(&no_rental)).await;
    assert_eq!(
        (status, body["error"].clone()),
        (
            StatusCode::UNPROCESSABLE_ENTITY,
            json!("order_has_no_rental_line")
        )
    );

    let old = key();
    shop.sql(
        "INSERT INTO orders (id, telegram_id, items, status, created_at) VALUES ($1, $2, $3, 'completed', \
         (SELECT applied_at FROM _schema_migrations WHERE name = '089_referral_credit.sql') - INTERVAL '1 day')",
        vec![old.clone().into(), friend.into(), rental_items().into()],
    )
    .await;
    let (status, body) = shop.record(friend, 1000, Some(&old)).await;
    assert_eq!(
        (status, body["error"].clone()),
        (StatusCode::CONFLICT, json!("order_before_program"))
    );

    let order = shop.rental_order(friend, "completed").await;
    let (status, first) = shop.record(friend, 1000, Some(&order)).await;
    assert_eq!(status, StatusCode::OK, "{first}");
    assert_eq!(first["rental"]["order_id"], json!(order));
    let (status, body) = shop.record(friend, 1000, Some(&order)).await;
    assert_eq!(
        (status, body["error"].clone()),
        (StatusCode::CONFLICT, json!("order_already_recorded"))
    );
    assert_eq!(shop.balance(inviter).await, 100);

    // A correction: reverse, then record again under a new key.
    let rental_id = first["rental"]["id"].as_i64().expect("an id");
    let (status, reversed) = shop.reverse(rental_id).await;
    assert_eq!(status, StatusCode::OK, "{reversed}");
    let (status, again) = shop.record(friend, 1200, Some(&order)).await;
    assert_eq!(status, StatusCode::OK, "{again}");
    assert_eq!(shop.balance(inviter).await, 120);
}

/// 6. A record that credits nobody and settles nothing is refused with its
/// reason and writes nothing; the recording admin may not credit himself.
#[tokio::test]
#[ignore]
async fn refusals() {
    let shop = shop_or_skip!();
    let rentals_of = |who: i64| {
        let shop = &shop;
        async move {
            shop.int(
                "SELECT COUNT(*)::bigint AS n FROM referral_rentals WHERE customer_telegram_id = $1",
                vec![who.into()],
            )
            .await
        }
    };

    let nobody_invited = person();
    let (status, body) = shop.record(nobody_invited, 1000, None).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["error"], "no_referral_effect");
    assert_eq!(body["reason"], "no_edge");
    assert_eq!(rentals_of(nobody_invited).await, 0);

    // A self edge, as migration 007's backfill wrote them: born confirmed.
    let backfilled = person();
    shop.sql(
        "INSERT INTO referral_events (id, referrer_id, referred_id, code, status) \
         VALUES (gen_random_uuid(), $1, $1, 'BACKFILL', 'confirmed')",
        vec![backfilled.into()],
    )
    .await;
    let (status, body) = shop.record(backfilled, 1000, None).await;
    assert_eq!(
        (status, body["reason"].clone()),
        (StatusCode::UNPROCESSABLE_ENTITY, json!("self_edge"))
    );
    assert_eq!(rentals_of(backfilled).await, 0);
    assert_eq!(shop.balance(backfilled).await, 0);

    // An edge newer than the order it is recorded against.
    let (late_inviter, late_friend) = (person(), person());
    let order = key();
    shop.sql(
        "INSERT INTO orders (id, telegram_id, items, status, created_at) VALUES ($1, $2, $3, 'completed', \
         (SELECT applied_at FROM _schema_migrations WHERE name = '089_referral_credit.sql') + INTERVAL '1 minute')",
        vec![order.clone().into(), late_friend.into(), rental_items().into()],
    )
    .await;
    shop.sql(
        "INSERT INTO referral_events (id, referrer_id, referred_id, code, status, created_at) \
         VALUES (gen_random_uuid(), $1, $2, 'TESTCODE', 'pending', \
         (SELECT applied_at FROM _schema_migrations WHERE name = '089_referral_credit.sql') + INTERVAL '2 minutes')",
        vec![late_inviter.into(), late_friend.into()],
    )
    .await;
    let (status, body) = shop.record(late_friend, 1000, Some(&order)).await;
    assert_eq!(
        (status, body["reason"].clone()),
        (StatusCode::UNPROCESSABLE_ENTITY, json!("edge_after_order"))
    );
    assert_eq!(rentals_of(late_friend).await, 0);

    // A customer who bought before the edge was recorded.
    let (inviter, old_customer) = (person(), person());
    shop.sql(
        "INSERT INTO loyalty_profiles (telegram_id, first_purchase_at) VALUES ($1, NOW() - INTERVAL '10 days')",
        vec![old_customer.into()],
    )
    .await;
    shop.edge(inviter, old_customer).await;
    let (status, body) = shop.record(old_customer, 1000, None).await;
    assert_eq!(
        (status, body["reason"].clone()),
        (StatusCode::UNPROCESSABLE_ENTITY, json!("existing_customer"))
    );
    assert_eq!(rentals_of(old_customer).await, 0);
    assert_eq!(shop.balance(inviter).await, 0);

    // The recording admin is the inviter.
    let admins_friend = person();
    shop.edge(ADMIN_ID, admins_friend).await;
    let (status, body) = shop
        .call(
            Method::POST,
            "/api/admin/referral-credit/rentals",
            &[("X-Telegram-Init-Data", make_init_data(ADMIN_ID, BOT_TOKEN))],
            Some(json!({"customer_telegram_id": admins_friend, "rental_amount_thb": 1000, "idempotency_key": key()})),
        )
        .await;
    assert_eq!(
        (status, body["error"].clone()),
        (
            StatusCode::UNPROCESSABLE_ENTITY,
            json!("recorder_is_inviter")
        )
    );
    assert_eq!(rentals_of(admins_friend).await, 0);
}

/// 7. A request holds the whole available balance; the same kind again is
/// the open one; the other kind is refused; nothing available is refused;
/// two at once leave one open request.
#[tokio::test]
#[ignore]
async fn requests() {
    let shop = shop_or_skip!();
    let (holder, _) = shop.funded(123).await;
    let (status, body) = shop.request(holder, "payout").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["already_open"], false);
    assert_eq!(body["request"]["amount_thb"], 123);
    assert_eq!(body["request"]["status"], "open");
    assert_eq!(body["credit"]["held_thb"], 123);
    assert_eq!(body["credit"]["available_thb"], 0);
    let request_id = body["request"]["id"].clone();

    let (status, body) = shop.request(holder, "payout").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["already_open"], true);
    assert_eq!(body["request"]["id"], request_id);

    let (status, body) = shop.request(holder, "redeem").await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body["error"], "request_open");
    assert_eq!(body["open_request"]["id"], request_id);

    let (status, body) = shop.request(holder, "accrual").await;
    assert_eq!(
        (status, body["error"].clone()),
        (StatusCode::BAD_REQUEST, json!("invalid_kind"))
    );

    let penniless = person();
    let (status, body) = shop.request(penniless, "payout").await;
    assert_eq!(
        (status, body["error"].clone()),
        (StatusCode::UNPROCESSABLE_ENTITY, json!("nothing_available"))
    );

    // Another person's balance is not the caller's to ask for.
    let (status, _) = shop
        .customer(
            penniless,
            Method::POST,
            &format!("/api/referral-credit/me/{holder}/requests"),
            Some(json!({"kind": "payout"})),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (twice, _) = shop.funded(50).await;
    let (a, b) = tokio::join!(shop.request(twice, "redeem"), shop.request(twice, "redeem"));
    assert_eq!(
        (a.0, b.0),
        (StatusCode::OK, StatusCode::OK),
        "{} / {}",
        a.1,
        b.1
    );
    assert_eq!(a.1["request"]["id"], b.1["request"]["id"]);
    assert_eq!(
        shop.int("SELECT COUNT(*)::bigint AS n FROM referral_requests WHERE telegram_id = $1 AND status = 'open'", vec![twice.into()]).await,
        1
    );
}

/// 8. «Списать в счёт аренды»: the hold is applied at the customer's next
/// recorded rental, as the least of the hold, the charge and the balance;
/// the customer's own inviter is credited on the cash the shop took.
#[tokio::test]
#[ignore]
async fn a_redeem_applies_at_the_next_recorded_rental() {
    let shop = shop_or_skip!();
    // P holds 200, then earns 50 more; P was invited by Q.
    let (p, _) = shop.funded(200).await;
    let q = person();
    shop.edge(q, p).await;
    let (status, opened) = shop.request(p, "redeem").await;
    assert_eq!(status, StatusCode::OK, "{opened}");
    assert_eq!(opened["request"]["amount_thb"], 200);
    let second_friend = person();
    shop.edge(p, second_friend).await;
    assert_eq!(
        shop.record(second_friend, 500, None).await.0,
        StatusCode::OK
    );
    assert_eq!(shop.balance(p).await, 250);

    let (status, body) = shop.record(p, 1000, None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["redemption"]["applied_thb"], 200);
    assert_eq!(body["redemption"]["request_id"], opened["request"]["id"]);
    assert_eq!(body["rental"]["applied_thb"], 200);
    assert_eq!(
        body["credit"],
        json!({"inviter_telegram_id": q, "credit_thb": 80})
    );
    assert_eq!(shop.balance(q).await, 80);
    let summary = shop.summary(p).await;
    assert_eq!(summary["balance_thb"], 50);
    assert_eq!(summary["open_request"], Value::Null);
    assert_eq!(
        summary["available_thb"], 50,
        "the rest of the balance is free again"
    );
    let status_of = shop
        .int(
            "SELECT COUNT(*)::bigint AS n FROM referral_requests WHERE id = $1 AND status = 'applied' AND applied_thb = 200",
            vec![opened["request"]["id"].as_i64().expect("id").into()],
        )
        .await;
    assert_eq!(status_of, 1);

    // A hold larger than the rental: the rental is the ceiling, and nothing is
    // left to credit Q on.
    let (r, _) = shop.funded(500).await;
    shop.edge(q, r).await;
    assert_eq!(shop.request(r, "redeem").await.0, StatusCode::OK);
    let (status, body) = shop.record(r, 300, None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["redemption"]["applied_thb"], 300);
    assert_eq!(body["credit"]["credit_thb"], 0);
    assert_eq!(shop.balance(r).await, 200);
    assert_eq!(
        shop.balance(q).await,
        80,
        "a zero credit wrote a ledger row"
    );
    assert_eq!(shop.summary(r).await["available_thb"], 200);
}

/// 9. «Запросить выплату»: a manager pays by hand and marks it paid; declined
/// frees the hold; a payout never overdraws; a redeem is never paid.
#[tokio::test]
#[ignore]
async fn a_payout_is_marked_paid_by_hand() {
    let shop = shop_or_skip!();
    let (holder, _) = shop.funded(123).await;
    let (_, opened) = shop.request(holder, "payout").await;
    let id = opened["request"]["id"].as_i64().expect("id");
    let (status, body) = shop.resolve(id, "paid").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["request"]["status"], "paid");
    assert_eq!(body["request"]["admin_note"], "by hand");
    assert_eq!(body["balance_thb"], 0);
    assert_eq!(body["idempotent_replay"], false);
    assert_eq!(
        shop.int(
            "SELECT amount_thb AS n FROM referral_ledger WHERE request_id = $1 AND kind = 'payout'",
            vec![id.into()]
        )
        .await,
        -123
    );
    let (status, body) = shop.resolve(id, "paid").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["idempotent_replay"], true);
    assert_eq!(shop.balance(holder).await, 0);
    let (status, body) = shop.resolve(id, "declined").await;
    assert_eq!(
        (status, body["error"].clone(), body["status"].clone()),
        (
            StatusCode::CONFLICT,
            json!("request_not_open"),
            json!("paid")
        )
    );

    // Declined: the hold goes, and no row is written.
    let (decliner, _) = shop.funded(40).await;
    let (_, opened) = shop.request(decliner, "payout").await;
    let id = opened["request"]["id"].as_i64().expect("id");
    let rows = shop
        .int(
            "SELECT COUNT(*)::bigint AS n FROM referral_ledger WHERE telegram_id = $1",
            vec![decliner.into()],
        )
        .await;
    let (status, body) = shop.resolve(id, "declined").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["request"]["status"], "declined");
    assert_eq!(
        shop.int(
            "SELECT COUNT(*)::bigint AS n FROM referral_ledger WHERE telegram_id = $1",
            vec![decliner.into()]
        )
        .await,
        rows
    );
    assert_eq!(shop.summary(decliner).await["available_thb"], 40);
    assert_eq!(
        shop.resolve(id, "declined").await.1["idempotent_replay"],
        true
    );

    // A reversal lowered the balance under the request: not paid.
    let (short, rental_id) = shop.funded(123).await;
    let (_, opened) = shop.request(short, "payout").await;
    let id = opened["request"]["id"].as_i64().expect("id");
    assert_eq!(shop.reverse(rental_id).await.0, StatusCode::OK);
    let (status, body) = shop.resolve(id, "paid").await;
    assert_eq!(
        (status, body["error"].clone()),
        (StatusCode::CONFLICT, json!("balance_below_request"))
    );

    // A redeem is applied at a rental, never paid out.
    let (redeemer, _) = shop.funded(30).await;
    let (_, opened) = shop.request(redeemer, "redeem").await;
    let id = opened["request"]["id"].as_i64().expect("id");
    let (status, body) = shop.resolve(id, "paid").await;
    assert_eq!(
        (status, body["error"].clone()),
        (
            StatusCode::UNPROCESSABLE_ENTITY,
            json!("redeem_cannot_be_paid")
        )
    );

    let (status, body) = shop.resolve(i64::MAX, "paid").await;
    assert_eq!(
        (status, body["error"].clone()),
        (StatusCode::NOT_FOUND, json!("request_not_found"))
    );
    let (status, body) = shop.resolve(id, "applied").await;
    assert_eq!(
        (status, body["error"].clone()),
        (StatusCode::BAD_REQUEST, json!("invalid_action"))
    );
}

/// 10. A reversal is whole, idempotent, and may leave a balance negative; a
/// negative balance shows as nothing available and blocks a request.
#[tokio::test]
#[ignore]
async fn a_reversal_can_make_a_balance_negative() {
    let shop = shop_or_skip!();
    let (spender, rental_id) = shop.funded(123).await;
    let (_, opened) = shop.request(spender, "payout").await;
    let id = opened["request"]["id"].as_i64().expect("id");
    assert_eq!(shop.resolve(id, "paid").await.0, StatusCode::OK);
    let (status, body) = shop.reverse(rental_id).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["accrual_reversed_thb"], 123);
    assert_eq!(body["already_reversed"], false);
    assert!(body["rental"]["reversed_at"].is_string());
    let summary = shop.summary(spender).await;
    assert_eq!(
        summary["balance_thb"], -123,
        "the API returns the signed balance"
    );
    assert_eq!(summary["available_thb"], 0);
    let (status, body) = shop.request(spender, "payout").await;
    assert_eq!(
        (status, body["error"].clone()),
        (StatusCode::UNPROCESSABLE_ENTITY, json!("nothing_available"))
    );

    let (status, body) = shop.reverse(rental_id).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["already_reversed"], true);
    assert_eq!(
        shop.balance(spender).await,
        -123,
        "a second reversal moved money"
    );

    // The customer's applied balance comes back with the reversal.
    let (f, _) = shop.funded(100).await;
    let p = person();
    shop.edge(p, f).await;
    assert_eq!(shop.request(f, "redeem").await.0, StatusCode::OK);
    let (status, body) = shop.record(f, 1000, None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["redemption"]["applied_thb"], 100);
    assert_eq!((shop.balance(f).await, shop.balance(p).await), (0, 90));
    let (status, body) = shop
        .reverse(body["rental"]["id"].as_i64().expect("id"))
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["redemption_returned_thb"], 100);
    assert_eq!((shop.balance(f).await, shop.balance(p).await), (100, 0));

    let (status, body) = shop.reverse(i64::MAX).await;
    assert_eq!(
        (status, body["error"].clone()),
        (StatusCode::NOT_FOUND, json!("rental_not_found"))
    );
}

/// 12. Loyalty points are not referral money: a points balance and a
/// cashback row put nothing in the referral ledger.
#[tokio::test]
#[ignore]
async fn loyalty_points_never_reach_a_payout() {
    let shop = shop_or_skip!();
    let rich = person();
    shop.sql(
        "INSERT INTO loyalty_profiles (telegram_id, bonus_balance) VALUES ($1, 1000)",
        vec![rich.into()],
    )
    .await;
    shop.sql(
        "INSERT INTO bonus_transactions (id, telegram_id, amount, tx_type, description) \
         VALUES ($1, $2, 1000, 'order_cashback', 'Cashback 5% for order x')",
        vec![key().into(), rich.into()],
    )
    .await;
    let (status, body) = shop.request(rich, "payout").await;
    assert_eq!(
        (status, body["error"].clone()),
        (StatusCode::UNPROCESSABLE_ENTITY, json!("nothing_available"))
    );
    assert_eq!(shop.summary(rich).await["balance_thb"], 0);
}

/// 13. What a customer is sent: figures only.
#[tokio::test]
#[ignore]
async fn the_customer_summary_names_no_friend_and_no_order() {
    let shop = shop_or_skip!();
    let (holder, _) = shop.funded(77).await;
    let keys = |v: &Value| {
        let mut k: Vec<String> = v.as_object().expect("an object").keys().cloned().collect();
        k.sort();
        k
    };
    let summary = shop.summary(holder).await;
    assert_eq!(
        keys(&summary),
        ["available_thb", "balance_thb", "held_thb", "open_request"]
    );
    assert_eq!(summary["open_request"], Value::Null);
    assert_eq!(shop.request(holder, "payout").await.0, StatusCode::OK);
    let summary = shop.summary(holder).await;
    assert_eq!(
        keys(&summary["open_request"]),
        ["amount_thb", "created_at", "id", "kind", "status"]
    );
    let created = summary["open_request"]["created_at"]
        .as_str()
        .expect("a timestamp");
    assert!(created.ends_with('Z') && created.len() == 20, "{created}");

    let (status, _) = shop
        .customer(
            person(),
            Method::GET,
            &format!("/api/referral-credit/me/{holder}"),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = shop
        .call(
            Method::GET,
            &format!("/api/referral-credit/me/{holder}"),
            &[],
            None,
        )
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

/// 14. A blocked customer can neither read nor request, and is still
/// credited when a friend's rental is recorded.
#[tokio::test]
#[ignore]
async fn a_blocked_customer_cannot_read_or_request_but_is_still_credited() {
    let shop = shop_or_skip!();
    let (blocked, friend) = (person(), person());
    shop.sql(
        "INSERT INTO loyalty_profiles (telegram_id, is_blocked) VALUES ($1, true)",
        vec![blocked.into()],
    )
    .await;
    shop.edge(blocked, friend).await;
    assert_eq!(shop.record(friend, 1000, None).await.0, StatusCode::OK);
    assert_eq!(shop.balance(blocked).await, 100);
    let (status, _) = shop
        .customer(
            blocked,
            Method::GET,
            &format!("/api/referral-credit/me/{blocked}"),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = shop.request(blocked, "payout").await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

/// 15. The schema itself refuses a row the code would never write.
#[tokio::test]
#[ignore]
async fn the_schema_refuses_bad_rows() {
    let shop = shop_or_skip!();
    let (inviter, rental_id) = shop.funded(10).await;
    let refused = |sql: &'static str, values: Vec<sea_orm::Value>| {
        let shop = &shop;
        async move {
            let result = shop
                .db
                .orm
                .execute(Statement::from_sql_and_values(
                    DbBackend::Postgres,
                    sql,
                    values,
                ))
                .await;
            assert!(result.is_err(), "the schema accepted: {sql}");
        }
    };
    // An accrual with the wrong sign, and a payout that names a rental.
    refused(
        "INSERT INTO referral_ledger (telegram_id, kind, amount_thb, rental_id, created_by) VALUES ($1, 'accrual', -5, $2, 0)",
        vec![inviter.into(), rental_id.into()],
    )
    .await;
    refused(
        "INSERT INTO referral_ledger (telegram_id, kind, amount_thb, rental_id, created_by) VALUES ($1, 'bonus', 5, $2, 0)",
        vec![inviter.into(), rental_id.into()],
    )
    .await;
    // A second accrual for one rental.
    refused(
        "INSERT INTO referral_ledger (telegram_id, kind, amount_thb, rental_id, created_by) VALUES ($1, 'accrual', 10, $2, 0)",
        vec![inviter.into(), rental_id.into()],
    )
    .await;
    // A self credit, and a credit that is not the rule.
    let me = person();
    refused(
        "INSERT INTO referral_rentals (customer_telegram_id, rental_amount_thb, inviter_telegram_id, credit_thb, idempotency_key, recorded_by) \
         VALUES ($1, 1000, $1, 100, 'schema-self', 0)",
        vec![me.into()],
    )
    .await;
    refused(
        "INSERT INTO referral_rentals (customer_telegram_id, rental_amount_thb, inviter_telegram_id, credit_thb, idempotency_key, recorded_by) \
         VALUES ($1, 1000, $2, 101, 'schema-rule', 0)",
        vec![me.into(), inviter.into()],
    )
    .await;
    refused(
        "INSERT INTO referral_rentals (customer_telegram_id, rental_amount_thb, idempotency_key, recorded_by) \
         VALUES ($1, 0, 'schema-zero', 0)",
        vec![me.into()],
    )
    .await;
    // Two open requests of one person.
    shop.sql(
        "INSERT INTO referral_requests (telegram_id, kind, amount_thb) VALUES ($1, 'payout', 5)",
        vec![me.into()],
    )
    .await;
    refused(
        "INSERT INTO referral_requests (telegram_id, kind, amount_thb) VALUES ($1, 'redeem', 5)",
        vec![me.into()],
    )
    .await;
    // A paid redeem.
    refused(
        "INSERT INTO referral_requests (telegram_id, kind, amount_thb, status, resolved_at) VALUES ($1, 'redeem', 5, 'paid', NOW())",
        vec![person().into()],
    )
    .await;
}

/// 16. Migration 089 creates and never touches an existing row: applied
/// again, it changes no count.
#[tokio::test]
#[ignore]
async fn migration_089_leaves_existing_rows_alone() {
    let shop = shop_or_skip!();
    let _ = shop.funded(10).await;
    let tables = [
        "orders",
        "loyalty_profiles",
        "bonus_transactions",
        "referral_events",
        "referral_milestones",
        "referral_rentals",
        "referral_requests",
        "referral_ledger",
    ];
    let mut before = Vec::new();
    for table in tables {
        before.push(
            shop.int(
                &format!("SELECT COUNT(*)::bigint AS n FROM {table}"),
                vec![],
            )
            .await,
        );
    }
    shop.db
        .orm
        .execute_unprepared(include_str!("../migrations/089_referral_credit.sql"))
        .await
        .expect("089 is idempotent");
    let mut after = Vec::new();
    for table in tables {
        after.push(
            shop.int(
                &format!("SELECT COUNT(*)::bigint AS n FROM {table}"),
                vec![],
            )
            .await,
        );
    }
    assert_eq!(before, after);
    assert_eq!(
        shop.int("SELECT COUNT(*)::bigint AS n FROM _schema_migrations WHERE name = '089_referral_credit.sql'", vec![]).await,
        1
    );
}

/// The admin overview carries what the manager acts on.
#[tokio::test]
#[ignore]
async fn the_overview_lists_open_requests_invitees_rentals_and_balances() {
    let shop = shop_or_skip!();
    let (holder, rental_id) = shop.funded(66).await;
    let (_, opened) = shop.request(holder, "payout").await;
    let (status, body) = shop
        .admin(Method::GET, "/api/admin/referral-credit/overview", None)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let open: Vec<&Value> = body["open_requests"]
        .as_array()
        .expect("a list")
        .iter()
        .filter(|r| r["id"] == opened["request"]["id"])
        .collect();
    assert_eq!(open.len(), 1);
    assert_eq!(open[0]["balance_thb"], 66);
    assert!(body["rentals"]
        .as_array()
        .expect("a list")
        .iter()
        .any(|r| r["id"] == rental_id));
    assert!(body["balances"]
        .as_array()
        .expect("a list")
        .iter()
        .any(|b| b["telegram_id"] == holder && b["balance_thb"] == 66));
    let invitee = body["invitees"]
        .as_array()
        .expect("a list")
        .iter()
        .find(|i| i["inviter_telegram_id"] == holder)
        .expect("the holder's friend");
    assert_eq!(invitee["rentals_recorded"], 1);
    assert_eq!(invitee["credit_thb"], 66);
    assert_eq!(invitee["creditable"], true);
    assert_eq!(invitee["edge_status"], "confirmed");

    let (status, _) = shop
        .call(
            Method::GET,
            "/api/admin/referral-credit/overview",
            &[("X-Forwarded-For", "10.77.0.2".to_string())],
            None,
        )
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}
