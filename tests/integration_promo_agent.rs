//! The promoter, against a real database.
//!
//! Three things a promotion agent has to get right, none of which a unit test
//! can show:
//!
//! 1. It finds what is genuinely new and **not** the 137 products already in
//!    the shop. A promoter whose first tick sends one message per existing
//!    product is worse than none.
//! 2. It never promotes the same thing twice. The claim is an `INSERT … ON
//!    CONFLICT DO NOTHING` on a UNIQUE key, so this is a claim about the
//!    schema, not about the code path.
//! 3. An event and its next-day reminder are two posts, not one — the reminder
//!    being the post with the seats left in it.
//!
//! Run with:
//! ```sh
//! DATABASE_URL=postgres://postgres:postgres@127.0.0.1:5432/woody_test \
//!   cargo test --features backend --test integration_promo_agent -- --ignored --test-threads=1
//! ```

#![cfg(feature = "backend")]

mod common;

use common::make_app_with_db;
use sea_orm::{ConnectionTrait, DbBackend, Statement};
use woody_weed_bot::trios::promo::{dedup_key, Subject};

const MARK: &str = "promo-agent-test";

async fn clean(db: &woody_weed_bot::db::Database) {
    for sql in [
        format!("DELETE FROM promo_posts WHERE subject_name LIKE '{MARK}%'"),
        format!("DELETE FROM strains WHERE name LIKE '{MARK}%'"),
        format!("DELETE FROM events WHERE title LIKE '{MARK}%'"),
    ] {
        db.orm
            .execute(Statement::from_string(DbBackend::Postgres, sql))
            .await
            .expect("clean slate");
    }
}

/// The claim, exactly as `promo::claim` performs it.
async fn claim(db: &woody_weed_bot::db::Database, subject: &Subject) -> bool {
    db.orm
        .execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "INSERT INTO promo_posts (dedup_key, kind, subject_id, subject_name, body) \
             VALUES ($1, $2, $3, $4, 'body') ON CONFLICT (dedup_key) DO NOTHING",
            [
                dedup_key(subject).into(),
                subject.kind().into(),
                subject.id().into(),
                subject.name().into(),
            ],
        ))
        .await
        .expect("claim")
        .rows_affected()
        > 0
}

/// The same thing is never promoted twice — across a restart, a crash
/// mid-send, or two instances overlapping during a rollover, all of which a
/// `SELECT` then `INSERT` would lose.
#[tokio::test]
#[ignore]
async fn a_subject_is_claimed_exactly_once() {
    let Some((_app, db)) = make_app_with_db().await else {
        eprintln!("DATABASE_URL unset — skipping");
        return;
    };
    clean(&db).await;

    let s = Subject::Strain {
        id: "promo-test-strain".into(),
        name: format!("{MARK} strain"),
    };
    assert!(claim(&db, &s).await, "the first claim must succeed");
    assert!(
        !claim(&db, &s).await,
        "the second claim succeeded, so the customer gets the post twice"
    );

    // And a genuinely different subject is not blocked by it.
    let other = Subject::Set {
        id: "promo-test-set".into(),
        name: format!("{MARK} set"),
    };
    assert!(claim(&db, &other).await, "a different subject was blocked");
}

/// An announcement and a reminder are two posts about one event.
///
/// Keyed on the id alone the reminder would be silently suppressed — and the
/// reminder is the one that fills the remaining seats.
#[tokio::test]
#[ignore]
async fn an_event_and_its_reminder_are_both_claimable() {
    let Some((_app, db)) = make_app_with_db().await else {
        return;
    };
    clean(&db).await;

    let announced = Subject::Event {
        id: "promo-test-event".into(),
        name: format!("{MARK} event"),
    };
    let soon = Subject::EventSoon {
        id: "promo-test-event".into(),
        name: format!("{MARK} event"),
        when: "20:00 – 23:00".into(),
        seats_left: Some(4),
    };

    assert!(claim(&db, &announced).await, "announcement blocked");
    assert!(
        claim(&db, &soon).await,
        "the reminder was suppressed by the announcement — the post with the \
         seats left in it is the one that never goes out"
    );
}

