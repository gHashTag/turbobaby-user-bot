// Loop #12/#13: abandoned-cart recovery.
//
// A lightweight background sweep looks for server-side carts that still have
// items but haven't been touched for a while. The customer gets up to two
// Telegram reminders with a deep link straight back into the cart so they can
// finish checkout in one tap. First reminder is assistance-only; second
// nudge (24 h later) adds a soft perk (bonus points) as an incentive ladder.
//
// A/B variant assignment is persisted on the cart row so the same user always
// sees the same copy within one cart session.

use sea_orm::{ConnectionTrait, DbBackend, Statement};
use teloxide::prelude::*;

/// Start the abandoned-cart reminder sweep.
///
/// `interval_secs` is the sleep between sweeps; the first sweep happens after
/// one interval so the server isn't slammed on boot.
pub(crate) fn spawn_cart_abandonment_reminder_loop(
    orm: sea_orm::DatabaseConnection,
    bot: std::sync::Arc<teloxide::Bot>,
    config: std::sync::Arc<crate::config::Config>,
    interval_secs: u64,
) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(interval_secs));
        interval.tick().await; // discard cold-start tick
        loop {
            interval.tick().await;
            match send_cart_abandonment_reminders(&orm, &bot, &config).await {
                // silent-tick: deliberate, not an omission -- ticks every 300s = 288 lines/day per sweep, and a quiet shop is the normal case.
                // Reporting "nothing due" here would announce health where there may be
                // total delivery failure, which is worse than the silence it replaced.
                Ok(0) => {}
                Ok(n) => tracing::info!("cart abandonment reminders: sent {} reminder(s)", n),
                Err(e) => tracing::warn!("cart abandonment sweep failed: {}", e),
            }
            match send_cart_second_nudges(&orm, &bot, &config).await {
                // A tick that found nothing must still say so: a loop whose only
                // evidence of life is an occasional line is indistinguishable from a
                // loop that stopped. Both garden sweeps were broken for months this way.
                // silent-tick: deliberate, same reason as the first arm above -- this loop
                // ticks every 300s, so a line per tick is ~288/day, and a quiet shop is
                // the normal case. Measured in production 2026-08-12: this arm alone
                // produced 21 lines in 1.7 hours before it was silenced.
                Ok(0) => {}
                Ok(n) => tracing::info!("cart abandonment second nudges: sent {} reminder(s)", n),
                Err(e) => tracing::warn!("cart abandonment second nudge sweep failed: {}", e),
            }
        }
    });
}

/// Cart snapshot used to build a personalized reminder message.
struct CartSnapshot {
    cart_id: String,
    telegram_id: i64,
    lang: String,
    variant: String,
    start_param: Option<String>,
    total: f64,
    items: Vec<CartItemLine>,
}

struct CartItemLine {
    name: String,
    quantity: i32,
    _unit_price: f64,
}

