//! What to promote, what to say, and how to know whether it sold anything.
//!
//! The rules live here rather than in the sweeper or the bot handler because
//! neither of those is reachable by `cargo test`: `src/db` is behind the
//! `backend` feature and `src/bot` needs a live Telegram. A promotion rule that
//! nobody can execute is a rule nobody has checked — and this one decides what
//! goes out to customers under the shop's name.
//!
//! Three things are decided here:
//!
//! 1. **What counts as a reason to post.** New strain, new set, new event, and
//!    an event about to happen. The fourth is not "something appeared in the
//!    database" and so is kept as its own case rather than folded in.
//! 2. **What the post says when the model is unavailable.** `GLM_API_KEY` is
//!    unset in production right now — the boot log says so on every start. A
//!    promoter that silently posts nothing when the key is missing is a
//!    promoter that looks like it works. Every trigger therefore has a written
//!    fallback that is worth sending on its own.
//! 3. **How a sale gets attributed back.** A post with no attribution cannot
//!    tell you whether it sold anything, and an unmeasurable promotion is not
//!    a sales lever, it is a hope. Every post carries a deep link whose payload
//!    the app already understands.

use chrono::Datelike;

/// A reason to write a post.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Subject {
    // `Strain` stood here until 2026-09-16. Nothing constructed it: the only
    // producer, `src/promo.rs`, stopped scanning the `strains` table when 083
    // dropped it. Its arms lived on, and three of them still named that table
    // in SQL — `SELECT price_per_gram FROM strains` — so the one thing this
    // variant could still do was raise a relation-does-not-exist error if
    // anybody ever built one. `fallback_copy` would have announced it with a
    // cannabis leaf.
    Accessory {
        id: String,
        name: String,
    },
    Tea {
        id: String,
        name: String,
    },
    Set {
        id: String,
        name: String,
    },
    Event {
        id: String,
        name: String,
    },
    /// Something that already sells, promoted again.
    ///
    /// The agent was motivated by newness, which is not a sales signal: the
    /// best-selling set in the shop is never news and is the thing most worth
    /// posting about. `period` is what keeps this from repeating forever —
    /// it goes into the dedup key, so the same bestseller can return at most
    /// once per period and never twice in one.
    Bestseller {
        id: String,
        name: String,
        kind: BestsellerKind,
        /// Units sold in the window that qualified it. Goes to the model as a
        /// fact and is never invented.
        sold: i64,
        /// `2026-W34`. See `weekly_period`.
        period: String,
    },
    /// An event happening soon. Separate from `Event` because the reason to
    /// post is the clock, not the row appearing, and the same event legitimately
    /// gets both — once when it is announced and once the day before.
    EventSoon {
        id: String,
        name: String,
        /// `09:00 – 12:00`, already formatted by `crate::trios::calendar`.
        when: String,
        seats_left: Option<i32>,
    },
}

/// Which catalog a bestseller lives in, so its link opens the right screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BestsellerKind {
    // `Strain` stood here too, and `src/promo.rs:334` had already stopped
    // producing it — the loop it lives in reads a one-element list with the
    // comment "`strains` left with 083; a strain bestseller can no longer
    // resolve." Dropping the variant is what makes the compiler agree.
    Set,
}

/// The period a recurring post belongs to.
///
/// ISO week rather than "30 days ago", because a rolling window makes the
/// dedup key move every tick and the same bestseller would be posted every
/// fifteen minutes. A week is a bucket: one post per bestseller per week, and
/// the key says which week it was.
pub fn weekly_period(now: chrono::DateTime<chrono::Utc>) -> String {
    let iso = now.iso_week();
    format!("{}-W{:02}", iso.year(), iso.week())
}

impl Subject {
    pub fn id(&self) -> &str {
        match self {
            Subject::Accessory { id, .. }
            | Subject::Tea { id, .. }
            | Subject::Set { id, .. }
            | Subject::Event { id, .. }
            | Subject::Bestseller { id, .. }
            | Subject::EventSoon { id, .. } => id,
        }
    }

    pub fn name(&self) -> &str {
        match self {
            Subject::Accessory { name, .. }
            | Subject::Tea { name, .. }
            | Subject::Set { name, .. }
            | Subject::Event { name, .. }
            | Subject::Bestseller { name, .. }
            | Subject::EventSoon { name, .. } => name,
        }
    }

