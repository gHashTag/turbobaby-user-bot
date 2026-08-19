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

/// A reason to write a post.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Subject {
    Strain {
        id: String,
        name: String,
    },
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

impl Subject {
    pub fn id(&self) -> &str {
        match self {
            Subject::Strain { id, .. }
            | Subject::Accessory { id, .. }
            | Subject::Tea { id, .. }
            | Subject::Set { id, .. }
            | Subject::Event { id, .. }
            | Subject::EventSoon { id, .. } => id,
        }
    }

    pub fn name(&self) -> &str {
        match self {
            Subject::Strain { name, .. }
            | Subject::Accessory { name, .. }
            | Subject::Tea { name, .. }
            | Subject::Set { name, .. }
            | Subject::Event { name, .. }
            | Subject::EventSoon { name, .. } => name,
        }
    }

    /// The word used in the dedup key and in logs. Stable — changing one
    /// re-promotes everything of that kind, because the key would no longer
    /// match what is already recorded.
    pub fn kind(&self) -> &'static str {
        match self {
            Subject::Strain { .. } => "strain",
            Subject::Accessory { .. } => "accessory",
            Subject::Tea { .. } => "tea",
            Subject::Set { .. } => "set",
            Subject::Event { .. } => "event",
            Subject::EventSoon { .. } => "event_soon",
        }
    }

    /// The deep-link target that opens this exact thing in the Mini App.
    pub fn deeplink_target(&self) -> Option<crate::trios::deeplink::Target> {
        use crate::trios::deeplink::{Kind, Target};
        let kind = match self {
            Subject::Strain { .. } => Kind::Strain,
            Subject::Accessory { .. } => Kind::Accessory,
            Subject::Tea { .. } => Kind::Tea,
            Subject::Set { .. } => Kind::Set,
            Subject::Event { .. } | Subject::EventSoon { .. } => Kind::Event,
        };
        Some(Target::Product {
            kind,
            id: self.id().to_string(),
        })
    }
}

/// What the sweeper writes down so the same thing is never promoted twice.
///
/// `kind` is in the key, not just the id, because `EventSoon` and `Event` are
/// two legitimate posts about one row. Keyed on the id alone, announcing an
/// event would silently suppress its reminder.
pub fn dedup_key(subject: &Subject) -> String {
    format!("{}:{}", subject.kind(), subject.id())
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
        Subject::Strain { name, .. } => format!("новый сорт «{name}» в меню"),
        Subject::Accessory { name, .. } => format!("новый аксессуар «{name}»"),
        Subject::Tea { name, .. } => format!("новая позиция в напитках — «{name}»"),
        Subject::Set { name, .. } => format!("новый набор «{name}» со скидкой"),
        Subject::Event { name, .. } => format!("новое мероприятие «{name}»"),
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
        "Ты пишешь короткий пост для Telegram-канала магазина WoodyWeedPecker на \
         острове Панган. Повод: {what}.\n\n\
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
        Subject::Strain { name, .. } => {
            format!("🌿 Новинка в меню — {name}\n\nУже доступен к заказу.")
        }
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

/// The attribution source recorded against this post.
///
/// The point of the whole agent is more sales, and a post with no attribution
/// cannot say whether it produced any. This travels in the deep link, the app
/// reports it as a client event, and an order placed in that session carries it
/// — the same mechanism `cart__<source>` already uses.
pub fn attribution(subject: &Subject) -> String {
    format!("promo_{}", subject.kind())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn every_subject() -> Vec<Subject> {
        vec![
            Subject::Strain {
                id: "s1".into(),
                name: "DA FUNK".into(),
            },
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
        ]
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
        let s = Subject::Strain {
            id: "s1".into(),
            name: "DA FUNK".into(),
        };
        let good = "🌿 DA FUNK уже в меню\n\nПлотный индика-доминант для вечера.\nОткрыть меню →";
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
}
