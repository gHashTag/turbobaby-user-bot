//! Can one date hold several events?
//!
//! The reported requirement: on the 25th, one event runs 09:00–12:00, another
//! 15:00–17:00, and a third or fourth may be added later. The customer opens
//! the date, sees everything happening that day, and picks one.
//!
//! Before changing anything, this establishes what the shipped code actually
//! does — because nothing found by reading it forbids the second event. There
//! is no unique constraint on `events`, `POST /api/admin/events` is a plain
//! `INSERT`, and the calendar screen filters the whole list by day into a
//! `Vec`. Either the capability is already there and the obstacle is somewhere
//! else, or this test fails and names it.
//!
//! Each event keeps its own title, start, end, description, image and booking,
//! so those are asserted per event rather than counted — three rows sharing one
//! title and image would satisfy a count and would not be the feature.
//!
//! Run with:
//! ```sh
//! DATABASE_URL=postgres://postgres:postgres@127.0.0.1:5432/woody_test \
//!   cargo test --features backend --test integration_events_same_day -- --ignored --test-threads=1
//! ```

#![cfg(feature = "backend")]

mod common;

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use common::make_app_with_db;
use sea_orm::{ConnectionTrait, DbBackend, Statement};
use tower::ServiceExt;

/// A date far enough out that the shop's real calendar cannot collide with it.
const DAY: &str = "2031-03-25";

/// The three events from the report, plus a fourth to show the rule is not
/// "two". Times are deliberately out of order so an ordering assertion means
/// something.
const SCHEDULE: [(&str, &str, &str, &str); 4] = [
    ("Afternoon set", "15:00:00", "17:00:00", "the second one"),
    ("Morning yoga", "09:00:00", "12:00:00", "the first one"),
    ("Late DJ", "21:00:00", "23:30:00", "the fourth one"),
    ("Evening tasting", "18:00:00", "19:30:00", "the third one"),
];

async fn get_events(app: &axum::Router) -> Vec<serde_json::Value> {
    let req = Request::builder()
        .method(Method::GET)
        .uri("/api/events")
        .body(Body::empty())
        .expect("request");
    let resp = app.clone().oneshot(req).await.expect("response");
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), 1 << 22)
        .await
        .expect("body");
    assert_eq!(
        status,
        StatusCode::OK,
        "GET /api/events: {}",
        String::from_utf8_lossy(&bytes)
    );
    let v: serde_json::Value = serde_json::from_slice(&bytes).expect("json");
    v["events"].as_array().cloned().unwrap_or_default()
}

/// The shop's timezone. `starts_at` goes out as RFC 3339 in UTC, and the
/// screen converts to Bangkok before taking the date — so a 01:00 event is on
/// the day the customer thinks it is, not the UTC day before. Comparing the
/// UTC string prefix instead is how this test first "failed" against correct
/// code: 15:00+07 is 08:00Z.
fn bangkok() -> chrono::FixedOffset {
    chrono::FixedOffset::east_opt(7 * 3600).expect("valid offset")
}

/// Read `starts_at` the way `parse_event_start` does.
fn local_start(e: &serde_json::Value) -> Option<chrono::DateTime<chrono::FixedOffset>> {
    e["starts_at"]
        .as_str()?
        .parse::<chrono::DateTime<chrono::Utc>>()
        .ok()
        .map(|dt| dt.with_timezone(&bangkok()))
}

/// Everything the API reports for one calendar day, grouped exactly as the
/// screen groups it.
fn on_day<'a>(all: &'a [serde_json::Value], day: &str) -> Vec<&'a serde_json::Value> {
    let want: chrono::NaiveDate = day.parse().expect("a valid date");
    all.iter()
        .filter(|e| {
            local_start(e)
                .map(|dt| dt.date_naive() == want)
                .unwrap_or(false)
        })
        .collect()
}

