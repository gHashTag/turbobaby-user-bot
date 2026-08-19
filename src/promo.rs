//! The promoter: finds what is new, writes about it, hands it to the owners.
//!
//! Runs on the same interval pattern as the other sweeps in `main`. Every tick
//! it asks four questions — is there a strain, an accessory or a tea nobody has
//! posted about; a set; an event; an event happening tomorrow — writes a post
//! for each answer, and sends it to the admins with a **Publish** button.
//!
//! Nothing here posts publicly. The button does, and only when a human presses
//! it. That is deliberate: the text is written by a model, and a mistake under
//! the shop's name reaches subscribers before it reaches the owner.
//!
//! **A tick that finds nothing still says so.** Both garden sweeps in this
//! project were broken for months precisely because a silent loop and a stopped
//! loop look identical in the log.
//!
//! ## Why it only looks forward
//!
//! `since` is the moment the agent started watching — the newest `drafted_at`
//! in `promo_posts`, or the process start on the very first run. A shop with
//! 137 products must not wake up to 137 drafts, and "everything I have not
//! covered" would mean exactly that.

use crate::config::Config;
use crate::db::Database;
use crate::trios::promo::{self, Subject};
use sea_orm::{ConnectionTrait, DbBackend, Statement};
use std::sync::Arc;
use teloxide::prelude::*;
use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup};

/// How many drafts one tick may produce.
///
/// A bulk catalogue import would otherwise send one message per row, and the
/// owner's phone is not a place to discover that. The rest are picked up on
/// following ticks; nothing is dropped, only spread out. If a tick hits the cap
/// it says so, because a silent cap reads as "that was everything".
const MAX_DRAFTS_PER_TICK: usize = 5;

/// The prefix on the Publish button's callback data.
pub const PUBLISH_CALLBACK: &str = "promo_pub:";

/// Find, write, and hand over. One tick.
pub async fn sweep(db: Arc<Database>, bot: Bot, config: Arc<Config>, ai: Arc<crate::ai::AiClient>) {
    let since = match watching_since(&db).await {
        Ok(t) => t,
        Err(e) => {
            tracing::warn!("promo: could not read the watermark, skipping tick: {e}");
            return;
        }
    };

    let mut found: Vec<Subject> = Vec::new();
    match new_catalog_items(&db, &since).await {
        Ok(mut v) => found.append(&mut v),
        Err(e) => tracing::warn!("promo: catalog scan failed: {e}"),
    }
    match new_events(&db, &since).await {
        Ok(mut v) => found.append(&mut v),
        Err(e) => tracing::warn!("promo: event scan failed: {e}"),
    }
    match events_happening_tomorrow(&db).await {
        Ok(mut v) => found.append(&mut v),
        Err(e) => tracing::warn!("promo: reminder scan failed: {e}"),
    }

    if found.is_empty() {
        tracing::info!("promo: nothing new to promote (tick ok)");
        return;
    }

    let capped = found.len() > MAX_DRAFTS_PER_TICK;
    if capped {
        tracing::info!(
            found = found.len(),
            cap = MAX_DRAFTS_PER_TICK,
            "promo: more to promote than one tick sends; the rest follow next tick"
        );
        found.truncate(MAX_DRAFTS_PER_TICK);
    }

    let mut drafted = 0usize;
    for subject in found {
        // Claim it first. Writing the row before sending means a crash between
        // the two loses a post rather than repeating one — and a repeated post
        // is the failure the owner sees.
        match claim(&db, &subject).await {
            Ok(false) => continue, // somebody else took it
            Ok(true) => {}
            Err(e) => {
                tracing::warn!("promo: could not claim {}: {e}", promo::dedup_key(&subject));
                continue;
            }
        }

        let (body, source) = write_copy(&subject, &db, &ai).await;
        if let Err(e) = record_body(&db, &subject, &body, source).await {
            tracing::warn!("promo: could not record the body: {e}");
        }
        send_draft(&bot, &config, &subject, &body).await;
        drafted += 1;
    }

    tracing::info!(drafted, "promo: drafts sent to admins for review");
}

/// The moment the agent started watching.
async fn watching_since(db: &Database) -> Result<chrono::DateTime<chrono::Utc>, sea_orm::DbErr> {
    let row = db
        .orm
        .query_one(Statement::from_string(
            DbBackend::Postgres,
            "SELECT MAX(drafted_at) AS latest FROM promo_posts".to_string(),
        ))
        .await?;
    let latest: Option<chrono::DateTime<chrono::FixedOffset>> =
        row.and_then(|r| r.try_get("", "latest").ok()).flatten();
    Ok(latest
        .map(|t| t.with_timezone(&chrono::Utc))
        // First run ever: watch from now, not from the beginning of the shop.
        .unwrap_or_else(chrono::Utc::now))
}

