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

/// The callback data on the "stop sending these" button.
pub const MUTE_CALLBACK: &str = "promo_mute";

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
    // Not only what is new. What already sells is never news and is the thing
    // most worth posting about.
    match bestsellers(&db).await {
        Ok(mut v) => found.append(&mut v),
        Err(e) => tracing::warn!("promo: bestseller scan failed: {e}"),
    }

    if found.is_empty() {
        tracing::info!("promo: nothing new to promote (tick ok)");
        return;
    }

    // Best first, by the money a post can plausibly move — not by whichever
    // row happened to be created earliest. With a per-tick cap, oldest-first
    // meant a 150 ฿ accessory could delay a 1200 ฿ set for hours.
    let mut scored: Vec<(Subject, Option<f64>)> = Vec::with_capacity(found.len());
    for s in found {
        let price = price_of(&s, &db).await;
        scored.push((s, price));
    }
    let mut found = crate::trios::promo::rank(scored);

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
        let image = image_for(&subject, &db).await;
        if let Err(e) = record_body(&db, &subject, &body, source, image.as_deref()).await {
            tracing::warn!("promo: could not record the body: {e}");
        }
        send_draft(&bot, &db, &config, &subject, &body, image.as_deref()).await;
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

/// New accessories and teas.
///
/// `strains` was dropped by 083_drop_cannabis_catalog; scanning it aborted the
/// whole tick with `relation "strains" does not exist` (production WARN, every
/// cycle, measured 2026-09-13). The tables that survive the rebrand stay in
/// the scan — they are real, and empty means "no news", which is correct.
async fn new_catalog_items(
    db: &Database,
    since: &chrono::DateTime<chrono::Utc>,
) -> Result<Vec<Subject>, sea_orm::DbErr> {
    let mut out = Vec::new();
    // Only what a customer could actually buy: an unavailable row is not news.
    for (table, avail) in [
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
    // The declared market's clock (D18), not an offset restated here. Written
    // without `expect` so a panic is impossible rather than merely unlikely —
    // this runs in a background loop where a panic takes the sweep with it.
    //
    // `None` now also means "this market moves its clocks", which no fixed
    // offset can express. Skipping the sweep is the right answer to that:
    // a reminder is a message to a customer naming a time, and a time this
    // program cannot derive must not be sent at all.
    let market_tz = match crate::trios::market::MARKET.fixed_offset() {
        Some(tz) => tz,
        None => {
            tracing::error!(
                timezone = crate::trios::market::MARKET.timezone_name,
                "promo: the declared market has no single UTC offset; skipping reminders"
            );
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
                &starts.with_timezone(&market_tz).format("%H:%M").to_string(),
                ends.map(|e| e.with_timezone(&market_tz).format("%H:%M").to_string())
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

/// What actually sells, from the orders themselves.
///
/// The agent was motivated by newness, which is not a sales signal: the
/// best-selling set in the shop is never news and is the thing most worth
/// posting about. This reads `orders.items` — a JSONB array of lines, each
/// with an `id` — and counts the last week.
///
/// The dedup key carries the ISO week, so a bestseller can return next week
/// and cannot return twice in this one.
async fn bestsellers(db: &Database) -> Result<Vec<Subject>, sea_orm::DbErr> {
    let period = crate::trios::promo::weekly_period(chrono::Utc::now());
    let rows = db
        .orm
        .query_all(Statement::from_string(
            DbBackend::Postgres,
            "SELECT line->>'id' AS item_id, COUNT(*)::int8 AS sold \
             FROM orders o, LATERAL jsonb_array_elements(o.items) AS line \
             WHERE o.created_at > NOW() - INTERVAL '7 days' \
               AND o.status <> 'cancelled' \
               AND line->>'id' IS NOT NULL \
             GROUP BY line->>'id' \
             HAVING COUNT(*) >= 2 \
             ORDER BY sold DESC \
             LIMIT 5"
                .to_string(),
        ))
        .await?;

    let mut out = Vec::new();
    for r in rows {
        let Ok(item_id) = r.try_get::<String>("", "item_id") else {
            continue;
        };
        let sold: i64 = r.try_get("", "sold").unwrap_or(0);
        // Which catalog it is, and whether it is still on sale. Promoting
        // something the shop has stopped selling is worse than saying nothing.
        // `strains` left with 083; a strain bestseller can no longer resolve.
        for (table, kind) in [("sets", crate::trios::promo::BestsellerKind::Set)] {
            let sql =
                format!("SELECT name FROM {table} WHERE id::text = $1 AND is_available = TRUE");
            if let Ok(Some(row)) = db
                .orm
                .query_one(Statement::from_sql_and_values(
                    DbBackend::Postgres,
                    &sql,
                    [item_id.clone().into()],
                ))
                .await
            {
                if let Ok(name) = row.try_get::<String>("", "name") {
                    out.push(Subject::Bestseller {
                        id: item_id.clone(),
                        name,
                        kind,
                        sold,
                        period: period.clone(),
                    });
                    break;
                }
            }
        }
    }
    Ok(out)
}

/// The price a subject would move, for ranking. `None` when the shop has no
/// price for it — an event, or a row that has gone missing.
async fn price_of(subject: &Subject, db: &Database) -> Option<f64> {
    let (table, col) = match subject {
        Subject::Accessory { .. } => ("accessories", "price"),
        Subject::Tea { .. } => ("tea_products", "price"),
        Subject::Set { .. } => ("sets", "total_price"),
        Subject::Bestseller { kind, .. } => match kind {
            crate::trios::promo::BestsellerKind::Set => ("sets", "total_price"),
        },
        Subject::Event { .. } | Subject::EventSoon { .. } => ("events", "price_baht"),
    };
    let sql = format!("SELECT {col}::float8 AS p FROM {table} WHERE id::text = $1");
    db.orm
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            &sql,
            [subject.id().into()],
        ))
        .await
        .ok()
        .flatten()
        .and_then(|r| r.try_get("", "p").ok())
        .flatten()
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
    image: Option<&str>,
) -> Result<(), sea_orm::DbErr> {
    // The deep-link payload is written down here because `publish` knows the
    // post only by its dedup key: a `bestseller` is about a strain or a set,
    // and the `kind` column alone cannot say which. Here the full `Subject`
    // is still in hand.
    let payload = subject
        .deeplink_target()
        .and_then(|t| crate::trios::deeplink::payload_for(&t));
    db.orm
        .execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "UPDATE promo_posts SET body = $2, source = $3, link_payload = $4, image_url = $5 \
             WHERE dedup_key = $1",
            [
                promo::dedup_key(subject).into(),
                body.into(),
                source.into(),
                payload.into(),
                image.map(str::to_string).into(),
            ],
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
    let answer = ai
        .ask_grok(&prompt, subject.name(), "Ты копирайтер магазина.")
        .await;
    copy_from_answer(answer.as_deref(), subject)
}

/// What the post says, and who wrote it.
///
/// Separated from `write_copy` because that function needs a live model and a
/// live database, so nothing could execute the decision — and this is the
/// decision that puts words under the shop's name.
///
/// Two rejections, and they are not the same kind of thing.
///
/// The first is ours. When the prompt sanitiser trips, `ask_grok` answers with
/// `crate::ai::PROMPT_FILTERED_REPLY`, a sentence this repository wrote,
/// returned inside `Some` so a chat gets a reply instead of silence. On this
/// path there is no chat: whatever comes back becomes a promotional post. It
/// is refused here by identity, against the constant that produces it, so
/// rewording the sentinel cannot quietly re-open this. Until 2026-09-21
/// nothing refused it at all — `usable_copy`'s refusal list holds "i cannot"
/// and "i'm sorry" and not "i can't", the sentence is well inside its length
/// bounds, and the sweeper stored it as the draft with source = "model".
///
/// The second is the model's own refusal, and there the wording really is all
/// the evidence there is: `usable_copy` keeps that judgement and this function
/// does not second-guess it.
fn copy_from_answer(answer: Option<&str>, subject: &Subject) -> (String, &'static str) {
    let Some(answer) = answer else {
        return (promo::fallback_copy(subject), "fallback");
    };
    if crate::ai::is_prompt_filtered_reply(answer) {
        tracing::info!(
            kind = subject.kind(),
            "promo: the prompt filter answered, not the model; sending the written copy"
        );
        return (promo::fallback_copy(subject), "fallback");
    }
    let usable = promo::usable_copy(answer, subject);
    if usable == promo::fallback_copy(subject) {
        tracing::info!(
            kind = subject.kind(),
            "promo: the model's answer was unusable; sending the written copy"
        );
        return (usable, "fallback");
    }
    (usable, "model")
}

/// The only facts the model is allowed to use. Read from the database rather
/// than left to the model, which does not know this shop's prices.
async fn facts_for(subject: &Subject, db: &Database) -> String {
    let (table, price_col) = match subject {
        Subject::Accessory { .. } => ("accessories", "price"),
        Subject::Tea { .. } => ("tea_products", "price"),
        Subject::Set { .. } => ("sets", "total_price"),
        Subject::Bestseller { kind, .. } => match kind {
            crate::trios::promo::BestsellerKind::Set => ("sets", "total_price"),
        },
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

/// The cover image for a subject.
///
/// A post without one is a wall of text in a feed of pictures. The owner asked
/// for the picture to be there always, and they are right about why: the same
/// offer reads as more expensive with it. When a row genuinely has no image the
/// draft still goes — a post that says nothing because it lacks a photo is
/// worse than a plain one — and the message says the picture is missing so the
/// owner can add it before publishing.
async fn image_for(subject: &Subject, db: &Database) -> Option<String> {
    let (table, col) = match subject {
        Subject::Accessory { .. } => ("accessories", "image_url"),
        Subject::Tea { .. } => ("tea_products", "image_url"),
        Subject::Set { .. } => ("sets", "image_url"),
        Subject::Bestseller { kind, .. } => match kind {
            crate::trios::promo::BestsellerKind::Set => ("sets", "image_url"),
        },
        Subject::Event { .. } | Subject::EventSoon { .. } => ("events", "image_url"),
    };
    let sql = format!("SELECT {col} AS url FROM {table} WHERE id::text = $1");
    let url: Option<String> = db
        .orm
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            &sql,
            [subject.id().into()],
        ))
        .await
        .ok()
        .flatten()
        .and_then(|r| r.try_get("", "url").ok())
        .flatten();
    // Telegram fetches the photo itself, so only an absolute http(s) URL is
    // any use: a relative path would fail the send and take the whole draft
    // with it.
    url.filter(|u| u.starts_with("https://") || u.starts_with("http://"))
}

/// Hand the draft to the owners, with the link a customer would follow and a
/// button that publishes it.
async fn send_draft(
    bot: &Bot,
    db: &Database,
    config: &Config,
    subject: &Subject,
    body: &str,
    image: Option<&str>,
) {
    let link = subject
        .deeplink_target()
        .and_then(|t| crate::trios::deeplink::payload_for(&t))
        .map(|p| crate::bot::miniapp_deep_link(&config.bot_username, &p))
        .unwrap_or_default();

    // A draft with no picture says so, rather than looking finished. The same
    // offer reads as more expensive with one, and a missing image is something
    // the owner can fix in the admin panel before publishing.
    let missing_photo = if image.is_none() {
        "\n⚠️ Без картинки — добавьте обложку в админке, с ней пост выглядит дороже"
    } else {
        ""
    };
    let text = format!(
        "📣 <b>Черновик поста</b> — {kind}\n\
         ━━━━━━━━━━━━━━━━\n\
         {body}\n\n\
         🔗 {link}\n\
         📊 Метка: <code>{src}</code>{missing_photo}",
        kind = crate::util::html_escape(subject.kind()),
        body = crate::util::html_escape(body),
        link = crate::util::html_escape(&link),
        src = crate::util::html_escape(&promo::attribution(subject)),
    );

    let keyboard = InlineKeyboardMarkup::new(vec![
        vec![InlineKeyboardButton::callback(
            "📢 Опубликовать",
            format!("{PUBLISH_CALLBACK}{}", promo::dedup_key(subject)),
        )],
        // Switching the agent off has to be one tap away, in the message
        // itself. An owner who wants it to stop and has to go looking for how
        // will instead learn to ignore it, which is the same outcome with none
        // of the signal.
        vec![InlineKeyboardButton::callback(
            "🔕 Отключить рассылку",
            MUTE_CALLBACK.to_string(),
        )],
    ]);

    for admin in &config.admin_ids {
        if is_muted(db, *admin).await {
            continue;
        }
        // A photo with the text as its caption, so the draft looks like the
        // post it will become rather than like a notification about one.
        // Telegram caps a caption at 1024 characters; `MAX_POST_LEN` plus this
        // envelope stays inside it.
        let sent = match image.and_then(|u| u.parse::<reqwest::Url>().ok()) {
            Some(url) => bot
                .send_photo(ChatId(*admin), teloxide::types::InputFile::url(url))
                .caption(&text)
                .parse_mode(teloxide::types::ParseMode::Html)
                .reply_markup(keyboard.clone())
                .await
                .map(|_| ()),
            None => bot
                .send_message(ChatId(*admin), &text)
                .parse_mode(teloxide::types::ParseMode::Html)
                .reply_markup(keyboard.clone())
                .await
                .map(|_| ()),
        };
        if let Err(e) = sent {
            tracing::warn!("promo: could not reach admin {admin}: {e}");
        }
    }
}

/// An owner pressed **Опубликовать**.
///
/// The only place in this file that messages clients, and it runs because a
/// human tapped it. Everything before this is a draft in the owner's chat.
///
/// The mailing is carried by the bot itself — a direct message per client —
/// because `PROMO_CHANNEL_ID` has sat unset since the agent shipped and a
/// publish that depends on it is a publish that never happens (#96). A
/// channel, when one is configured, is posted to as well, best-effort.
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
            "SELECT kind, body, subject_name, published_at, link_payload, image_url \
             FROM promo_posts WHERE dedup_key = $1",
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
        // button. Saying so beats posting the same thing to every client again.
        return "Уже опубликовано".to_string();
    }

    // Nothing retired reaches a customer off an old draft's button. Refused
    // before the stamp below, so the row stays a draft and nothing is written.
    let kind: String = row.try_get("", "kind").unwrap_or_default();
    if let Some(refusal) = publish_refusal(&kind) {
        tracing::info!(dedup_key, kind = %kind, by, "promo: publish refused, not a rental");
        return refusal.to_string();
    }

    // Marked published *before* the fan-out starts. A broadcast takes minutes
    // and must not be launchable twice off the same button; the summary says
    // what actually went out, so a total failure never reads as success.
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
        tracing::warn!("promo: could not mark the post published: {e}");
    }

    let subject_name: String = row.try_get("", "subject_name").unwrap_or_default();
    let link_payload: Option<String> = row.try_get("", "link_payload").ok().flatten();
    let image_url: Option<String> = row.try_get("", "image_url").ok().flatten();

    // The channel, when there is one, is posted to as well — but its failure
    // must not take the direct mailing down with it.
    if let Some(channel) = config.promo_channel_id {
        match bot
            .send_message(ChatId(channel), &body)
            .parse_mode(teloxide::types::ParseMode::Html)
            .await
        {
            Ok(_) => tracing::info!(dedup_key, "promo: posted to the channel too"),
            Err(e) => tracing::warn!("promo: channel send failed (broadcast continues): {e}"),
        }
    }

    let recipients = match broadcast_recipients(&db, &config.admin_ids).await {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("promo: could not read the client list: {e}");
            return "Не удалось прочитать список клиентов".to_string();
        }
    };
    if recipients.is_empty() {
        return "Некому отправлять: в базе нет клиентов".to_string();
    }
    let total = recipients.len();
    let post = BroadcastPost {
        dedup_key: dedup_key.to_string(),
        subject_name,
        body,
        link: link_payload
            .map(|p| crate::bot::miniapp_deep_link(&config.bot_username, &p))
            .filter(|l| !l.is_empty()),
        image: image_url,
    };
    tracing::info!(dedup_key, by, total, "promo: broadcast starting");
    tokio::spawn(async move {
        broadcast_post(db, bot, config, post, recipients).await;
    });
    format!("Рассылка запущена: {total} получателей")
}

