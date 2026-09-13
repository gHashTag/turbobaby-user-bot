//! The friends panel called everybody "Friend #6794".
//!
//! Not a formatting choice. `user_languages.first_name` was NULL for
//! essentially every user, because the only function that wrote it —
//! `save_user_name` — had no callers anywhere in the codebase. The name was
//! never recorded, so `COALESCE(first_name, 'Friend')` did the only thing an
//! empty column allows, and a panel meant to show your friends showed a list of
//! identical strangers.
//!
//! This walks the real path: record an identity the way the bot now does on
//! every command, then ask the endpoint the garden asks, and require the name
//! and the handle to come back.
//!
//! Run with:
//! ```sh
//! DATABASE_URL=postgres://postgres:postgres@127.0.0.1:5432/woody_test \
//!   cargo test --features backend --test integration_invitee_names -- --ignored --test-threads=1
//! ```

#![cfg(feature = "backend")]

mod common;

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use common::{make_app_with_db, make_init_data};
use sea_orm::{ConnectionTrait, DbBackend, Statement};
use tower::ServiceExt;

const REFERRER: i64 = 971_101;
const NAMED: i64 = 971_102;
const HANDLE_ONLY: i64 = 971_103;
const UNKNOWN: i64 = 976_794;

async fn invitees(app: &axum::Router, who: i64) -> Vec<serde_json::Value> {
    let req = Request::builder()
        .method(Method::GET)
        .uri(format!("/api/referrals/me/{who}/invitees"))
        .header(
            "X-Telegram-Init-Data",
            make_init_data(who, "dummy_test_token"),
        )
        .body(Body::empty())
        .expect("request");
    let resp = app.clone().oneshot(req).await.expect("response");
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), 1 << 20)
        .await
        .expect("body");
    assert_eq!(
        status,
        StatusCode::OK,
        "GET invitees: {}",
        String::from_utf8_lossy(&bytes)
    );
    let v: serde_json::Value = serde_json::from_slice(&bytes).expect("json");
    v["invitees"].as_array().cloned().unwrap_or_default()
}

/// Find a row by the handle it should carry, or by its fallback suffix when it
/// should have none. Returns `None` rather than panicking so the assertion
/// that follows can say what was actually there.
fn row_with<'a>(list: &'a [serde_json::Value], needle: &str) -> Option<&'a serde_json::Value> {
    list.iter()
        .find(|r| r["display_name"].as_str().unwrap_or("").contains(needle))
}

async fn clean(db: &turbobaby_bot::db::Database) {
    for id in [REFERRER, NAMED, HANDLE_ONLY, UNKNOWN] {
        for sql in [
            "DELETE FROM referral_events WHERE referrer_id = $1 OR referred_id = $1",
            "DELETE FROM user_languages WHERE telegram_id = $1",
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
}

async fn follows_the_link(db: &turbobaby_bot::db::Database, friend: i64) {
    db.orm
        .execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "INSERT INTO referral_events (id, referrer_id, referred_id, code, status, source, created_at) \
             VALUES (gen_random_uuid(), $1, $2, 'NAMETEST', 'pending', 'telegram_start', NOW())",
            [REFERRER.into(), friend.into()],
        ))
        .await
        .expect("record a followed link");
}