/// Load up to 100 carts that qualify for the first reminder, together with
/// their line items so the message can be personalized.
#[cfg(not(target_arch = "wasm32"))]
async fn load_abandoned_carts(
    orm: &sea_orm::DatabaseConnection,
    reminder_count: i32,
    threshold_minutes: i64,
    extra_predicate: &str,
) -> Result<Vec<CartSnapshot>, sea_orm::DbErr> {
    // Assign an A/B variant to any cart that doesn't have one yet. This must
    // happen before the SELECT so every cart in this batch has a stable value.
    orm.execute(Statement::from_string(
        DbBackend::Postgres,
        "UPDATE carts SET reminder_variant = CASE WHEN random() < 0.5 THEN 'v1' ELSE 'v2' END \
         WHERE reminder_variant IS NULL"
            .to_string(),
    ))
    .await?;

    let sql = format!(
        "SELECT c.id, c.telegram_id, COALESCE(ul.language, 'en') AS lang, \
         c.reminder_variant, c.start_param, SUM(ci.unit_price * ci.quantity) AS total \
         FROM carts c \
         JOIN cart_items ci ON ci.cart_id = c.id \
         LEFT JOIN user_languages ul ON ul.telegram_id = c.telegram_id \
         WHERE c.reminder_count = $1 \
           AND c.updated_at < (now() - interval '1 minute' * $2) \
           AND c.telegram_id != 0 \
           {extra_predicate} \
         GROUP BY c.id, c.telegram_id, ul.language, c.reminder_variant, c.start_param \
         LIMIT 100"
    );

    let rows = orm
        .query_all(Statement::from_sql_and_values(
            DbBackend::Postgres,
            &sql,
            [reminder_count.into(), threshold_minutes.into()],
        ))
        .await?;

    let mut out = Vec::new();
    for r in rows {
        let cart_id: String = r.try_get("", "id").unwrap_or_default();
        let telegram_id: i64 = r.try_get("", "telegram_id").unwrap_or(0);
        if telegram_id == 0 {
            continue;
        }
        let lang: String = r.try_get("", "lang").unwrap_or_else(|_| "en".into());
        let variant: String = r
            .try_get("", "reminder_variant")
            .unwrap_or_else(|_| "v1".into());
        let start_param: Option<String> = r.try_get("", "start_param").ok();
        let total: f64 = r.try_get::<f64>("", "total").unwrap_or_else(|_| {
            r.try_get::<String>("", "total")
                .unwrap_or_default()
                .parse()
                .unwrap_or(0.0)
        });

        let item_rows = orm
            .query_all(Statement::from_sql_and_values(
                DbBackend::Postgres,
                "SELECT name, quantity, unit_price FROM cart_items WHERE cart_id = $1 ORDER BY created_at",
                [cart_id.clone().into()],
            ))
            .await?;
        let items = item_rows
            .into_iter()
            .map(|ir| CartItemLine {
                name: ir.try_get("", "name").unwrap_or_default(),
                quantity: ir.try_get("", "quantity").unwrap_or(1),
                _unit_price: ir.try_get("", "unit_price").unwrap_or(0.0),
            })
            .collect();

        out.push(CartSnapshot {
            cart_id,
            telegram_id,
            lang,
            variant,
            start_param,
            total,
            items,
        });
    }
    Ok(out)
}

/// Send the first abandoned-cart reminder. Variant copy is chosen from the
/// cart row; the message includes the cart total and a short item list.
#[cfg(not(target_arch = "wasm32"))]
async fn send_cart_abandonment_reminders(
    orm: &sea_orm::DatabaseConnection,
    bot: &teloxide::Bot,
    config: &crate::config::Config,
) -> Result<usize, sea_orm::DbErr> {
    // 10 minutes of inactivity before we consider the cart abandoned.
    // Telegram Mini App users often context-switch; this window is long
    // enough to avoid annoying fast shoppers but short enough to catch real
    // drop-offs while the intent is still warm.
    let carts = load_abandoned_carts(orm, 0, 10, "").await?;

    let mut sent = 0usize;
    for cart in carts {
        let locale = crate::locales::get_locale(&cart.lang);
        let text = build_first_reminder_text(&cart, &locale);
        let deep_link = build_cart_deep_link(config, &cart);
        let markup = teloxide::types::InlineKeyboardMarkup::new(vec![vec![crate::bot::url_btn(
            &locale.cart_open,
            &deep_link,
        )]]);

        if let Err(e) = bot
            .send_message(ChatId(cart.telegram_id), text)
            .reply_markup(markup)
            .await
        {
            tracing::warn!(
                "cart abandonment reminder failed: cart_id={} telegram_id={} err={}",
                cart.cart_id,
                cart.telegram_id,
                e
            );
            // Still mark reminder sent so a transient send failure doesn't
            // retry every sweep and hammer Telegram.
        }

        orm.execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "UPDATE carts SET reminder_sent_at = now(), first_reminder_sent_at = now(), reminder_count = reminder_count + 1 WHERE id = $1",
            [cart.cart_id.into()],
        ))
        .await?;

        crate::metrics::cart_abandonment_reminder_sent(&cart.variant);
        sent += 1;
    }

    Ok(sent)
}