/// The draft kinds the Publish button may still send to customers.
///
/// Empty. The shop has been rental-only since the owner's ruling of
/// 2026-09-24, and no kind the sweeper has ever written names a rental: every
/// one of them is an event or the retired catalog's (`RENTAL_SUBJECT_KIND_COUNT`
/// in `specs/turbobaby/promo_broadcast.t27` is zero). The first promotion about
/// a bike declares its kind here, and
/// `every_subject_kind_is_classified_for_publishing` below stops compiling
/// until the new variant is classified.
const PUBLISHABLE_KINDS: &[&str] = &[];

/// The two kinds a draft about an event is written under: the announcement
/// and the day-before reminder (`Subject::kind`).
const RETIRED_EVENT_KINDS: [&str; 2] = ["event", "event_soon"];

/// The owner's answer when the draft is about an event.
const PUBLISH_REFUSED_EVENT: &str = "Не опубликовано: мероприятия сняты, рассылка о них не идёт";

/// The owner's answer for every other draft that is not about a rental.
const PUBLISH_REFUSED_NOT_RENTAL: &str = "Не опубликовано: рассылка идёт только об аренде байков";

/// Why this draft must not reach customers, or `None` when it may.
///
/// Operator decision of 2026-09-25, under the owner's delegation (recorded in
/// `specs/turbobaby/promo_broadcast.t27`, `PUBLISH_REFUSAL_DECIDED_AT`). Events
/// left every customer surface with the rental-only ruling, and the retired
/// shop's goods with it, but `promo_posts` keeps every draft the sweeper ever
/// wrote, and each one still carries its Publish button in the owners' chat.
/// Pressed, an old event draft went out to the whole customer list.
///
/// Keyed on the row's `kind` column. A kind in neither list -- an empty
/// column, or one written by the other shop's bot this database forked from
/// (DECISIONS.md D19) -- is refused too: only a kind declared publishable is
/// sent. Nothing is deleted; the caller refuses before stamping the row, so it
/// stays a draft.
fn publish_refusal(kind: &str) -> Option<&'static str> {
    if PUBLISHABLE_KINDS.contains(&kind) {
        None
    } else if RETIRED_EVENT_KINDS.contains(&kind) {
        Some(PUBLISH_REFUSED_EVENT)
    } else {
        Some(PUBLISH_REFUSED_NOT_RENTAL)
    }
}