/// New strains, accessories and teas.
async fn new_catalog_items(
    db: &Database,
    since: &chrono::DateTime<chrono::Utc>,
) -> Result<Vec<Subject>, sea_orm::DbErr> {
    let mut out = Vec::new();
    // Only what a customer could actually buy: an unavailable row is not news.
    for (table, avail) in [
        ("strains", "is_available"),
        ("accessories", "is_available"),
        ("tea_products", "is_available"),
    ] {
        let sql = format!(
            "SELECT id::text AS id, name FROM {table} \
             WHERE created_at > $1 AND {avail} = TRUE \
             ORDER BY created_at ASC LIMIT 20"
        );
        let rows = db
            .orm
            .query_all(Statement::from_sql_and_values(
                DbBackend::Postgres,
                &sql,
                [(*since).into()],
            ))
            .await?;
        for r in rows {
            let id: String = r.try_get("", "id").unwrap_or_default();
            let name: String = r.try_get("", "name").unwrap_or_default();
            if id.is_empty() || name.trim().is_empty() {
                continue;
            }
            out.push(match table {
                "strains" => Subject::Strain { id, name },
                "accessories" => Subject::Accessory { id, name },
                _ => Subject::Tea { id, name },
            });
        }
    }

    let rows = db
        .orm
        .query_all(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT id::text AS id, name FROM sets \
             WHERE created_at > $1 AND is_available = TRUE \
             ORDER BY created_at ASC LIMIT 20",
            [(*since).into()],
        ))
        .await?;
    for r in rows {
        let id: String = r.try_get("", "id").unwrap_or_default();
        let name: String = r.try_get("", "name").unwrap_or_default();
        if !id.is_empty() && !name.trim().is_empty() {
            out.push(Subject::Set { id, name });
        }
    }
    Ok(out)
}

/// Events announced since the watermark, and still in the future — announcing
/// an event that already happened is worse than saying nothing.
async fn new_events(
    db: &Database,
    since: &chrono::DateTime<chrono::Utc>,
) -> Result<Vec<Subject>, sea_orm::DbErr> {
    let rows = db
        .orm
        .query_all(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT id::text AS id, title FROM events \
             WHERE created_at > $1 AND is_public = TRUE AND starts_at > NOW() \
             ORDER BY created_at ASC LIMIT 20",
            [(*since).into()],
        ))
        .await?;
    Ok(rows
        .into_iter()
        .filter_map(|r| {
            let id: String = r.try_get("", "id").ok()?;
            let name: String = r.try_get("", "title").ok()?;
            (!id.is_empty() && !name.trim().is_empty()).then_some(Subject::Event { id, name })
        })
        .collect())
}

/// Events starting within the next day. The reminder is the post with the
/// seats left in it, which is the one that fills them.
async fn events_happening_tomorrow(db: &Database) -> Result<Vec<Subject>, sea_orm::DbErr> {
    let rows = db
        .orm
        .query_all(Statement::from_string(
            DbBackend::Postgres,
            "SELECT e.id::text AS id, e.title, e.starts_at, e.ends_at, e.max_seats, \
                    COALESCE(b.taken, 0)::int4 AS taken \
             FROM events e \
             LEFT JOIN ( \
                 SELECT event_id, SUM(seats)::int4 AS taken FROM event_bookings \
                 WHERE status <> 'cancelled' GROUP BY event_id \
             ) b ON b.event_id = e.id \
             WHERE e.is_public = TRUE \
               AND e.starts_at > NOW() \
               AND e.starts_at <= NOW() + INTERVAL '24 hours' \
             ORDER BY e.starts_at ASC LIMIT 20"
                .to_string(),
        ))
        .await?;
    // The shop's timezone. A hard-coded offset that cannot fail, written
    // without `expect` so a panic is impossible rather than merely unlikely —
    // this runs in a background loop where a panic takes the sweep with it.
    let bangkok = match chrono::FixedOffset::east_opt(7 * 3600) {
        Some(tz) => tz,
        None => {
            tracing::error!("promo: the Bangkok offset stopped being valid; skipping reminders");
            return Ok(Vec::new());
        }
    };
    Ok(rows
        .into_iter()
        .filter_map(|r| {
            let id: String = r.try_get("", "id").ok()?;
            let name: String = r.try_get("", "title").ok()?;
            let starts: chrono::DateTime<chrono::FixedOffset> = r.try_get("", "starts_at").ok()?;
            let ends: Option<chrono::DateTime<chrono::FixedOffset>> =
                r.try_get("", "ends_at").ok().flatten();
            let when = crate::trios::calendar::time_range(
                &starts.with_timezone(&bangkok).format("%H:%M").to_string(),
                ends.map(|e| e.with_timezone(&bangkok).format("%H:%M").to_string())
                    .as_deref(),
            );
            // `max_seats` NULL means unlimited, which is not "zero left".
            let max: Option<i32> = r.try_get("", "max_seats").ok().flatten();
            let taken: i32 = r.try_get("", "taken").unwrap_or(0);
            let seats_left = max.map(|m| (m - taken).max(0));
            (!id.is_empty() && !name.trim().is_empty()).then_some(Subject::EventSoon {
                id,
                name,
                when,
                seats_left,
            })
        })
        .collect())
}

