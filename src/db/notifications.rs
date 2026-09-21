//! Notification queue helpers for referral lifecycle events.
//!
//! Loop #21: instead of fire-and-forget Telegram sends, all referrer-facing
//! lifecycle messages (friend joined / watered / ordered / milestone) are
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
// Its reader was NOT deleted: `render_referral_text` in
// src/notification_queue.rs still has a `"friend_watered"` arm. The queue is a
// Postgres table, and a deployed database can hold rows written before the
// mechanic was removed. Deleting the arm would make those rows render as the
// fallback text instead of the message they were queued for. Writer and reader
// have different lifetimes here: the writer is code, the rows are data.

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

/// Up to `limit` pending notifications, oldest first.
///
/// `max_attempts` arrives from the caller instead of being written here again.
/// This filter and the drain's give-up branch are the same bound seen from two
/// sides, and the literal `3` that used to stand in it was a second copy of
/// `MAX_ATTEMPTS` (src/notification_queue.rs): the day one moved, rows would
/// have been withheld from the drain without ever being abandoned by it, which
/// looks exactly like a queue that quietly forgets messages.
#[allow(dead_code)] // Consumed by the binary-only notification worker.
pub(crate) async fn pending_notifications(
    orm: &sea_orm::DatabaseConnection,
    limit: usize,
    max_attempts: i32,
) -> Result<Vec<crate::db::entities::notification_queue::Model>> {
    use crate::db::entities::notification_queue::{Column, Entity};
    let rows = Entity::find()
        .filter(Column::ProcessedAt.is_null())
        .filter(Column::Attempts.lt(max_attempts))
        .order_by_asc(Column::ScheduledAt)
        .limit(Some(limit as u64))
        .all(orm)
        .await
        .context("fetch pending notifications")?;
    Ok(rows)
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