/// What a broadcast carries, lifted out of the row so the background task owns
/// one value rather than re-reading the database mid-send.
struct BroadcastPost {
    dedup_key: String,
    subject_name: String,
    body: String,
    /// The Mini App link a customer opens. `None` only for rows drafted before
    /// migration 076 — those go out without the open button.
    link: Option<String>,
    image: Option<String>,
}

/// Everyone the bot may write to directly: every user it has seen, minus those
/// who asked it to stop, those blocked for fraud, and the owners themselves
/// (they already have the draft in their chat).
async fn broadcast_recipients(db: &Database, admins: &[i64]) -> Result<Vec<i64>, sea_orm::DbErr> {
    let rows = db
        .orm
        .query_all(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT u.telegram_id FROM user_languages u \
             WHERE NOT EXISTS (SELECT 1 FROM promo_muted m \
                               WHERE m.telegram_id = u.telegram_id) \
               AND NOT COALESCE((SELECT p.is_blocked FROM loyalty_profiles p \
                                 WHERE p.telegram_id = u.telegram_id), FALSE) \
               AND u.telegram_id <> ALL($1) \
             ORDER BY u.telegram_id",
            [admins.to_vec().into()],
        ))
        .await?;
    Ok(rows
        .iter()
        .filter_map(|r| r.try_get::<i64>("", "telegram_id").ok())
        .collect())
}