/// Take this subject, or find that somebody already has.
///
/// `ON CONFLICT DO NOTHING` on the UNIQUE key is what makes "promote once"
/// true across a restart, a crash mid-send, or two instances overlapping
/// during a rollover — none of which a `SELECT` then `INSERT` would survive.
async fn claim(db: &Database, subject: &Subject) -> Result<bool, sea_orm::DbErr> {
    let res = db
        .orm
        .execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "INSERT INTO promo_posts (dedup_key, kind, subject_id, subject_name, body) \
             VALUES ($1, $2, $3, $4, '…') ON CONFLICT (dedup_key) DO NOTHING",
            [
                promo::dedup_key(subject).into(),
                subject.kind().into(),
                subject.id().into(),
                subject.name().into(),
            ],
        ))
        .await?;
    Ok(res.rows_affected() > 0)
}

async fn record_body(
    db: &Database,
    subject: &Subject,
    body: &str,
    source: &str,
) -> Result<(), sea_orm::DbErr> {
    db.orm
        .execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "UPDATE promo_posts SET body = $2, source = $3 WHERE dedup_key = $1",
            [promo::dedup_key(subject).into(), body.into(), source.into()],
        ))
        .await?;
    Ok(())
}

/// Ask the model; fall back to the written copy.
///
/// Returns the text and where it came from, so `source` in the table says
/// `glm` or `fallback` rather than leaving it to be inferred. With
/// `GLM_API_KEY` unset — which is production today — this always returns the
/// fallback, and the log says so once rather than looking like a failure.
async fn write_copy(
    subject: &Subject,
    db: &Database,
    ai: &crate::ai::AiClient,
) -> (String, &'static str) {
    let facts = facts_for(subject, db).await;
    let prompt = promo::prompt_for(subject, "ru", &facts);
    match ai
        .ask_grok(&prompt, subject.name(), "Ты копирайтер магазина.")
        .await
    {
        Some(answer) => {
            let usable = promo::usable_copy(&answer, subject);
            if usable == promo::fallback_copy(subject) {
                tracing::info!(
                    kind = subject.kind(),
                    "promo: the model's answer was unusable; sending the written copy"
                );
                (usable, "fallback")
            } else {
                (usable, "model")
            }
        }
        None => (promo::fallback_copy(subject), "fallback"),
    }
}

/// The only facts the model is allowed to use. Read from the database rather
/// than left to the model, which does not know this shop's prices.
async fn facts_for(subject: &Subject, db: &Database) -> String {
    let (table, price_col) = match subject {
        Subject::Strain { .. } => ("strains", "price_per_gram"),
        Subject::Accessory { .. } => ("accessories", "price"),
        Subject::Tea { .. } => ("tea_products", "price"),
        Subject::Set { .. } => ("sets", "total_price"),
        Subject::Event { .. } | Subject::EventSoon { .. } => {
            return match subject {
                Subject::EventSoon { when, .. } => {
                    format!("Название: {}\nВремя: {when}", subject.name())
                }
                _ => format!("Название: {}", subject.name()),
            }
        }
    };
    let sql = format!("SELECT {price_col}::float8 AS price FROM {table} WHERE id::text = $1");
    let price: Option<f64> = db
        .orm
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            &sql,
            [subject.id().into()],
        ))
        .await
        .ok()
        .flatten()
        .and_then(|r| r.try_get("", "price").ok());
    match price {
        Some(p) if p > 0.0 => format!("Название: {}\nЦена: {p:.0} ฿", subject.name()),
        _ => format!("Название: {}", subject.name()),
    }
}

