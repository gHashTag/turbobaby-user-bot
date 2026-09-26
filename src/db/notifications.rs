//! Notification queue helpers for referral lifecycle events.
//!
//! Loop #21: instead of fire-and-forget Telegram sends, all referrer-facing
//! lifecycle messages (friend joined / friend ordered / milestone) are
//! appended to `notification_queue` and delivered by the background worker in
//! `src/notification_queue.rs`. This gives retry, idempotency (via worker
//! marking rows `processed_at`), and decouples slow Telegram calls from the
//! request path.

use anyhow::{Context, Result};
use sea_orm::ActiveValue::Set;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use serde_json::json;

/// Queue a "your friend accepted the invite" notification.
pub(crate) async fn enqueue_friend_joined(
    orm: &sea_orm::DatabaseConnection,
    referrer_id: i64,
    referred_name: &str,
) -> Result<()> {
    let payload = json!({
        "referred_name": referred_name,
    });
    insert_queue_row(orm, referrer_id, "friend_joined", payload).await
}

// `enqueue_friend_watered` stood here. Watering was a garden action (D5), so
// nothing can produce that event any more and the writer was dead code.
//
// Its reader went on 2026-09-26: `build_message` in src/notification_queue.rs
// no longer has a `friend_watered` arm (owner: nothing cannabis-related
// anywhere, 2026-09-25). Rows already stored are data and stay as they are --
// nothing is deleted (2026-09-24) -- so they are HELD instead: the scan below
// is handed only the kinds the worker delivers, and never reads them. The
// writer is code, the rows are data, and neither outlives the ruling now.

/// Queue a "your friend placed their first order" notification.
pub(crate) async fn enqueue_friend_ordered(
    orm: &sea_orm::DatabaseConnection,
    referrer_id: i64,
    referred_name: &str,
    bonus: f64,
) -> Result<()> {
    let payload = json!({
        "referred_name": referred_name,
        "bonus": bonus,
    });
    insert_queue_row(orm, referrer_id, "friend_ordered", payload).await
}

/// Queue a referral milestone award notification.
pub(crate) async fn enqueue_milestone(
    orm: &sea_orm::DatabaseConnection,
    referrer_id: i64,
    milestone: i32,
    bonus_amount: f64,
) -> Result<()> {
    let payload = json!({
        "milestone": milestone,
        "bonus_amount": bonus_amount,
    });
    insert_queue_row(orm, referrer_id, "milestone", payload).await
}

async fn insert_queue_row(
    orm: &sea_orm::DatabaseConnection,
    telegram_id: i64,
    kind: &str,
    payload: serde_json::Value,
) -> Result<()> {
    use crate::db::entities::notification_queue::{ActiveModel, Entity};
    let am = ActiveModel {
        id: Set(uuid::Uuid::new_v4()),
        telegram_id: Set(telegram_id),
        kind: Set(kind.to_string()),
        payload: Set(payload),
        scheduled_at: Set(chrono::Utc::now().into()),
        processed_at: Set(None),
        attempts: Set(0),
        created_at: Set(chrono::Utc::now().into()),
    };
    Entity::insert(am)
        .exec(orm)
        .await
        .context("insert notification_queue row")?;
    crate::metrics::notification_queued(kind);
    Ok(())
}

/// Up to `limit` pending notifications of a kind in `kinds`, oldest first.
///
/// `max_attempts` arrives from the caller instead of being written here again.
/// This filter and the drain's give-up branch are the same bound seen from two
/// sides, and the literal `3` that used to stand in it was a second copy of
/// `MAX_ATTEMPTS` (src/notification_queue.rs): the day one moved, rows would
/// have been withheld from the drain without ever being abandoned by it, which
/// looks exactly like a queue that quietly forgets messages.
///
/// `kinds` arrives the same way, for the same reason: it is the drain's list
/// (`DeliverableKind::names()`), and the scan must not keep a copy. A row of
/// any other kind is never handed out, so it is never sent, counted or marked
/// and holds no slot at the head of the queue (owner rulings of 2026-09-24
/// and 2026-09-25; specs/turbobaby/notification_queue.t27).
#[allow(dead_code)] // Consumed by the binary-only notification worker.
pub(crate) async fn pending_notifications(
    orm: &sea_orm::DatabaseConnection,
    limit: usize,
    max_attempts: i32,
    kinds: &[&str],
) -> Result<Vec<crate::db::entities::notification_queue::Model>> {
    let rows = pending_scan(limit, max_attempts, kinds)
        .all(orm)
        .await
        .context("fetch pending notifications")?;
    Ok(rows)
}

/// The scan `pending_notifications` runs, built apart from running it so a
/// test can read the SQL without a database.
fn pending_scan(
    limit: usize,
    max_attempts: i32,
    kinds: &[&str],
) -> sea_orm::Select<crate::db::entities::notification_queue::Entity> {
    use crate::db::entities::notification_queue::{Column, Entity};
    Entity::find()
        .filter(Column::ProcessedAt.is_null())
        .filter(Column::Attempts.lt(max_attempts))
        .filter(Column::Kind.is_in(kinds.iter().copied()))
        .order_by_asc(Column::ScheduledAt)
        .limit(Some(limit as u64))
}