/// The keyboard under a broadcast message. The mute button is in the message
/// itself for the same reason it is on the drafts: an opt-out the customer has
/// to go looking for is an opt-out Telegram bans bots for not having. A link
/// that does not parse drops its button rather than the whole message.
fn broadcast_keyboard(link: Option<&str>) -> InlineKeyboardMarkup {
    let open = link
        .and_then(|l| reqwest::Url::parse(l).ok())
        .map(|u| vec![InlineKeyboardButton::url("🛒 Открыть", u)]);
    let mut rows: Vec<Vec<InlineKeyboardButton>> = open.into_iter().collect();
    rows.push(vec![InlineKeyboardButton::callback(
        "🔕 Отключить рассылку",
        MUTE_CALLBACK.to_string(),
    )]);
    InlineKeyboardMarkup::new(rows)
}

/// How long to wait between two clients' messages.
///
/// Telegram's ceiling is ~30 messages a second, but a bot mailing hundreds of
/// people at that pace is the profile of a spammer, and the account is the
/// shop's. Half a second per client is undetectable to a customer and keeps a
/// thousand-person list inside ten minutes.
const BROADCAST_DELAY: std::time::Duration = std::time::Duration::from_millis(500);

async fn send_once(
    bot: &Bot,
    chat: i64,
    post: &BroadcastPost,
) -> Result<(), teloxide::RequestError> {
    let keyboard = broadcast_keyboard(post.link.as_deref());
    match post
        .image
        .as_deref()
        .and_then(|u| u.parse::<reqwest::Url>().ok())
    {
        Some(url) => bot
            .send_photo(ChatId(chat), teloxide::types::InputFile::url(url))
            .caption(&post.body)
            .parse_mode(teloxide::types::ParseMode::Html)
            .reply_markup(keyboard)
            .await
            .map(|_| ()),
        None => bot
            .send_message(ChatId(chat), &post.body)
            .parse_mode(teloxide::types::ParseMode::Html)
            .reply_markup(keyboard)
            .await
            .map(|_| ()),
    }
}