    /// The word used in the dedup key and in logs. Stable — changing one
    /// re-promotes everything of that kind, because the key would no longer
    /// match what is already recorded.
    pub fn kind(&self) -> &'static str {
        match self {
            Subject::Accessory { .. } => "accessory",
            Subject::Tea { .. } => "tea",
            Subject::Set { .. } => "set",
            Subject::Event { .. } => "event",
            Subject::EventSoon { .. } => "event_soon",
            Subject::Bestseller { .. } => "bestseller",
        }
    }

    /// The deep-link target that opens this exact thing in the Mini App.
    pub fn deeplink_target(&self) -> Option<crate::trios::deeplink::Target> {
        use crate::trios::deeplink::{Kind, Target};
        let kind = match self {
            Subject::Accessory { .. } => Kind::Accessory,
            Subject::Tea { .. } => Kind::Tea,
            Subject::Set { .. } => Kind::Set,
            Subject::Event { .. } | Subject::EventSoon { .. } => Kind::Event,
            Subject::Bestseller { kind, .. } => match kind {
                BestsellerKind::Set => Kind::Set,
            },
        };
        Some(Target::Product {
            kind,
            id: self.id().to_string(),
            // The campaign this post is. Without it the link is
            // indistinguishable from a customer sharing the same card, and the
            // agent could never tell which of its posts sold anything.
            source: Some(attribution(self)),
        })
    }
}

/// What the sweeper writes down so the same thing is never promoted twice.
///
/// `kind` is in the key, not just the id, because `EventSoon` and `Event` are
/// two legitimate posts about one row. Keyed on the id alone, announcing an
/// event would silently suppress its reminder.
pub fn dedup_key(subject: &Subject) -> String {
    match subject {
        // A recurring post is keyed by period as well, so it can come back
        // next week and cannot come back twice this week.
        Subject::Bestseller { id, period, .. } => {
            format!("{}:{}:{}", subject.kind(), id, period)
        }
        _ => format!("{}:{}", subject.kind(), subject.id()),
    }
}

/// Longest post the promoter will send.
///
/// Telegram caps a photo caption at 1024 characters and rejects the whole
/// message over it — so an over-long caption is not a long post, it is no post.
pub const MAX_POST_LEN: usize = 900;

/// The instruction given to the model.
///
/// Written as rules rather than a vibe, because the output goes out under the
/// shop's name: no invented facts (the model does not know the THC figure, the
/// price or the schedule unless it is in the prompt), no invented discounts,
/// and no claims about effects that would be a legal problem for a cannabis
/// shop to publish.
pub fn prompt_for(subject: &Subject, lang: &str, facts: &str) -> String {
    let what = match subject {
        Subject::Accessory { name, .. } => format!("новый аксессуар «{name}»"),
        Subject::Tea { name, .. } => format!("новая позиция в напитках — «{name}»"),
        Subject::Set { name, .. } => format!("новый набор «{name}» со скидкой"),
        Subject::Event { name, .. } => format!("новое мероприятие «{name}»"),
        Subject::Bestseller { name, sold, .. } => {
            format!("«{name}» — один из самых заказываемых за неделю ({sold} шт.)")
        }
        Subject::EventSoon {
            name,
            when,
            seats_left,
            ..
        } => {
            let seats = match seats_left {
                Some(n) if *n > 0 => format!(", осталось мест: {n}"),
                Some(_) => ", мест не осталось".to_string(),
                None => String::new(),
            };
            format!("мероприятие «{name}» уже завтра, {when}{seats}")
        }
    };

    format!(
        "Ты пишешь короткий пост для Telegram-канала проката мотоциклов и \
         скутеров TurboBaby в Камале на Пхукете. Повод: {what}.\n\n\
         Факты, которыми можно пользоваться (других у тебя нет):\n{facts}\n\n\
         Правила:\n\
         - Пиши на языке: {lang}.\n\
         - Не больше 4 коротких строк, до {MAX_POST_LEN} символов.\n\
         - Ничего не выдумывай: ни цен, ни скидок, ни процентов, ни дат, ни \
           эффектов — только то, что дано выше.\n\
         - Не обещай медицинского или лечебного действия.\n\
         - Один эмодзи в начале, не больше двух во всём тексте.\n\
         - Заканчивай одним призывом открыть приложение.\n\
         - Верни только текст поста, без кавычек и без пояснений."
    )
}