/// Mark a notification as delivered.
#[allow(dead_code)] // Consumed by the binary-only notification worker.
pub(crate) async fn mark_delivered(
    orm: &sea_orm::DatabaseConnection,
    id: uuid::Uuid,
) -> Result<()> {
    use crate::db::entities::notification_queue::{ActiveModel, Column, Entity};
    let am = ActiveModel {
        id: Set(id),
        processed_at: Set(Some(chrono::Utc::now().into())),
        ..Default::default()
    };
    Entity::update_many()
        .set(am)
        .filter(Column::Id.eq(id))
        .exec(orm)
        .await
        .context("mark notification delivered")?;
    Ok(())
}

/// Increment attempts for a failed delivery.
#[allow(dead_code)] // Consumed by the binary-only notification worker.
pub(crate) async fn increment_attempts(
    orm: &sea_orm::DatabaseConnection,
    id: uuid::Uuid,
) -> Result<()> {
    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    orm.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "UPDATE notification_queue SET attempts = attempts + 1 WHERE id = $1",
        [id.into()],
    ))
    .await
    .context("increment notification attempts")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    //! The scan is where a held row is kept from the drain, so it is read two
    //! ways: as SQL, which needs nothing, and against a throwaway Postgres,
    //! which is ignored by default. Run the second with:
    //! ```sh
    //! DATABASE_URL=postgres://…/<throwaway db> cargo test --features backend --lib \
    //!   db::notifications::tests -- --ignored --test-threads=1
    //! ```
    use super::*;
    use sea_orm::{DbBackend, QueryTrait};

    /// The kinds the drain hands the scan (`DeliverableKind::names()` in
    /// src/notification_queue.rs), written out here because this crate cannot
    /// see the binary's module. `tests/notification_drain_wiring.rs` holds the
    /// drain's list, the producers' kinds and the contract's to one another.
    const DELIVERED: [&str; 3] = ["friend_joined", "friend_ordered", "milestone"];

    #[test]
    fn the_scan_selects_only_the_kinds_it_is_handed() {
        let sql = pending_scan(50, 3, &DELIVERED)
            .build(DbBackend::Postgres)
            .to_string();
        assert!(sql.starts_with("SELECT "), "the scan writes: {sql}");
        for part in [
            r#""notification_queue"."processed_at" IS NULL"#,
            r#""notification_queue"."attempts" < 3"#,
            r#""notification_queue"."kind" IN ('friend_joined', 'friend_ordered', 'milestone')"#,
            r#"ORDER BY "notification_queue"."scheduled_at" ASC"#,
            "LIMIT 50",
        ] {
            assert!(sql.contains(part), "the scan lost `{part}`: {sql}");
        }
        assert!(
            !sql.contains("friend_watered"),
            "the retired garden's kind is handed out: {sql}"
        );
    }

    /// A throwaway database, migrated the way `tests/common` migrates one.
    /// `None` only when no database was named: one that was named and cannot
    /// be reached or migrated fails the test instead of skipping it.
    async fn db() -> Option<sea_orm::DatabaseConnection> {
        let url = std::env::var("DATABASE_URL").ok()?;
        assert!(
            url.contains("localhost") || url.contains("127.0.0.1") || url.contains("test"),
            "refusing to run against {url:?}: this test writes rows"
        );
        let db = crate::db::Database::connect(&url)
            .await
            .expect("connect to the DATABASE_URL this test was pointed at");
        db.run_migrations()
            .await
            .expect("migrate the test database");
        Some(db.orm)
    }

    /// On Postgres: a held row is never handed out, and the scan leaves it
    /// exactly as stored. Rows are inserted under a chat id of their own and
    /// nothing is deleted afterwards; a held row is simply never read again.
    #[tokio::test]
    #[ignore]
    async fn a_held_row_is_never_handed_out_and_stays_as_stored() {
        let Some(orm) = db().await else {
            eprintln!("DATABASE_URL unset — skipping");
            return;
        };
        use crate::db::entities::notification_queue::{Column, Entity};

        let chat = 7_000_000_000_i64 + (uuid::Uuid::new_v4().as_u128() % 1_000_000_000) as i64;
        for kind in [
            "friend_joined",
            "friend_watered",
            "milestone",
            "garden_water_reminder",
        ] {
            insert_queue_row(&orm, chat, kind, json!({ "referred_name": "Held" }))
                .await
                .expect("insert a queue row");
        }

        let mut handed: Vec<String> = pending_notifications(&orm, 100_000, 3, &DELIVERED)
            .await
            .expect("scan")
            .into_iter()
            .filter(|row| row.telegram_id == chat)
            .map(|row| row.kind)
            .collect();
        handed.sort();
        assert_eq!(handed, ["friend_joined", "milestone"]);

        let held = Entity::find()
            .filter(Column::TelegramId.eq(chat))
            .filter(Column::Kind.is_not_in(DELIVERED))
            .all(&orm)
            .await
            .expect("read the held rows back");
        assert_eq!(held.len(), 2, "a held row went missing: {held:?}");
        for row in held {
            assert!(row.processed_at.is_none(), "{} was marked", row.kind);
            assert_eq!(row.attempts, 0, "{} was counted", row.kind);
        }
    }
}