/// Send the second nudge 24 hours after the first reminder. Uses an incentive
/// ladder (bonus points) to recover high-intent users without training everyone
/// to wait for a discount.
#[cfg(not(target_arch = "wasm32"))]
async fn send_cart_second_nudges(
    orm: &sea_orm::DatabaseConnection,
    bot: &teloxide::Bot,
    config: &crate::config::Config,
) -> Result<usize, sea_orm::DbErr> {
    let nudge_after_hours = 24i64;
    let carts = load_abandoned_carts(
        orm,
        1,
        0,
        &format!(
            "AND c.first_reminder_sent_at < (now() - interval '1 hour' * {})",
            nudge_after_hours
        ),
    )
    .await?;

    let mut sent = 0usize;
    for cart in carts {
        let locale = crate::locales::get_locale(&cart.lang);
        let text = build_second_nudge_text(&cart, &locale);
        let deep_link = build_cart_deep_link(config, &cart);
        let markup = teloxide::types::InlineKeyboardMarkup::new(vec![vec![crate::bot::url_btn(
            &locale.cart_open,
            &deep_link,
        )]]);

        if let Err(e) = bot
            .send_message(ChatId(cart.telegram_id), text)
            .reply_markup(markup)
            .await
        {
            tracing::warn!(
                "cart abandonment second nudge failed: cart_id={} telegram_id={} err={}",
                cart.cart_id,
                cart.telegram_id,
                e
            );
        }

        orm.execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "UPDATE carts SET reminder_sent_at = now(), reminder_count = reminder_count + 1 WHERE id = $1",
            [cart.cart_id.into()],
        ))
        .await?;

        crate::metrics::cart_abandonment_second_nudge_sent(&cart.variant);
        sent += 1;
    }

    Ok(sent)
}

/// Build a deep link that optionally carries the original start_param for
/// attribution. The payload stays within Telegram's 64-char limit.
#[cfg(not(target_arch = "wasm32"))]
fn build_cart_deep_link(config: &crate::config::Config, cart: &CartSnapshot) -> String {
    let start_param = cart
        .start_param
        .as_deref()
        .filter(|s| !s.is_empty() && s.len() <= 50)
        .map(|s| format!("cart__{}", s))
        .unwrap_or_else(|| "cart".to_string());
    crate::bot::miniapp_deep_link(&config.bot_username, &start_param)
}

/// Compose the first reminder text. v2 uses a more conversational tone.
#[cfg(not(target_arch = "wasm32"))]
fn build_first_reminder_text(cart: &CartSnapshot, locale: &crate::locales::Locale) -> String {
    let total_str = crate::trios::pricing::format_baht(cart.total);
    let item_summary = format_item_summary(&cart.items);
    let base = if cart.variant == "v2" {
        &locale.cart_abandonment_reminder_v2
    } else {
        &locale.cart_abandonment_reminder
    };
    format!("{}\n\n{}\n\n💰 {}", base, item_summary, total_str)
}

/// Compose the second nudge with the soft-perk copy.
#[cfg(not(target_arch = "wasm32"))]
fn build_second_nudge_text(cart: &CartSnapshot, locale: &crate::locales::Locale) -> String {
    let total_str = crate::trios::pricing::format_baht(cart.total);
    let item_summary = format_item_summary(&cart.items);
    format!(
        "{}\n\n{}\n\n💰 {}",
        locale.cart_second_nudge, item_summary, total_str
    )
}

/// Compact item list for the Telegram message: "Item A x2, Item B x1".
#[cfg(not(target_arch = "wasm32"))]
fn format_item_summary(items: &[CartItemLine]) -> String {
    if items.is_empty() {
        return String::new();
    }
    let parts: Vec<String> = items
        .iter()
        .map(|i| format!("{} x{}", i.name, i.quantity))
        .collect();
    // Keep the summary short so the total message stays readable on phones.
    let joined = parts.join(", ");
    if joined.chars().count() > 180 {
        let mut truncated = parts[0].clone();
        if parts.len() > 1 {
            truncated.push_str(&format!(" + {} more", parts.len() - 1));
        }
        truncated
    } else {
        joined
    }
}