/// One `Retry-After` is survived, not retried forever: a single 429 with its
/// requested wait honoured once, then the client is recorded by outcome.
async fn send_to_client(
    bot: &Bot,
    chat: i64,
    post: &BroadcastPost,
) -> Result<(), teloxide::RequestError> {
    match send_once(bot, chat, post).await {
        Err(teloxide::RequestError::RetryAfter(wait)) => {
            tracing::warn!("promo: rate-limited, waiting {wait:?} as asked");
            tokio::time::sleep(wait.duration()).await;
            send_once(bot, chat, post).await
        }
        other => other,
    }
}

/// The fan-out itself. Runs in the background because a thousand half-second
/// sends cannot live inside a button press, and reports to the owners when it
/// ends — a mailing whose outcome nobody sees is a mailing nobody can trust.
async fn broadcast_post(
    db: Arc<Database>,
    bot: Bot,
    config: Arc<Config>,
    post: BroadcastPost,
    recipients: Vec<i64>,
) {
    let mut sent = 0usize;
    let mut failed = 0usize;
    let mut blocked = 0usize;
    for who in &recipients {
        let outcome = send_to_client(&bot, *who, &post).await;
        let (status, detail) = match outcome {
            Ok(()) => {
                sent += 1;
                ("sent", None)
            }
            Err(teloxide::RequestError::Api(teloxide::errors::ApiError::BotBlocked)) => {
                failed += 1;
                blocked += 1;
                ("failed", Some("blocked the bot"))
            }
            Err(_) => {
                failed += 1;
                ("failed", Some("unreachable"))
            }
        };
        if let Err(e) = db
            .orm
            .execute(Statement::from_sql_and_values(
                DbBackend::Postgres,
                "INSERT INTO promo_deliveries (dedup_key, telegram_id, status, detail) \
                 VALUES ($1, $2, $3, $4) \
                 ON CONFLICT (dedup_key, telegram_id) \
                 DO UPDATE SET status = $3, detail = $4, sent_at = NOW()",
                [
                    post.dedup_key.clone().into(),
                    (*who).into(),
                    status.into(),
                    detail.map(str::to_string).into(),
                ],
            ))
            .await
        {
            // A delivery row is reporting, not gating: the send already
            // happened, and losing the record must not lose the mailing.
            tracing::warn!("promo: could not record delivery to {who}: {e}");
        }
        tokio::time::sleep(BROADCAST_DELAY).await;
    }

    let summary = broadcast_summary(&post.subject_name, sent, failed, blocked, recipients.len());
    tracing::info!(dedup_key = %post.dedup_key, sent, failed, blocked, "promo: broadcast finished");
    for admin in &config.admin_ids {
        if is_muted(&db, *admin).await {
            continue;
        }
        let text = format!("📣 <b>Рассылка завершена</b>\n{summary}");
        if let Err(e) = bot
            .send_message(ChatId(*admin), &text)
            .parse_mode(teloxide::types::ParseMode::Html)
            .await
        {
            tracing::warn!("promo: could not reach admin {admin} with the summary: {e}");
        }
    }
}

/// What the owners read when a mailing ends. Extracted to be executed.
pub(crate) fn broadcast_summary(
    subject_name: &str,
    sent: usize,
    failed: usize,
    blocked: usize,
    total: usize,
) -> String {
    let line = format!(
        "«{}» — доставлено {} из {}",
        crate::util::html_escape(subject_name),
        sent,
        total
    );
    if failed == 0 {
        format!("{line}. Все сообщения дошли.")
    } else if blocked == failed {
        format!(
            "{line}, не дошло {failed}. Все {blocked} заблокировали бота — \
             рассылка к ним больше не идёт сама по себе."
        )
    } else {
        format!(
            "{line}, не дошло {failed} (из них {blocked} заблокировали бота). \
             Обычно это значит, что чат недоступен."
        )
    }
}

/// Has this owner asked the promoter to stop?
///
/// Failing open — a database hiccup means the draft is sent — because an owner
/// who muted it will mute it again, whereas silence caused by an error looks
/// exactly like an agent that has stopped working. Both garden sweeps in this
/// project were mistaken for dead that way.
async fn is_muted(db: &Database, admin: i64) -> bool {
    db.orm
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT 1 AS x FROM promo_muted WHERE telegram_id = $1",
            [admin.into()],
        ))
        .await
        .map(|r| r.is_some())
        .unwrap_or(false)
}