async fn clean(db: &turbobaby_bot::db::Database) {
    db.orm
        .execute(Statement::from_string(
            DbBackend::Postgres,
            format!("DELETE FROM events WHERE starts_at::date = DATE '{DAY}'"),
        ))
        .await
        .expect("clean slate");
}

/// Insert the way `POST /api/admin/events` does — the same columns, the same
/// table — without needing an admin session in a test.
async fn add_event(
    db: &turbobaby_bot::db::Database,
    title: &str,
    start: &str,
    end: &str,
    description: &str,
) {
    db.orm
        .execute(Statement::from_string(
            DbBackend::Postgres,
            format!(
                "INSERT INTO events \
                 (id, title, description, starts_at, ends_at, image_url, is_public) \
                 VALUES (gen_random_uuid(), '{title}', '{description}', \
                         TIMESTAMPTZ '{DAY} {start}+07', TIMESTAMPTZ '{DAY} {end}+07', \
                         'https://example.test/{start}.jpg', TRUE)"
            ),
        ))
        .await
        .unwrap_or_else(|e| panic!("the database refused a second event on {DAY}: {e}"));
}

#[tokio::test]
#[ignore]
async fn one_date_holds_four_events_each_with_its_own_everything() {
    let Some((app, db)) = make_app_with_db().await else {
        eprintln!("DATABASE_URL unset — skipping");
        return;
    };
    clean(&db).await;

    for (title, start, end, description) in SCHEDULE {
        add_event(&db, title, start, end, description).await;
    }

    let all = get_events(&app).await;
    let day = on_day(&all, DAY);

    assert_eq!(
        day.len(),
        SCHEDULE.len(),
        "the API reported {} of {} events on {DAY}: {day:#?}",
        day.len(),
        SCHEDULE.len()
    );

    // Each one keeps its own fields. A count alone would pass on four rows
    // that are copies of each other, which is not the feature asked for.
    for (title, start, end, description) in SCHEDULE {
        let found = day
            .iter()
            .find(|e| e["title"].as_str() == Some(title))
            .unwrap_or_else(|| panic!("{title:?} is missing from {DAY}: {day:#?}"));

        let got_start = local_start(found)
            .unwrap_or_else(|| panic!("{title:?} has an unreadable start: {found:#?}"));
        assert_eq!(
            got_start.format("%H:%M:%S").to_string(),
            start,
            "{title:?} lost its start time"
        );
        let got_end = found["ends_at"]
            .as_str()
            .and_then(|s| s.parse::<chrono::DateTime<chrono::Utc>>().ok())
            .map(|dt| dt.with_timezone(&bangkok()))
            .unwrap_or_else(|| panic!("{title:?} has no end time: {found:#?}"));
        assert_eq!(
            got_end.format("%H:%M:%S").to_string(),
            end,
            "{title:?} lost its end time"
        );
        assert_eq!(
            found["description"].as_str(),
            Some(description),
            "{title:?} has another event's description"
        );
        assert!(
            found["image_url"]
                .as_str()
                .unwrap_or_default()
                .contains(&start[..2]),
            "{title:?} shows another event's image: {found:#?}"
        );
        assert!(
            found["id"].as_str().map(|s| !s.is_empty()).unwrap_or(false),
            "{title:?} has no id, so it cannot be opened or booked"
        );
    }

    // Four distinct ids: the customer must be able to open one of them, not a
    // merged pseudo-event.
    let ids: std::collections::BTreeSet<&str> =
        day.iter().filter_map(|e| e["id"].as_str()).collect();
    assert_eq!(ids.len(), SCHEDULE.len(), "events share an id: {ids:?}");
}