/// The post to send when the model is unavailable or answers badly.
///
/// Not a placeholder. `GLM_API_KEY` is unset in production and the boot log
/// says so on every start, so this is the text that actually goes out today.
/// It is written to be worth sending on its own: what it is, and one reason to
/// tap.
pub fn fallback_copy(subject: &Subject) -> String {
    match subject {
        Subject::Accessory { name, .. } => {
            format!("📦 Новый аксессуар — {name}\n\nЗабрать можно вместе с заказом.")
        }
        Subject::Tea { name, .. } => {
            format!("🥤 Новое в напитках — {name}\n\nПопробуйте в заведении или с собой.")
        }
        Subject::Set { name, .. } => {
            format!("🎁 Новый набор — {name}\n\nСобран со скидкой к обычной цене.")
        }
        Subject::Event { name, .. } => {
            format!("📅 Новое мероприятие — {name}\n\nМеста можно занять заранее.")
        }
        Subject::Bestseller { name, sold, .. } => {
            format!("🔥 Берут чаще всего — {name}\n\nЗа неделю заказали {sold} раз.")
        }
        Subject::EventSoon {
            name,
            when,
            seats_left,
            ..
        } => {
            let tail = match seats_left {
                Some(n) if *n > 0 => format!("\nОсталось мест: {n}."),
                Some(_) => "\nСвободных мест не осталось.".to_string(),
                None => String::new(),
            };
            format!("⏰ Уже завтра — {name}\n{when}{tail}")
        }
    }
}

/// Keep a model's answer, or fall back.
///
/// A model that returns an apology, an empty string or a wall of text has not
/// written a post, and sending one of those under the shop's name is worse
/// than sending the written fallback. Length is measured in characters rather
/// than bytes: Telegram's caption limit is characters, and a Cyrillic post cut
/// on bytes would be cut mid-letter.
pub fn usable_copy(model_output: &str, subject: &Subject) -> String {
    let text = model_output.trim();
    let too_short = text.chars().count() < 12;
    let too_long = text.chars().count() > MAX_POST_LEN;
    // A model declining the task usually says so in the first line.
    let refused = {
        let head: String = text.chars().take(80).collect::<String>().to_lowercase();
        [
            "не могу",
            "как ии",
            "как язык",
            "i cannot",
            "i'm sorry",
            "as an ai",
        ]
        .iter()
        .any(|m| head.contains(m))
    };
    if too_short || too_long || refused {
        return fallback_copy(subject);
    }
    text.to_string()
}

/// What a post is worth, so the best one goes out first.
///
/// The agent used to take whatever was oldest. Age is not a sales signal: a
/// 150 ฿ accessory added on Monday beat a 1200 ฿ set added on Tuesday, and the
/// per-tick cap meant the set might wait hours. This ranks by the money a post
/// can plausibly move.
///
/// Deliberately crude, and crude in a way that is honest: the shop has no
/// margin data, so price stands in for value, and the multipliers below are
/// stated preferences rather than measurements. They are here to be replaced
/// once `promo_posts` has enough published rows to say which kind actually
/// converts — which is what the attribution shipped in #94 is for.
pub fn score(subject: &Subject, price_baht: Option<f64>) -> f64 {
    let price = price_baht
        .filter(|p| p.is_finite() && *p > 0.0)
        .unwrap_or(0.0);

    let weight = match subject {
        // A sold-out event is worth nothing to post about: there is nothing
        // left to sell and the post would spend the day's attention on it.
        Subject::EventSoon {
            seats_left: Some(0),
            ..
        } => return 0.0,
        // Perishable. A seat unsold when the event starts is revenue that
        // cannot be recovered, so among things of similar value it goes first.
        Subject::EventSoon { .. } => 3.0,
        // The biggest single basket the shop sells.
        Subject::Set { .. } => 2.0,
        // Already proven to sell; the only open question is reach.
        Subject::Bestseller { sold, .. } => 1.8 + (*sold as f64).min(50.0) * 0.02,
        Subject::Event { .. } => 1.5,
        Subject::Tea { .. } => 0.6,
        Subject::Accessory { .. } => 0.5,
    };

    // Price enters logarithmically. A 2000 ฿ set is worth more to post about
    // than a 300 ฿ strain, but not seven times more — far fewer people buy it.
    // Under a linear or even square-root price, one expensive item would win
    // every tick forever and the rest of the shop would never be posted.
    // With `ln`, a 300x price difference is barely a 2x difference in rank.
    weight * (1.0 + (1.0 + price).ln())
}