/// An owner pressed **Отключить рассылку**.
pub async fn mute(db: Arc<Database>, who: i64) -> String {
    match db
        .orm
        .execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "INSERT INTO promo_muted (telegram_id) VALUES ($1) \
             ON CONFLICT (telegram_id) DO NOTHING",
            [who.into()],
        ))
        .await
    {
        Ok(_) => {
            tracing::info!(who, "promo: muted for this admin");
            // Saying how to undo it, because a switch with no visible way back
            // is one nobody dares press.
            "Больше не присылаю. Включить обратно: /promo_on".to_string()
        }
        Err(e) => {
            tracing::warn!("promo: could not mute {who}: {e}");
            "Не удалось отключить, попробуйте ещё раз".to_string()
        }
    }
}

/// And back on again.
pub async fn unmute(db: &Database, who: i64) -> bool {
    db.orm
        .execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "DELETE FROM promo_muted WHERE telegram_id = $1",
            [who.into()],
        ))
        .await
        .is_ok()
}

/// What each published post actually sold.
///
/// The chain: a post is published, a customer opens its link (which carries
/// the campaign and the subject, so `client_event_logs` records
/// `promo_link_opened` with `promo_<kind>:<subject id>`), and an order from the
/// same `telegram_id` follows within a day.
///
/// Joined on the **subject**, not the kind. On the kind alone, two sets
/// promoted in the same month both claim every set-link open, and the question
/// "which post sold what" can only be answered per category — which was the
/// question.
///
/// This is the only thing that makes the agent answerable to sales rather than
/// to newness. Without it every claim about whether promotion works is a
/// guess — and the previous version of this agent shipped with the attribution
/// printed to the owner and never carried in the link, which is exactly the
/// shape of a metric nobody can compute.
///
/// **The window is a choice, and a generous one.** 24 hours after the open,
/// same customer. It cannot prove the post caused the order — somebody who was
/// going to buy anyway also taps links — so this is an upper bound on the
/// effect, and it is reported as "orders after" rather than "orders caused".
pub async fn report(db: &Database, days: i64) -> Result<Vec<PromoResult>, sea_orm::DbErr> {
    let rows = db
        .orm
        .query_all(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "WITH published AS ( \
                 SELECT dedup_key, kind, subject_id, subject_name, published_at \
                 FROM promo_posts \
                 WHERE published_at IS NOT NULL AND published_at > NOW() - ($1 || ' days')::interval \
             ), opens AS ( \
                 SELECT p.dedup_key, e.telegram_id, e.occurred_at \
                 FROM published p \
                 JOIN client_event_logs e \
                   ON e.event = 'promo_link_opened' \
                  AND e.detail = 'promo_' || p.kind || ':' || p.subject_id \
                  AND e.occurred_at >= p.published_at \
                 WHERE e.telegram_id IS NOT NULL \
             ) \
             SELECT p.dedup_key, p.kind, p.subject_name, \
                    COUNT(DISTINCT o.telegram_id)::int4          AS openers, \
                    COUNT(DISTINCT ord.id)::int4                 AS orders, \
                    COALESCE(SUM(DISTINCT ord.total), 0)::float8 AS revenue \
             FROM published p \
             LEFT JOIN opens o ON o.dedup_key = p.dedup_key \
             LEFT JOIN orders ord \
                    ON ord.telegram_id = o.telegram_id \
                   AND ord.created_at BETWEEN o.occurred_at AND o.occurred_at + INTERVAL '24 hours' \
             GROUP BY p.dedup_key, p.kind, p.subject_name, p.published_at \
             ORDER BY revenue DESC, orders DESC",
            [days.to_string().into()],
        ))
        .await?;

    Ok(rows
        .into_iter()
        .map(|r| PromoResult {
            kind: r.try_get("", "kind").unwrap_or_default(),
            subject_name: r.try_get("", "subject_name").unwrap_or_default(),
            openers: r.try_get("", "openers").unwrap_or(0),
            orders: r.try_get("", "orders").unwrap_or(0),
            revenue: r.try_get("", "revenue").unwrap_or(0.0),
        })
        .collect())
}

/// One published post and what followed it.
#[derive(Debug, Clone, serde::Serialize)]
pub struct PromoResult {
    pub kind: String,
    pub subject_name: String,
    /// Distinct people who opened the link.
    pub openers: i32,
    /// Orders from those people within a day. **After**, not *because of* —
    /// somebody who was going to buy anyway also taps links.
    pub orders: i32,
    pub revenue: f64,
}

