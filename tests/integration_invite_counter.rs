//! The invite counter counted the wrong thing.
//!
//! The profile screen prints a number under **«👥 Приглашено друзей: {0}»** and
//! that number was `loyalty_profiles.referral_count` — a denormalised column
//! written in exactly one place, `confirm_referral`, which is called from
//! exactly one place, `complete_order`. So it counts friends whose **order
//! completed**, not friends who arrived. Invite ten people, watch all ten join,
//! and the counter reads zero.
//!
//! The data was never missing: `record_referral` writes a `referral_events` row
//! the moment somebody follows the link, and `get_referrer_stats` has always
//! reported it. Three numbers for one idea, and the screen read the one that
//! disagreed with its own label.
//!
//! `invited_count` is now on the loyalty response and is what the label shows.
//!
//! Run with:
//! ```sh
//! DATABASE_URL=postgres://postgres:postgres@127.0.0.1:5432/woody_test \
//!   cargo test --features backend --test integration_invite_counter -- --ignored --test-threads=1
//! ```

#![cfg(feature = "backend")]

mod common;

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use common::{make_app_with_db, make_init_data};
use sea_orm::{ConnectionTrait, DbBackend, Statement};
use tower::ServiceExt;

const REFERRER: i64 = 970_001;
const FRIENDS: [i64; 3] = [970_002, 970_003, 970_004];

async fn profile(app: &axum::Router, who: i64) -> serde_json::Value {
    let req = Request::builder()
        .method(Method::GET)
        .uri(format!("/api/loyalty/{who}"))
        .header(
            "X-Telegram-Init-Data",
            make_init_data(who, "dummy_test_token"),
        )
        .body(Body::empty())
        .expect("request");
    let resp = app.clone().oneshot(req).await.expect("response");
    assert_eq!(resp.status(), StatusCode::OK, "GET /api/loyalty/{who}");
    let bytes = axum::body::to_bytes(resp.into_body(), 1 << 20)
        .await
        .expect("body");
    serde_json::from_slice(&bytes).expect("json")
}

/// Three friends follow the link and none of them buys anything.
///
/// This is the ordinary case — the whole point of an invite link is that it is
/// shared before anybody has ordered — and it is the case the old counter got
/// wrong. The assertion that discriminates is the pair: `invited_count` moves
/// and `referral_count` does not, so a fix that simply renamed one field would
/// fail here.
#[tokio::test]
#[ignore]
async fn friends_who_arrive_are_counted_before_they_buy_anything() {
    let Some((app, db)) = make_app_with_db().await else {
        eprintln!("DATABASE_URL unset — skipping");
        return;
    };

    // Clear the events, and make sure every participant has a profile row —
    // `GET /api/loyalty/:id` answers 404 without one, and a 404 would hide the
    // very number under test.
    for id in std::iter::once(REFERRER).chain(FRIENDS) {
        for sql in [
            "DELETE FROM referral_events WHERE referrer_id = $1 OR referred_id = $1",
            "INSERT INTO loyalty_profiles (telegram_id) VALUES ($1) ON CONFLICT (telegram_id) DO NOTHING",
        ] {
            let _ = db
                .orm
                .execute(Statement::from_sql_and_values(
                    DbBackend::Postgres,
                    sql,
                    [id.into()],
                ))
                .await;
        }
    }

    let before = profile(&app, REFERRER).await;
    assert_eq!(
        before["profile"]["invited_count"].as_i64(),
        Some(0),
        "a referrer nobody has followed starts at zero"
    );

    // Each friend follows the link. This is what the bot's `/start ref_<code>`
    // branch does — a pending event, no purchase.
    for (n, friend) in FRIENDS.iter().enumerate() {
        db.orm
            .execute(Statement::from_sql_and_values(
                DbBackend::Postgres,
                "INSERT INTO referral_events (id, referrer_id, referred_id, code, status, source, created_at) \
                 VALUES (gen_random_uuid(), $1, $2, 'TESTCODE', 'pending', 'telegram_start', NOW())",
                [REFERRER.into(), (*friend).into()],
            ))
            .await
            .expect("record a followed link");

        let now = profile(&app, REFERRER).await;
        assert_eq!(
            now["profile"]["invited_count"].as_i64(),
            Some(n as i64 + 1),
            "after {} friend(s) followed the link the counter reads {:?}",
            n + 1,
            now["profile"]["invited_count"]
        );
    }

    // And the number the screen used to print is still zero, because none of
    // them has bought anything. Both are true; only one of them is "invited".
    let after = profile(&app, REFERRER).await;
    assert_eq!(
        after["profile"]["referral_count"].as_i64(),
        Some(0),
        "referral_count counts completed orders and must NOT have moved — if it \
         did, this test is passing for the wrong reason"
    );
    assert_eq!(after["profile"]["invited_count"].as_i64(), Some(3));
}

/// The counter is per referrer, not global.
#[tokio::test]
#[ignore]
async fn one_persons_invites_are_not_another_persons() {
    let Some((app, db)) = make_app_with_db().await else {
        return;
    };
    let other = 970_055i64;
    for id in [REFERRER, other] {
        for sql in [
            "DELETE FROM referral_events WHERE referrer_id = $1 OR referred_id = $1",
            "INSERT INTO loyalty_profiles (telegram_id) VALUES ($1) ON CONFLICT (telegram_id) DO NOTHING",
        ] {
            let _ = db
                .orm
                .execute(Statement::from_sql_and_values(
                    DbBackend::Postgres,
                    sql,
                    [id.into()],
                ))
                .await;
        }
    }
    db.orm
        .execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "INSERT INTO referral_events (id, referrer_id, referred_id, code, status, source, created_at) \
             VALUES (gen_random_uuid(), $1, $2, 'TESTCODE', 'pending', 'telegram_start', NOW())",
            [REFERRER.into(), FRIENDS[0].into()],
        ))
        .await
        .expect("insert");

    assert_eq!(
        profile(&app, other).await["profile"]["invited_count"].as_i64(),
        Some(0),
        "somebody else's invite must not appear on this profile"
    );
}