/// The day's events must arrive in chronological order — a schedule listed
/// 15:00, 09:00, 21:00, 18:00 is not a schedule.
#[tokio::test]
#[ignore]
async fn the_day_reads_in_time_order() {
    let Some((app, db)) = make_app_with_db().await else {
        return;
    };
    clean(&db).await;
    for (title, start, end, description) in SCHEDULE {
        add_event(&db, title, start, end, description).await;
    }

    let all = get_events(&app).await;
    let starts: Vec<String> = on_day(&all, DAY)
        .iter()
        .filter_map(|e| e["starts_at"].as_str().map(str::to_string))
        .collect();

    let mut sorted = starts.clone();
    sorted.sort();
    assert_eq!(
        starts, sorted,
        "the day is out of order, so a customer reads the afternoon set first: {starts:?}"
    );
}

/// Opening one event by its own id must return that event and no other — the
/// step after "pick the one you want".
#[tokio::test]
#[ignore]
async fn each_event_opens_on_its_own() {
    let Some((app, db)) = make_app_with_db().await else {
        return;
    };
    clean(&db).await;
    for (title, start, end, description) in SCHEDULE {
        add_event(&db, title, start, end, description).await;
    }

    for e in on_day(&get_events(&app).await, DAY) {
        let id = e["id"].as_str().expect("id");
        let title = e["title"].as_str().expect("title");

        let req = Request::builder()
            .method(Method::GET)
            .uri(format!("/api/events/{id}"))
            .body(Body::empty())
            .expect("request");
        let resp = app.clone().oneshot(req).await.expect("response");
        assert_eq!(resp.status(), StatusCode::OK, "GET /api/events/{id}");
        let bytes = axum::body::to_bytes(resp.into_body(), 1 << 20)
            .await
            .expect("body");
        let one: serde_json::Value = serde_json::from_slice(&bytes).expect("json");
        let got = one.get("event").unwrap_or(&one);

        assert_eq!(
            got["title"].as_str(),
            Some(title),
            "opening {id} returned a different event on the same day"
        );
    }
}

/// The write side, through the handler the admin form actually calls.
///
/// The read side above uses SQL to set up state; this one does not, because
/// the question is precisely whether `POST /api/admin/events` refuses the
/// second event of a day. If a limit exists anywhere, it is here.
#[tokio::test]
#[ignore]
async fn the_admin_endpoint_accepts_four_events_on_one_date() {
    let Some((app, db)) = make_app_with_db().await else {
        return;
    };
    clean(&db).await;

    for (n, (title, start, end, description)) in SCHEDULE.iter().enumerate() {
        let body = serde_json::json!({
            "title": title,
            "description": description,
            "starts_at": format!("{DAY}T{start}+07:00"),
            "ends_at": format!("{DAY}T{end}+07:00"),
            "image_url": format!("https://example.test/{start}.jpg"),
            "is_public": true,
        });
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/admin/events")
                    .method("POST")
                    .header(
                        "X-Telegram-Init-Data",
                        common::make_init_data(42, "dummy_test_token"),
                    )
                    .header("X-Admin-Token", "test_password")
                    .header("X-Admin-Telegram-Id", "42")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .expect("request"),
            )
            .await
            .expect("response");
        let status = resp.status();
        let bytes = axum::body::to_bytes(resp.into_body(), 1 << 20)
            .await
            .expect("body");
        assert_eq!(
            status,
            StatusCode::OK,
            "event #{} on {DAY} ({title}) was refused: {}",
            n + 1,
            String::from_utf8_lossy(&bytes)
        );
    }

    let all = get_events(&app).await;
    let day = on_day(&all, DAY);
    assert_eq!(
        day.len(),
        SCHEDULE.len(),
        "created {} events on {DAY} and the calendar shows {}",
        SCHEDULE.len(),
        day.len()
    );
    // And each is its own row rather than the last create having overwritten
    // the earlier ones, which is what a one-per-day rule would look like.
    let titles: std::collections::BTreeSet<&str> =
        day.iter().filter_map(|e| e["title"].as_str()).collect();
    assert_eq!(titles.len(), SCHEDULE.len(), "titles collapsed: {titles:?}");
}