/// Hand the draft to the owners, with the link a customer would follow and a
/// button that publishes it.
async fn send_draft(bot: &Bot, config: &Config, subject: &Subject, body: &str) {
    let link = subject
        .deeplink_target()
        .and_then(|t| crate::trios::deeplink::payload_for(&t))
        .map(|p| crate::bot::miniapp_deep_link(&config.bot_username, &p))
        .unwrap_or_default();

    let text = format!(
        "📣 <b>Черновик поста</b> — {kind}\n\
         ━━━━━━━━━━━━━━━━\n\
         {body}\n\n\
         🔗 {link}\n\
         📊 Метка: <code>{src}</code>",
        kind = crate::util::html_escape(subject.kind()),
        body = crate::util::html_escape(body),
        link = crate::util::html_escape(&link),
        src = crate::util::html_escape(&promo::attribution(subject)),
    );

    let keyboard = InlineKeyboardMarkup::new(vec![vec![InlineKeyboardButton::callback(
        "📢 Опубликовать",
        format!("{PUBLISH_CALLBACK}{}", promo::dedup_key(subject)),
    )]]);

    for admin in &config.admin_ids {
        let sent = bot
            .send_message(ChatId(*admin), &text)
            .parse_mode(teloxide::types::ParseMode::Html)
            .reply_markup(keyboard.clone())
            .await;
        if let Err(e) = sent {
            tracing::warn!("promo: could not reach admin {admin}: {e}");
        }
    }
}

/// An owner pressed **Опубликовать**.
///
/// The only place in this file that posts publicly, and it runs because a
/// human tapped it. Everything before this is a draft in the owner's chat.
pub async fn publish(
    db: Arc<Database>,
    bot: Bot,
    config: Arc<Config>,
    dedup_key: &str,
    by: i64,
) -> String {
    if !config.admin_ids.contains(&by) {
        tracing::warn!("promo: {by} pressed publish and is not an admin");
        return "Недостаточно прав".to_string();
    }

    let row = db
        .orm
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT body, subject_name, published_at FROM promo_posts WHERE dedup_key = $1",
            [dedup_key.into()],
        ))
        .await;
    let Ok(Some(row)) = row else {
        return "Черновик не найден".to_string();
    };
    let body: String = row.try_get("", "body").unwrap_or_default();
    let already: Option<chrono::DateTime<chrono::FixedOffset>> =
        row.try_get("", "published_at").ok().flatten();
    if already.is_some() {
        // Pressing twice is ordinary — the message stays in the chat with its
        // button. Saying so beats posting the same thing to the channel again.
        return "Уже опубликовано".to_string();
    }

    let Some(channel) = config.promo_channel_id else {
        // Refusing out loud. A button that reports success and posted nowhere
        // is worse than one that says the channel is not set up.
        tracing::warn!("promo: publish pressed but PROMO_CHANNEL_ID is unset");
        return "Канал не настроен: задайте PROMO_CHANNEL_ID и добавьте бота в админы канала"
            .to_string();
    };

    match bot
        .send_message(ChatId(channel), &body)
        .parse_mode(teloxide::types::ParseMode::Html)
        .await
    {
        Ok(_) => {
            // Recorded only after Telegram accepted it, so a failed send never
            // reads as published.
            if let Err(e) = db
                .orm
                .execute(Statement::from_sql_and_values(
                    DbBackend::Postgres,
                    "UPDATE promo_posts SET published_at = NOW(), published_by = $2 \
                     WHERE dedup_key = $1",
                    [dedup_key.into(), by.into()],
                ))
                .await
            {
                tracing::warn!("promo: published but could not record it: {e}");
            }
            tracing::info!(dedup_key, by, "promo: published to the channel");
            "Опубликовано ✅".to_string()
        }
        Err(e) => {
            tracing::warn!("promo: channel send failed: {e}");
            "Telegram отклонил публикацию".to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The callback prefix and the dedup key have to compose into something
    /// Telegram will carry: callback data is capped at 64 **bytes**, and
    /// Telegram rejects the whole message when a button exceeds it — so an
    /// over-long key means no button at all, on the message that needs one.
    #[test]
    fn the_publish_button_fits_in_telegrams_callback_limit() {
        let worst = Subject::EventSoon {
            // A UUID is the longest id this shop produces.
            id: "fe346171-aa5b-4f88-93ed-8be0ec38aa6c".into(),
            name: "irrelevant to the key".into(),
            when: "09:00 – 12:00".into(),
            seats_left: Some(4),
        };
        let data = format!("{PUBLISH_CALLBACK}{}", promo::dedup_key(&worst));
        assert!(
            data.len() <= 64,
            "callback data is {} bytes; Telegram drops the button over 64: {data}",
            data.len()
        );
    }
}