/// Best first.
///
/// Ties keep their original order, which is oldest-first — so among equals the
/// thing that has waited longest still goes first.
pub fn rank(mut candidates: Vec<(Subject, Option<f64>)>) -> Vec<Subject> {
    candidates.sort_by(|a, b| {
        score(&b.0, b.1)
            .partial_cmp(&score(&a.0, a.1))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    candidates.into_iter().map(|(s, _)| s).collect()
}

/// The attribution source recorded against this post.
///
/// The point of the whole agent is more sales, and a post with no attribution
/// cannot say whether it produced any. This travels in the deep link, the app
/// reports it as a client event, and an order placed in that session carries it
/// — the same mechanism `cart__<source>` already uses.
pub fn attribution(subject: &Subject) -> String {
    format!("promo_{}", subject.kind())
}

/// One published post's measured result, as `/promo` shows it to the owner.
///
/// A mirror of the backend's `PromoResult` under a platform-free name: this
/// module compiles for WASM too, and the report row type lives behind the
/// backend gate next to the query that produces it.
pub struct DigestRow {
    pub kind: String,
    pub name: String,
    pub openers: i32,
    pub orders: i32,
    pub revenue: f64,
}

/// Rows one digest message shows at most.
///
/// Telegram rejects a message over 4096 characters outright, and a month of
/// posts with long names would sail past it — an uncapped digest is a command
/// that stops working on the first busy month. When the cap bites, the message
/// says so: a silent cap reads as "that was everything".
pub const DIGEST_MAX_ROWS: usize = 25;

/// Longest name shown, measured in characters like every Telegram limit here.
pub const DIGEST_MAX_NAME_CHARS: usize = 32;

/// The icon a kind shows in the digest — the same one its posts use, so the
/// owner reads a row the way they already read the drafts.
fn digest_icon(kind: &str) -> &'static str {
    match kind {
        "set" => "🎁",
        "strain" => "🌿",
        "event" => "📅",
        "event_soon" => "⏰",
        "bestseller" => "🔥",
        "tea" => "🥤",
        "accessory" => "📦",
        _ => "📣",
    }
}

/// Russian plural for a count: `1 пост`, `2 поста`, `5 постов`.
fn ru_plural(n: i64, one: &str, few: &str, many: &str) -> String {
    let abs = n.abs();
    let last_two = abs % 100;
    let word = if (11..=14).contains(&last_two) {
        many
    } else {
        match abs % 10 {
            1 => one,
            2..=4 => few,
            _ => many,
        }
    };
    format!("{n} {word}")
}

/// Baht without a tail of zeros: `3900 ฿`, not `3900.00 ฿`.
fn baht(v: f64) -> String {
    if (v - v.round()).abs() < f64::EPSILON {
        format!("{} ฿", v.round() as i64)
    } else {
        format!("{v:.2} ฿")
    }
}

/// Local copy of the escaping `trios::health` also keeps: this module compiles
/// for WASM too, and `crate::util` sits behind the backend gate. The digest is
/// sent with ParseMode::Html, and Telegram rejects malformed HTML outright.
fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// The owner-facing `/promo` digest: what each published post sold.
///
/// Zero posts and zero sales are different states and read differently here —
/// "nothing was published" must not look like "posts sold nothing". The
/// causality caveat travels with every table because the numbers invite
/// exactly the reading they cannot support: orders *after* an opening, not
/// orders *because of* it.
pub fn format_promo_digest(days: i64, rows: &[DigestRow]) -> String {
    let mut out = format!("📊 Промо за {}\n", ru_plural(days, "день", "дня", "дней"));
    if rows.is_empty() {
        out.push_str("\nЗа этот период не опубликовано ни одного поста.");
        return out;
    }

    let total_orders: i64 = rows.iter().map(|r| r.orders as i64).sum();
    let total_revenue: f64 = rows.iter().map(|r| r.revenue).sum();

    out.push('\n');
    for row in rows.iter().take(DIGEST_MAX_ROWS) {
        // The reminder is a different post about the same row the announcement
        // already covered; the suffix keeps the owner from reading two rows as
        // two events.
        let reminder = if row.kind == "event_soon" {
            " (напом.)"
        } else {
            ""
        };
        let name: String = row.name.chars().take(DIGEST_MAX_NAME_CHARS).collect();
        let cut = if row.name.chars().count() > DIGEST_MAX_NAME_CHARS {
            "…"
        } else {
            ""
        };
        out.push_str(&format!(
            "{} {}{cut}{reminder} — {} откр · {} зак · {}\n",
            digest_icon(&row.kind),
            escape_html(&name),
            row.openers,
            row.orders,
            baht(row.revenue),
        ));
    }
    if rows.len() > DIGEST_MAX_ROWS {
        out.push_str(&format!(
            "… и ещё {}\n",
            ru_plural(
                (rows.len() - DIGEST_MAX_ROWS) as i64,
                "пост",
                "поста",
                "постов"
            )
        ));
    }

    out.push_str(&format!(
        "\nИтого: {} · {} · {}\n",
        ru_plural(rows.len() as i64, "пост", "поста", "постов"),
        ru_plural(total_orders, "заказ", "заказа", "заказов"),
        baht(total_revenue),
    ));
    out.push_str(
        "\n⚠️ Заказы в течение 24 ч после открытия ссылки,\n\
         тем же покупателем — верхняя оценка, не причинность.",
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn every_subject() -> Vec<Subject> {
        vec![
            Subject::Accessory {
                id: "a1".into(),
                name: "Asia 420".into(),
            },
            Subject::Tea {
                id: "t1".into(),
                name: "Americano".into(),
            },
            Subject::Set {
                id: "k1".into(),
                name: "Snickers Cake".into(),
            },
            Subject::Event {
                id: "e1".into(),
                name: "UFC NIGHT".into(),
            },
            Subject::EventSoon {
                id: "e1".into(),
                name: "UFC NIGHT".into(),
                when: "20:00 – 23:00".into(),
                seats_left: Some(4),
            },
            // `Bestseller` was missing from this list until 2026-09-16, so the
            // two checks below — that every reason to post has copy of its own,
            // and that no two share an attribution source — ran over six of the
            // seven variants and called it every. A helper named `every_*` that
            // is hand-written is a claim, not a fact; the compiler cannot check
            // it, which is why the sweep over `Subject` below now can.
            Subject::Bestseller {
                id: "b1".into(),
                name: "Honda Click 125i".into(),
                kind: BestsellerKind::Set,
                sold: 12,
                period: "2026-W38".into(),
            },
        ]
    }

    /// `every_subject` names every variant of `Subject`.
    ///
    /// The list is written by hand and nothing stops it going stale — it had
    /// already missed `Bestseller`. This walks the enum's own source instead,
    /// so a variant added to `Subject` fails here until the fixture covers it.
    #[test]
    fn every_subject_covers_every_variant() {
        let source = include_str!("promo.rs");
        let start = source
            .find("pub enum Subject {")
            .expect("Subject enum declaration");
        let body = &source[start..start + source[start..].find("\n}\n").expect("enum close")];

        let declared: Vec<&str> = body
            .lines()
            .skip(1)
            .filter_map(|line| {
                let t = line.trim();
                t.strip_suffix(" {")
                    .filter(|n| n.chars().next().is_some_and(char::is_uppercase))
            })
            .collect();
        assert!(
            declared.len() >= 6,
            "parsed only {} variants out of `enum Subject` — this check is \
             reading an empty corpus and would pass on anything: {declared:?}",
            declared.len()
        );

        let covered: Vec<String> = every_subject().iter().map(|s| format!("{s:?}")).collect();
        let missing: Vec<&&str> = declared
            .iter()
            .filter(|v| !covered.iter().any(|c| c.starts_with(*v)))
            .collect();
        assert!(
            missing.is_empty(),
            "every_subject() does not name {missing:?} — every check built on \
             it silently skips those variants"
        );
    }

    /// Every reason to post produces a post worth sending without any model.
    ///
    /// `GLM_API_KEY` is unset in production — the boot log says so on every
    /// start — so this is not a corner case, it is today's behaviour.
    #[test]
    fn every_subject_has_copy_that_stands_on_its_own() {
        for s in every_subject() {
            let copy = fallback_copy(&s);
            assert!(
                copy.contains(s.name()),
                "{:?} produced copy that does not name the thing: {copy}",
                s.kind()
            );
            assert!(
                copy.chars().count() <= MAX_POST_LEN,
                "{} copy is {} characters, over the {MAX_POST_LEN} cap",
                s.kind(),
                copy.chars().count()
            );
            assert!(
                copy.chars().count() > 12,
                "{} copy is too short to be a post: {copy}",
                s.kind()
            );
        }
    }

    /// An announcement and a reminder are two posts about one event.
    ///
    /// Keyed on the id alone, announcing an event would silently suppress its
    /// reminder the next day — the post that matters most, because it is the
    /// one with the seats left in it.
    #[test]
    fn an_event_and_its_reminder_are_not_the_same_post() {
        let announced = Subject::Event {
            id: "e1".into(),
            name: "UFC NIGHT".into(),
        };
        let soon = Subject::EventSoon {
            id: "e1".into(),
            name: "UFC NIGHT".into(),
            when: "20:00".into(),
            seats_left: None,
        };
        assert_ne!(dedup_key(&announced), dedup_key(&soon));
        assert_eq!(dedup_key(&announced), "event:e1");
        assert_eq!(dedup_key(&soon), "event_soon:e1");
    }

    /// Two different things never collide, and the same thing always produces
    /// the same key — otherwise the sweeper either re-posts or skips silently.
    #[test]
    fn dedup_keys_are_stable_and_distinct() {
        let keys: std::collections::BTreeSet<String> =
            every_subject().iter().map(dedup_key).collect();
        assert_eq!(keys.len(), every_subject().len(), "keys collided: {keys:?}");
        for s in every_subject() {
            assert_eq!(dedup_key(&s), dedup_key(&s));
        }
    }

    /// Each post opens the exact thing it is about.
    #[test]
    fn every_post_can_link_to_what_it_advertises() {
        for s in every_subject() {
            let target = s
                .deeplink_target()
                .unwrap_or_else(|| panic!("{} has no link", s.kind()));
            let payload = crate::trios::deeplink::payload_for(&target)
                .unwrap_or_else(|| panic!("{} produced an untransportable link", s.kind()));
            assert_eq!(
                crate::trios::deeplink::parse(&payload).as_ref(),
                Some(&target),
                "{} link does not read back",
                s.kind()
            );
            assert!(
                crate::trios::deeplink::is_miniapp_payload(&payload),
                "the bot would not answer {}'s link, so the post leads nowhere",
                s.kind()
            );
        }
    }

    /// A model that refuses, returns nothing, or writes an essay has not
    /// written a post. Sending any of those under the shop's name is worse
    /// than sending the written fallback.
    #[test]
    fn a_bad_answer_never_reaches_a_customer() {
        let s = Subject::Set {
            id: "k1".into(),
            name: "Snickers Cake".into(),
        };
        for bad in [
            "",
            "   ",
            "ok",
            "Не могу помочь с этим запросом.",
            "I'm sorry, but I cannot write promotional content for cannabis.",
            "As an AI language model, I must decline.",
        ] {
            assert_eq!(
                usable_copy(bad, &s),
                fallback_copy(&s),
                "{bad:?} was sent to customers"
            );
        }
        let long = "я".repeat(MAX_POST_LEN + 1);
        assert_eq!(usable_copy(&long, &s), fallback_copy(&s));
    }

    /// A good answer is kept as written — the fallback is a floor, not a
    /// ceiling, and a promoter that always sends its own text has no use for
    /// a model at all.
    #[test]
    fn a_good_answer_is_kept() {
        let s = Subject::Set {
            id: "s1".into(),
            name: "Honda Click 125i".into(),
        };
        let good = "🏍 Honda Click 125i уже в парке\n\nЛёгкий и экономичный — для города.\nОткрыть каталог →";
        assert_eq!(usable_copy(good, &s), good);
        // Whitespace the model left around it is not part of the post.
        assert_eq!(usable_copy(&format!("\n  {good}  \n"), &s), good);
    }

    /// A post that cannot be attributed cannot be shown to have sold anything.
    #[test]
    fn every_post_carries_its_own_attribution() {
        let sources: std::collections::BTreeSet<String> =
            every_subject().iter().map(attribution).collect();
        // One source per reason to post — an event and its reminder included,
        // so "the announcement sold nothing but the reminder sold four" is a
        // sentence the data can support.
        assert_eq!(
            sources.len(),
            every_subject().len(),
            "two reasons to post share one attribution: {sources:?}"
        );
        for s in every_subject() {
            let src = attribution(&s);
            assert!(src.starts_with("promo_"), "{src} is not a promo source");
            // The app's attribution segment allows only these characters; a
            // source with anything else is dropped on arrival and the post
            // becomes unmeasurable without saying so.
            assert!(
                src.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-'),
                "{src} would be rejected by the deep-link parser"
            );
        }
    }

    /// The money decides, not the calendar.
    ///
    /// A cheap accessory added first used to beat an expensive set added
    /// second, and with a per-tick cap the set could wait hours.
    #[test]
    fn an_expensive_set_outranks_a_cheap_accessory_added_earlier() {
        let acc = Subject::Accessory {
            id: "a".into(),
            name: "rolling papers".into(),
        };
        let set = Subject::Set {
            id: "k".into(),
            name: "party pack".into(),
        };
        let order = rank(vec![
            (acc.clone(), Some(150.0)),
            (set.clone(), Some(1200.0)),
        ]);
        assert_eq!(order.first().map(|s| s.kind()), Some("set"), "{order:?}");
    }

    /// Between two events, the one with seats left wins — the other has
    /// nothing left to sell.
    ///
    /// Note what this does NOT claim: that a free event outranks an expensive
    /// set. It does not, and asserting so would be a preference with nothing
    /// behind it. A set moves 2000 ฿ that the data can see; a free event moves
    /// footfall that it cannot. Perishability breaks ties among comparable
    /// things rather than overriding revenue.
    #[test]
    fn between_two_events_the_one_with_seats_left_wins() {
        let with_seats = Subject::EventSoon {
            id: "e1".into(),
            name: "UFC".into(),
            when: "20:00".into(),
            seats_left: Some(6),
        };
        let sold_out = Subject::EventSoon {
            id: "e2".into(),
            name: "Yoga".into(),
            when: "09:00".into(),
            seats_left: Some(0),
        };
        let order = rank(vec![(sold_out, Some(500.0)), (with_seats.clone(), None)]);
        assert_eq!(order.first().map(|s| s.kind()), Some("event_soon"));
        assert_eq!(order.first().map(|s| s.id()), Some("e1"), "{order:?}");
    }

    /// And an event ranks above a strain of the same price — perishable first
    /// among comparable things.
    #[test]
    fn an_expiring_event_beats_an_evergreen_product_of_equal_value() {
        let soon = Subject::EventSoon {
            id: "e".into(),
            name: "UFC".into(),
            when: "20:00".into(),
            seats_left: Some(6),
        };
        let quiet = Subject::Accessory {
            id: "a".into(),
            name: "шлем".into(),
        };
        let order = rank(vec![(quiet, Some(500.0)), (soon, Some(500.0))]);
        assert_eq!(
            order.first().map(|s| s.kind()),
            Some("event_soon"),
            "{order:?}"
        );
    }

    /// A sold-out event is worth nothing to post about — there is nothing left
    /// to sell, and the post would spend the day's attention on it.
    #[test]
    fn a_sold_out_event_scores_zero() {
        let full = Subject::EventSoon {
            id: "e".into(),
            name: "UFC".into(),
            when: "20:00".into(),
            seats_left: Some(0),
        };
        assert_eq!(score(&full, None), 0.0);
        let quiet = Subject::Accessory {
            id: "a".into(),
            name: "шлем".into(),
        };
        assert!(score(&quiet, Some(300.0)) > score(&full, None));
    }

    /// Price must not let one item crowd out the whole shop.
    ///
    /// Stated as a ratio rather than an ordering, because the ordering across
    /// categories is a preference and the flattening is a property: a 333x
    /// price difference must not become a 333x difference in rank, or the gold
    /// grinder is the only thing ever posted.
    #[test]
    fn one_expensive_item_does_not_dwarf_the_whole_shop() {
        let cheap = Subject::Accessory {
            id: "a1".into(),
            name: "papers".into(),
        };
        let dear = Subject::Accessory {
            id: "a2".into(),
            name: "gold grinder".into(),
        };
        let (lo, hi) = (score(&cheap, Some(150.0)), score(&dear, Some(50_000.0)));
        assert!(hi > lo, "the expensive one should still rank higher");
        assert!(
            hi / lo < 2.5,
            "a 333x price difference became a {:.1}x rank difference; one item \
             would win every tick forever",
            hi / lo
        );
    }

    /// A recurring post returns next week and not twice this week.
    #[test]
    fn a_bestseller_can_come_back_but_not_twice_in_one_week() {
        let mk = |period: &str| Subject::Bestseller {
            id: "k1".into(),
            name: "Snickers Cake".into(),
            kind: BestsellerKind::Set,
            sold: 12,
            period: period.into(),
        };
        assert_eq!(dedup_key(&mk("2026-W34")), dedup_key(&mk("2026-W34")));
        assert_ne!(dedup_key(&mk("2026-W34")), dedup_key(&mk("2026-W35")));
        assert!(dedup_key(&mk("2026-W34")).contains("2026-W34"));
    }

    /// The period is a bucket, not a rolling window — a rolling one would move
    /// on every tick and repost the same bestseller every fifteen minutes.
    #[test]
    fn the_period_is_stable_within_a_week() {
        let monday = chrono::DateTime::parse_from_rfc3339("2026-08-17T01:00:00+00:00")
            .expect("date")
            .with_timezone(&chrono::Utc);
        let friday = chrono::DateTime::parse_from_rfc3339("2026-08-21T23:00:00+00:00")
            .expect("date")
            .with_timezone(&chrono::Utc);
        let next_monday = chrono::DateTime::parse_from_rfc3339("2026-08-24T01:00:00+00:00")
            .expect("date")
            .with_timezone(&chrono::Utc);
        assert_eq!(weekly_period(monday), weekly_period(friday));
        assert_ne!(weekly_period(monday), weekly_period(next_monday));
    }

    /// The prompt must carry the facts and must forbid inventing the rest.
    /// The model does not know the price, and a made-up discount is a promise
    /// the shop has to honour.
    #[test]
    fn the_prompt_forbids_inventing_what_it_was_not_given() {
        let s = Subject::Set {
            id: "k1".into(),
            name: "Snickers Cake".into(),
        };
        let p = prompt_for(&s, "ru", "Цена: 950 ฿. В наборе: 5 сортов.");
        assert!(p.contains("Snickers Cake"));
        assert!(p.contains("950"), "the facts were not passed to the model");
        assert!(
            p.contains("выдумывай"),
            "nothing in the prompt stops the model inventing a price: {p}"
        );
        assert!(
            p.contains("медицинского"),
            "no rule against health claims, which a cannabis shop cannot publish"
        );
    }

    fn row(kind: &str, name: &str, openers: i32, orders: i32, revenue: f64) -> DigestRow {
        DigestRow {
            kind: kind.into(),
            name: name.into(),
            openers,
            orders,
            revenue,
        }
    }

    /// Zero posts and zero sales must not read the same: "nothing was
    /// published" is a pipeline state, "posts sold nothing" is a sales
    /// result, and confusing them sends the owner looking for the wrong bug.
    #[test]
    fn a_month_without_posts_says_so_without_a_table() {
        let d = format_promo_digest(30, &[]);
        assert!(d.contains("не опубликовано ни одного поста"));
        assert!(!d.contains("Итого"), "an empty month rendered a total: {d}");
        assert!(
            !d.contains("причинность"),
            "a caveat about orders makes no sense when there were no posts: {d}"
        );
    }

    #[test]
    fn posts_with_no_sales_still_show_the_table_and_the_caveat() {
        let d = format_promo_digest(30, &[row("event_soon", "UFC NIGHT", 21, 0, 0.0)]);
        assert!(d.contains("⏰ UFC NIGHT (напом.) — 21 откр · 0 зак · 0 ฿"));
        assert!(d.contains("1 пост · 0 заказов · 0 ฿"));
        assert!(d.contains("верхняя оценка, не причинность"));
    }

    /// The whole point of the digest is the money line, and the caveat is the
    /// only thing stopping it from being read as causation. A digest that
    /// loses either has stopped being an answer.
    #[test]
    fn rows_render_with_icons_and_the_causality_caveat() {
        let rows = [
            row("set", "Party Pack", 12, 3, 3600.0),
            row("strain", "DA FUNK", 8, 1, 300.0),
            row("event_soon", "UFC NIGHT", 21, 0, 0.0),
        ];
        let d = format_promo_digest(30, &rows);
        assert!(d.starts_with("📊 Промо за 30 дней"));
        assert!(d.contains("🎁 Party Pack — 12 откр · 3 зак · 3600 ฿"));
        assert!(d.contains("🌿 DA FUNK — 8 откр · 1 зак · 300 ฿"));
        assert!(d.contains("Итого: 3 поста · 4 заказа · 3900 ฿"));
        assert!(d.contains("24 ч после открытия ссылки"));
    }

    /// Names come from the shop's data and go into an HTML-parsed message.
    #[test]
    fn names_are_html_escaped() {
        let d = format_promo_digest(30, &[row("tea", "<b>Americano</b>", 1, 0, 0.0)]);
        assert!(
            d.contains("&lt;b&gt;Americano&lt;/b&gt;"),
            "raw HTML leaked: {d}"
        );
        assert!(!d.contains("<b>"));
    }

    /// The message is sent with ParseMode::Html — over 4096 characters
    /// Telegram rejects it and the command just fails. The cap must be
    /// spoken, not silent: a silent cap reads as "that was everything".
    #[test]
    fn a_long_month_is_capped_out_loud_and_fits_telegram() {
        let rows: Vec<DigestRow> = (0..40)
            .map(|i| {
                row(
                    "strain",
                    &format!("Very Long Strain Name Number {i}"),
                    3,
                    1,
                    100.0,
                )
            })
            .collect();
        let d = format_promo_digest(30, &rows);
        assert!(
            d.contains("… и ещё 15 постов"),
            "the cap was silent: tail of {d}"
        );
        assert!(d.chars().count() < 4096, "digest would not send");
        // All 40 still count in the totals — capped display, not capped truth.
        assert!(d.contains("40 постов"));
    }

    /// `Итого` must agree with the rows it stands over; a total computed from
    /// the capped 25 instead of all rows would understate the month.
    #[test]
    fn the_total_counts_capped_rows_too() {
        let rows: Vec<DigestRow> = (0..30)
            .map(|_| row("accessory", "Grinder", 1, 1, 50.0))
            .collect();
        let d = format_promo_digest(30, &rows);
        assert!(d.contains("1500 ฿"), "total ignored capped rows: {d}");
    }
}