impl From<PromoResult> for crate::trios::promo::DigestRow {
    fn from(r: PromoResult) -> Self {
        Self {
            kind: r.kind,
            name: r.subject_name,
            openers: r.openers,
            orders: r.orders,
            revenue: r.revenue,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The prompt filter's own sentence must never be published as copy.
    ///
    /// When the sanitiser trips, `ask_grok` hands back a sentence this
    /// repository wrote, inside the same `Some` a real answer arrives in. In a
    /// chat that is an answer; here it would be a promotional post, stored
    /// with source = "model" and one owner press away from every subscriber.
    ///
    /// Asserted BY IDENTITY, against the constant that produces the sentence,
    /// so this cannot pass by matching words the sentinel no longer uses.
    #[test]
    fn the_prompt_filters_own_sentence_is_never_published_as_copy() {
        let s = Subject::Set {
            id: "s1".into(),
            name: "Honda Click 125i".into(),
        };
        let (body, source) = copy_from_answer(Some(crate::ai::PROMPT_FILTERED_REPLY), &s);
        assert_eq!(
            body,
            promo::fallback_copy(&s),
            "the prompt filter's own sentence was published as the post"
        );
        assert_eq!(
            source, "fallback",
            "the shop's own sentinel was recorded as the model's writing"
        );
    }

    /// Why the check above has to be an identity and not another phrase in a
    /// list: the sentinel passes every bound `usable_copy` applies. This is a
    /// measurement of that function, not a wish about it — if it ever starts
    /// rejecting the sentinel, come back and read why the identity check is
    /// still the one that must hold.
    #[test]
    fn the_refusal_list_does_not_stop_the_sentinel() {
        let s = Subject::Set {
            id: "s1".into(),
            name: "Honda Click 125i".into(),
        };
        assert_eq!(
            promo::usable_copy(crate::ai::PROMPT_FILTERED_REPLY, &s),
            crate::ai::PROMPT_FILTERED_REPLY,
            "the refusal list now catches the sentinel too; the identity check above is still what the post depends on"
        );
    }

    /// The rejection is narrow: a model answer that is usable is still kept,
    /// and still says the model wrote it. A promoter that always sends its own
    /// text has no use for a model at all.
    #[test]
    fn a_usable_model_answer_is_still_the_models() {
        let s = Subject::Set {
            id: "s1".into(),
            name: "Honda Click 125i".into(),
        };
        // Any well-formed answer will do here; what is under test is that a
        // usable one still passes through, not the language it is written in.
        let good = "Honda Click 125i is in the park now. Light, cheap to run, easy in town.";
        let (body, source) = copy_from_answer(Some(good), &s);
        assert_eq!(body, good);
        assert_eq!(source, "model");
    }

    /// No answer at all is still the written copy, attributed to nobody --
    /// and the whole mapping around that row, because the row alone proves
    /// nothing.
    ///
    /// Measured 2026-09-21, adversarial review: this test asserted only the
    /// `None` row and passed with the sentinel check removed. It had to. An
    /// absent answer lands on the fallback under every version of this
    /// function ever shipped, the one that published the sentinel included.
    /// What CAN fail is the table: the four things that can come back and
    /// where each of them lands, which is the same table the contract states
    /// as `body_is_the_written_fallback` and `source_is_the_model`
    /// (specs/turbobaby/promo_broadcast.t27). The sentinel row is the one that
    /// moves when the identity check goes.
    #[test]
    fn a_silent_model_falls_back() {
        let s = Subject::Set {
            id: "s1".into(),
            name: "Honda Click 125i".into(),
        };
        let fallback = promo::fallback_copy(&s);
        // Any well-formed answer will do for the last row; what is under test
        // is where each KIND of answer lands, not the language one is in.
        let good = "Honda Click 125i is in the park now. Light, cheap to run, easy in town.";
        for (answer, body, source, what_came_back) in [
            (
                None,
                fallback.as_str(),
                "fallback",
                "a model that said nothing at all",
            ),
            (
                Some(crate::ai::PROMPT_FILTERED_REPLY),
                fallback.as_str(),
                "fallback",
                "the prompt filter's own sentence, which no model wrote",
            ),
            (
                Some("too short"),
                fallback.as_str(),
                "fallback",
                "an answer the owner's copy check rejects",
            ),
            (
                Some(good),
                good,
                "model",
                "an answer the owner's copy check keeps",
            ),
        ] {
            assert_eq!(
                copy_from_answer(answer, &s),
                (body.to_string(), source),
                "{what_came_back}"
            );
        }
    }

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

    /// A mailing the owners cannot see the end of is a mailing they cannot
    /// trust. The summary must say how many were reached — and distinguish a
    /// client who blocked the bot (gone for good) from a chat that merely
    /// failed, because only one of those is a reason to clean the list.
    #[test]
    fn the_broadcast_summary_counts_and_explains() {
        let all_sent = broadcast_summary("Party Pack", 30, 0, 0, 30);
        assert!(all_sent.contains("доставлено 30 из 30"));
        assert!(all_sent.contains("Все сообщения дошли"));

        let all_blocked = broadcast_summary("Party Pack", 28, 2, 2, 30);
        assert!(all_blocked.contains("не дошло 2"));
        assert!(all_blocked.contains("заблокировали бота"));

        let mixed = broadcast_summary("Party Pack", 27, 3, 1, 30);
        assert!(mixed.contains("не дошло 3"));
        assert!(mixed.contains("из них 1 заблокировали"));
    }

    /// The subject name reaches the summary through HTML parse mode; a name
    /// with markup in it must not turn into markup.
    #[test]
    fn the_broadcast_summary_escapes_the_subject_name() {
        let s = broadcast_summary("<b>Sets & More</b>", 1, 0, 0, 1);
        assert!(
            s.contains("&lt;b&gt;Sets &amp; More&lt;/b&gt;"),
            "leaked: {s}"
        );
        assert!(!s.contains("<b>"));
    }

    /// The broadcast message always carries the mute button, with or without
    /// a link — an opt-out that only ships when the link is parseable is not
    /// an opt-out.
    #[test]
    fn the_broadcast_keyboard_always_has_the_mute_button() {
        use teloxide::types::InlineKeyboardButtonKind;
        let has_mute = |kb: &InlineKeyboardMarkup| {
            kb.inline_keyboard
                .iter()
                .flatten()
                .any(|b| b.kind == InlineKeyboardButtonKind::CallbackData(MUTE_CALLBACK.into()))
        };
        let has_url = |kb: &InlineKeyboardMarkup| {
            kb.inline_keyboard
                .iter()
                .flatten()
                .any(|b| matches!(b.kind, InlineKeyboardButtonKind::Url(_)))
        };
        for link in [
            Some("https://t.me/turboagent_phuket_bot/app?startapp=p_set_abc"),
            None,
            Some("not a url at all"),
        ] {
            let kb = broadcast_keyboard(link);
            assert!(
                has_mute(&kb),
                "no mute button in the broadcast for {link:?}"
            );
            assert_eq!(
                has_url(&kb),
                link == Some("https://t.me/turboagent_phuket_bot/app?startapp=p_set_abc"),
                "open button presence wrong for {link:?}"
            );
        }
    }

    /// Every kind the sweeper writes today is refused, and a draft about an
    /// event is told apart from the rest.
    ///
    /// Six kinds, read 2026-09-25 off `Subject::kind`: two event kinds and four
    /// others. It once wrote a seventh, the retired catalog's own kind, whose
    /// variant left with migration 083 while a draft written under it can
    /// still sit unpublished in `promo_posts`. That word is not run here,
    /// because nothing outside a comment under src/ may name it
    /// (tests/legacy_vocabulary_wiring.rs; owner ruling 2026-09-25, item 12).
    /// No list classifies it, so it takes the fail-closed branch that
    /// `a_kind_nobody_classified_is_refused_too` runs; that it sits in neither
    /// list, and that the contract still lists it among the retired shop's
    /// kinds, is held by tests/promo_publish_wiring.rs.
    #[test]
    fn no_draft_kind_written_today_is_sent_while_the_shop_is_rental_only() {
        for kind in ["event", "event_soon"] {
            assert_eq!(
                publish_refusal(kind),
                Some(PUBLISH_REFUSED_EVENT),
                "a draft of kind {kind:?} would reach every customer"
            );
        }
        for kind in ["accessory", "tea", "set", "bestseller"] {
            assert_eq!(
                publish_refusal(kind),
                Some(PUBLISH_REFUSED_NOT_RENTAL),
                "a draft of kind {kind:?} would reach every customer"
            );
        }
    }

    /// Fail closed. The database forked from another shop's bot (DECISIONS.md
    /// D19), so a kind this file has never classified may be in the table; it
    /// is refused like the retired ones, not waved through because nobody
    /// listed it. The column is NOT NULL, but a failed read lands on "" and
    /// that is refused as well.
    #[test]
    fn a_kind_nobody_classified_is_refused_too() {
        for kind in ["", "Event", "event ", "sommelier", "garden"] {
            assert_eq!(
                publish_refusal(kind),
                Some(PUBLISH_REFUSED_NOT_RENTAL),
                "an unclassified kind {kind:?} was let through"
            );
        }
    }

    /// Every variant the sweeper can draft today is classified. The match
    /// below has no wildcard on purpose: a new `Subject` variant -- the first
    /// rental one, one day -- stops this test compiling until somebody decides
    /// whether its drafts may be sent, and adds it to `PUBLISHABLE_KINDS` if
    /// they may.
    #[test]
    fn every_subject_kind_is_classified_for_publishing() {
        fn is_about_an_event(s: &Subject) -> bool {
            match s {
                Subject::Event { .. } | Subject::EventSoon { .. } => true,
                Subject::Accessory { .. }
                | Subject::Tea { .. }
                | Subject::Set { .. }
                | Subject::Bestseller { .. } => false,
            }
        }
        let id = || "x1".to_string();
        let name = || "irrelevant to the kind".to_string();
        let every = [
            Subject::Accessory {
                id: id(),
                name: name(),
            },
            Subject::Tea {
                id: id(),
                name: name(),
            },
            Subject::Set {
                id: id(),
                name: name(),
            },
            Subject::Event {
                id: id(),
                name: name(),
            },
            Subject::EventSoon {
                id: id(),
                name: name(),
                when: "09:00 – 12:00".into(),
                seats_left: None,
            },
            Subject::Bestseller {
                id: id(),
                name: name(),
                kind: promo::BestsellerKind::Set,
                sold: 2,
                period: "2026-W39".into(),
            },
        ];
        let mut kinds: Vec<&str> = every.iter().map(Subject::kind).collect();
        kinds.sort_unstable();
        kinds.dedup();
        assert_eq!(kinds.len(), every.len(), "one of each variant: {kinds:?}");
        for s in &every {
            let expected = if is_about_an_event(s) {
                PUBLISH_REFUSED_EVENT
            } else {
                PUBLISH_REFUSED_NOT_RENTAL
            };
            assert_eq!(publish_refusal(s.kind()), Some(expected), "{}", s.kind());
        }
    }

    /// The answer is shown in the Publish button's alert, and Telegram
    /// documents `answerCallbackQuery` text as 0-200 characters. It must also
    /// read as a refusal: the owner has just pressed a button that used to
    /// start a mailing.
    #[test]
    fn the_refusals_fit_in_the_alert_and_say_nothing_was_sent() {
        for answer in [PUBLISH_REFUSED_EVENT, PUBLISH_REFUSED_NOT_RENTAL] {
            assert!(answer.chars().count() <= 200, "too long: {answer}");
            assert!(answer.starts_with("Не опубликовано"), "{answer}");
        }
        assert_ne!(PUBLISH_REFUSED_EVENT, PUBLISH_REFUSED_NOT_RENTAL);
    }
}