/// The reported screen, end to end.
///
/// Three friends in the three states the naming rule has, so a fix that only
/// handled the happy one would fail here.
#[tokio::test]
#[ignore]
async fn friends_are_shown_by_name_and_handle() {
    let Some((app, db)) = make_app_with_db().await else {
        eprintln!("DATABASE_URL unset — skipping");
        return;
    };
    clean(&db).await;

    // What the bot now does on every command it receives.
    db.save_user_identity(NAMED, Some("Дмитрий"), Some("Васильев"), Some("woody_dev"))
        .await
        .expect("record a full identity");
    db.save_user_identity(HANDLE_ONLY, None, None, Some("just_a_handle"))
        .await
        .expect("record a handle with no name");
    // UNKNOWN gets nothing — nobody ever saw them.

    for friend in [NAMED, HANDLE_ONLY, UNKNOWN] {
        follows_the_link(&db, friend).await;
    }

    let list = invitees(&app, REFERRER).await;
    assert_eq!(list.len(), 3, "expected three invitees, got {list:?}");

    // A person with a name is printed by it, with the handle carried
    // separately so the row can style it.
    let named = row_with(&list, "Дмитрий").unwrap_or_else(|| {
        panic!("the named friend was not printed by name; the panel showed {list:?}")
    });
    assert_eq!(
        named["display_name"].as_str(),
        Some("Дмитрий Васильев @woody_dev")
    );
    assert_eq!(named["username"].as_str(), Some("woody_dev"));
    assert_eq!(named["is_anonymous"].as_bool(), Some(false));
    // The assertion that discriminates the fix from the defect.
    assert!(
        !named["display_name"]
            .as_str()
            .unwrap_or("")
            .contains("Friend"),
        "a friend with a recorded name was still printed as a stranger: {named}"
    );

    // A handle with no name is a person, not an anonymous one.
    let handle_only = row_with(&list, "just_a_handle")
        .unwrap_or_else(|| panic!("the handle-only friend vanished: {list:?}"));
    assert_eq!(handle_only["display_name"].as_str(), Some("@just_a_handle"));
    assert_eq!(handle_only["is_anonymous"].as_bool(), Some(false));

    // And somebody genuinely unknown still says so, and is still told apart
    // from the next unknown person. This is the state that must survive: the
    // fix must not invent a name for somebody nobody has ever named.
    let unknown = row_with(&list, "#6794")
        .unwrap_or_else(|| panic!("the unknown friend lost their suffix: {list:?}"));
    assert_eq!(unknown["is_anonymous"].as_bool(), Some(true));
    assert!(
        unknown["username"].is_null(),
        "a person with no handle reported one: {unknown}"
    );
}

/// Telegram omits `username` for people who have not set one and omits
/// `last_name` for most. A later update that carries less must not erase what
/// an earlier one recorded — otherwise a friend is named once and then
/// silently un-named by their next message.
#[tokio::test]
#[ignore]
async fn a_later_update_that_knows_less_does_not_erase_a_name() {
    let Some((app, db)) = make_app_with_db().await else {
        return;
    };
    clean(&db).await;

    db.save_user_identity(NAMED, Some("Дмитрий"), Some("Васильев"), Some("woody_dev"))
        .await
        .expect("first sighting");
    // The same person sends another message; this time Telegram gives only the
    // first name, which is the common case.
    db.save_user_identity(NAMED, Some("Дмитрий"), None, None)
        .await
        .expect("second sighting");

    follows_the_link(&db, NAMED).await;
    let list = invitees(&app, REFERRER).await;
    let row = row_with(&list, "Дмитрий")
        .unwrap_or_else(|| panic!("the friend lost their name entirely: {list:?}"));

    assert_eq!(
        row["username"].as_str(),
        Some("woody_dev"),
        "the handle was erased by an update that simply did not mention it: {row}"
    );
    assert_eq!(
        row["display_name"].as_str(),
        Some("Дмитрий Васильев @woody_dev"),
        "the surname was erased by an update that simply did not mention it: {row}"
    );
}

/// A handle is rendered with an `@` glued to the front. Anything that is not a
/// Telegram handle must not be stored as one, or the panel prints something
/// that looks tappable and is not.
#[tokio::test]
#[ignore]
async fn junk_is_not_stored_as_a_handle() {
    let Some((_app, db)) = make_app_with_db().await else {
        return;
    };
    clean(&db).await;

    db.save_user_identity(NAMED, Some("Дмитрий"), None, Some("not a handle at all"))
        .await
        .expect("record");

    let row = db
        .orm
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT username, first_name FROM user_languages WHERE telegram_id = $1",
            [NAMED.into()],
        ))
        .await
        .expect("query")
        .expect("the row was not written at all");

    let stored: Option<String> = row.try_get("", "username").ok().flatten();
    assert_eq!(
        stored, None,
        "free text was stored as a Telegram handle: {stored:?}"
    );
    // ...and the name it arrived with was still recorded, so one bad field
    // does not discard the whole sighting.
    let name: Option<String> = row.try_get("", "first_name").ok().flatten();
    assert_eq!(name.as_deref(), Some("Дмитрий"));
}