/// The watermark is what stops a shop with a full catalogue waking up to a
/// message per product.
///
/// A row created *before* the agent started watching is not news; one created
/// after is. This is the whole difference between a promoter and a spammer.
#[tokio::test]
#[ignore]
async fn only_what_appeared_after_the_agent_started_watching_counts() {
    let Some((_app, db)) = make_app_with_db().await else {
        return;
    };
    clean(&db).await;

    // Something that has been in the shop for a month.
    db.orm
        .execute(Statement::from_string(
            DbBackend::Postgres,
            format!(
                "INSERT INTO strains (id, name, price_per_gram, is_available, created_at) \
                 VALUES ('promo-old', '{MARK} old', 300, TRUE, NOW() - INTERVAL '30 days')"
            ),
        ))
        .await
        .expect("seed an old row");

    // The moment the agent starts watching.
    let watermark = chrono::Utc::now();

    // And something added afterwards.
    db.orm
        .execute(Statement::from_string(
            DbBackend::Postgres,
            format!(
                "INSERT INTO strains (id, name, price_per_gram, is_available, created_at) \
                 VALUES ('promo-new', '{MARK} new', 300, TRUE, NOW() + INTERVAL '1 second')"
            ),
        ))
        .await
        .expect("seed a new row");

    let rows = db
        .orm
        .query_all(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT name FROM strains WHERE created_at > $1 AND is_available = TRUE \
             AND name LIKE 'promo-agent-test%'",
            [watermark.into()],
        ))
        .await
        .expect("scan");

    let names: Vec<String> = rows
        .iter()
        .filter_map(|r| r.try_get::<String>("", "name").ok())
        .collect();

    assert!(
        names.iter().any(|n| n.ends_with("new")),
        "the newly added strain was not found: {names:?}"
    );
    assert!(
        !names.iter().any(|n| n.ends_with("old")),
        "a month-old product was treated as news — the first tick would message \
         the owner about the entire catalogue: {names:?}"
    );
}

/// A draft is not a publication.
///
/// `published_at` starts NULL and only a human pressing the button sets it.
/// The distinction has to survive in the schema, because "written" and "sent
/// to customers" are the two states this whole design rests on.
#[tokio::test]
#[ignore]
async fn a_draft_starts_unpublished() {
    let Some((_app, db)) = make_app_with_db().await else {
        return;
    };
    clean(&db).await;

    let s = Subject::Strain {
        id: "promo-test-draft".into(),
        name: format!("{MARK} draft"),
    };
    claim(&db, &s).await;

    let row = db
        .orm
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT published_at, published_by, source FROM promo_posts WHERE dedup_key = $1",
            [dedup_key(&s).into()],
        ))
        .await
        .expect("query")
        .expect("the claim wrote no row");

    let published: Option<chrono::DateTime<chrono::FixedOffset>> =
        row.try_get("", "published_at").ok().flatten();
    assert!(
        published.is_none(),
        "a freshly written draft is already marked published"
    );
    let by: Option<i64> = row.try_get("", "published_by").ok().flatten();
    assert!(by.is_none(), "a draft nobody published has a publisher");
    // And the origin of the text is recorded rather than left to be inferred.
    let source: String = row.try_get("", "source").unwrap_or_default();
    assert_eq!(
        source, "fallback",
        "the default source should say the written copy wrote it, since \
         GLM_API_KEY is unset in production"
    );
}

/// Muting is per owner and survives a restart.
///
/// A button that says "больше не присылаю" and forgets on the next deploy is
/// worse than no button: the owner stops trusting the switch and starts
/// ignoring the messages instead, which is the same silence with none of the
/// signal.
#[tokio::test]
#[ignore]
async fn muting_is_per_owner_and_persists() {
    let Some((_app, db)) = make_app_with_db().await else {
        return;
    };
    let (quiet, loud) = (975_001i64, 975_002i64);
    for id in [quiet, loud] {
        db.orm
            .execute(Statement::from_sql_and_values(
                DbBackend::Postgres,
                "DELETE FROM promo_muted WHERE telegram_id = $1",
                [id.into()],
            ))
            .await
            .expect("clean");
    }

    db.orm
        .execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "INSERT INTO promo_muted (telegram_id) VALUES ($1) ON CONFLICT DO NOTHING",
            [quiet.into()],
        ))
        .await
        .expect("mute");

    let is_muted = |id: i64| {
        let db = &db;
        async move {
            db.orm
                .query_one(Statement::from_sql_and_values(
                    DbBackend::Postgres,
                    "SELECT 1 AS x FROM promo_muted WHERE telegram_id = $1",
                    [id.into()],
                ))
                .await
                .map(|r| r.is_some())
                .unwrap_or(false)
        }
    };

    assert!(
        is_muted(quiet).await,
        "the owner who muted it still gets drafts"
    );
    assert!(
        !is_muted(loud).await,
        "one owner muting the promoter silenced another"
    );

    // Pressing twice is ordinary and must not error.
    db.orm
        .execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "INSERT INTO promo_muted (telegram_id) VALUES ($1) ON CONFLICT DO NOTHING",
            [quiet.into()],
        ))
        .await
        .expect("muting twice must be harmless");
}
