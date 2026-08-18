//! How a person is named on screen.
//!
//! The garden's friends panel printed **Friend #6794** for everybody. That was
//! not a formatting choice — `user_languages.first_name` was NULL, because the
//! only function that writes it, `save_user_name`, had no callers at all. The
//! name was never recorded, so `COALESCE(first_name, 'Friend')` did the only
//! thing it could.
//!
//! Naming is decided here rather than in the query or in the component, because
//! both of those are invisible to `cargo test`: `src/db` is behind the
//! `backend` feature and `src/ui` is `#[cfg(target_arch = "wasm32")]`. A rule
//! nobody can execute is a rule nobody has checked.
//!
//! Three states, not two. "No name" is its own case and must not be dressed up
//! as one — that is what produced a screen full of identical strangers.

/// What is known about a person, straight from Telegram.
///
/// Every field is optional because every field genuinely can be missing:
/// `last_name` and `username` are optional in Telegram itself, and `first_name`
/// is absent for anybody whose identity was never captured.
#[derive(Debug, Clone, Copy, Default)]
pub struct Known<'a> {
    pub first_name: Option<&'a str>,
    pub last_name: Option<&'a str>,
    pub username: Option<&'a str>,
}

/// What to print.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Naming {
    /// A real name, and the handle to show beside it when there is one.
    Named {
        name: String,
        handle: Option<String>,
    },
    /// No name was ever recorded, but the handle is a person — print it alone.
    HandleOnly { handle: String },
    /// Nothing is known. The caller prints its own localized word for "friend"
    /// followed by this suffix, so two unknown people stay distinguishable.
    Anonymous { suffix: String },
}

/// Longest name rendered. A display name is other people's free text arriving
/// in a 11px flex row; Telegram allows 64 characters per part, and two of them
/// plus a space overflow the panel long before they overflow the column.
pub const MAX_NAME_LEN: usize = 48;

/// Telegram caps usernames at 32 characters.
pub const MAX_HANDLE_LEN: usize = 32;

/// Empty and whitespace-only are the same thing as absent.
///
/// This matters more than it looks: `first_name` is `TEXT` with no `NOT NULL`
/// and no `CHECK`, so `''` reaches this code from a legacy row and would
/// otherwise render as a nameless person with a stray space.
fn present(s: Option<&str>) -> Option<&str> {
    s.map(str::trim).filter(|s| !s.is_empty())
}

/// A Telegram username, normalised, or nothing.
///
/// Accepts a leading `@` because that is how the handle is stored in
/// `orders.customer_telegram` — migration 070 had to strip it there too.
/// Rejects anything outside Telegram's own alphabet rather than printing it:
/// the string is rendered with an `@` glued to the front, and `@` in front of
/// arbitrary text reads as a handle you could tap, which this one would not be.
pub fn handle(raw: Option<&str>) -> Option<String> {
    let s = present(raw)?;
    // Exactly one `@`, because exactly one is the storage convention. A string
    // like `@@woody` is not a handle somebody typed, it is a value that has
    // been prefixed twice, and normalising it away would hide that.
    let s = s.strip_prefix('@').unwrap_or(s).trim();
    if s.is_empty() || s.len() > MAX_HANDLE_LEN {
        return None;
    }
    if !s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return None;
    }
    Some(s.to_string())
}

/// The stable four-digit suffix that keeps two unknown people apart.
///
/// `rem_euclid` rather than `%` so a negative id — Telegram uses them for
/// channels and groups — cannot produce `#-794`.
pub fn suffix(telegram_id: i64) -> String {
    format!("#{:04}", telegram_id.rem_euclid(10_000))
}

/// Decide how to print somebody.
pub fn name_for(known: &Known<'_>, telegram_id: i64) -> Naming {
    let handle = handle(known.username);

    let mut parts: Vec<&str> = Vec::with_capacity(2);
    if let Some(f) = present(known.first_name) {
        parts.push(f);
    }
    if let Some(l) = present(known.last_name) {
        parts.push(l);
    }

    if parts.is_empty() {
        return match handle {
            Some(h) => Naming::HandleOnly { handle: h },
            None => Naming::Anonymous {
                suffix: suffix(telegram_id),
            },
        };
    }

    let joined = parts.join(" ");
    let name = if joined.chars().count() > MAX_NAME_LEN {
        joined.chars().take(MAX_NAME_LEN).collect::<String>()
    } else {
        joined
    };
    Naming::Named { name, handle }
}

/// One line, for callers that have nowhere to put a second field.
///
/// `friend_word` is the caller's own translation — this module holds no
/// language, so the Russian screen does not have to print an English noun the
/// way the old `COALESCE(first_name, 'Friend')` did.
pub fn one_line(known: &Known<'_>, telegram_id: i64, friend_word: &str) -> String {
    match name_for(known, telegram_id) {
        Naming::Named {
            name,
            handle: Some(h),
        } => format!("{name} @{h}"),
        Naming::Named { name, handle: None } => name,
        Naming::HandleOnly { handle } => format!("@{handle}"),
        Naming::Anonymous { suffix } => format!("{friend_word} {suffix}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ID: i64 = 856_794;

    fn known<'a>(f: Option<&'a str>, l: Option<&'a str>, u: Option<&'a str>) -> Known<'a> {
        Known {
            first_name: f,
            last_name: l,
            username: u,
        }
    }

    /// All eight combinations of the three optional fields, exhaustively —
    /// this is a small enough space that sampling it would be a choice to
    /// leave cases unchecked.
    #[test]
    fn every_combination_of_what_is_known() {
        let cases: [(Option<&str>, Option<&str>, Option<&str>, Naming); 8] = [
            (
                None,
                None,
                None,
                Naming::Anonymous {
                    suffix: "#6794".into(),
                },
            ),
            (
                None,
                None,
                Some("woody"),
                Naming::HandleOnly {
                    handle: "woody".into(),
                },
            ),
            (
                None,
                Some("Ivanov"),
                None,
                Naming::Named {
                    name: "Ivanov".into(),
                    handle: None,
                },
            ),
            (
                None,
                Some("Ivanov"),
                Some("woody"),
                Naming::Named {
                    name: "Ivanov".into(),
                    handle: Some("woody".into()),
                },
            ),
            (
                Some("Дмитрий"),
                None,
                None,
                Naming::Named {
                    name: "Дмитрий".into(),
                    handle: None,
                },
            ),
            (
                Some("Дмитрий"),
                None,
                Some("woody"),
                Naming::Named {
                    name: "Дмитрий".into(),
                    handle: Some("woody".into()),
                },
            ),
            (
                Some("Дмитрий"),
                Some("Васильев"),
                None,
                Naming::Named {
                    name: "Дмитрий Васильев".into(),
                    handle: None,
                },
            ),
            (
                Some("Дмитрий"),
                Some("Васильев"),
                Some("woody"),
                Naming::Named {
                    name: "Дмитрий Васильев".into(),
                    handle: Some("woody".into()),
                },
            ),
        ];
        for (f, l, u, want) in cases {
            assert_eq!(
                name_for(&known(f, l, u), ID),
                want,
                "first={f:?} last={l:?} username={u:?}"
            );
        }
    }

    /// The reported screen, and the assertion that discriminates the fix from
    /// the defect: with a name recorded, nothing anonymous is printed.
    #[test]
    fn a_friend_with_a_name_is_not_printed_as_a_stranger() {
        let line = one_line(&known(Some("Дмитрий"), None, Some("woody")), ID, "Друг");
        assert_eq!(line, "Дмитрий @woody");
        assert!(
            !line.contains("6794"),
            "the anonymous handle leaked into a named person: {line}"
        );
    }

    /// And the state that must survive: somebody genuinely unknown is still
    /// distinguishable from the next unknown person, in the caller's language.
    #[test]
    fn nothing_known_falls_back_and_stays_distinguishable() {
        assert_eq!(one_line(&known(None, None, None), ID, "Друг"), "Друг #6794");
        assert_ne!(
            one_line(&known(None, None, None), 111, "Друг"),
            one_line(&known(None, None, None), 222, "Друг"),
            "two different strangers printed the same string"
        );
    }

    /// `''` is what a legacy row holds, and it is not a name.
    #[test]
    fn empty_and_blank_are_absent() {
        for blank in ["", "   ", "\t", "\n "] {
            assert_eq!(
                name_for(&known(Some(blank), Some(blank), Some(blank)), ID),
                Naming::Anonymous {
                    suffix: "#6794".into()
                },
                "blank {blank:?} was treated as a name"
            );
        }
        // A blank first name must not leave a leading space on the last name.
        assert_eq!(
            name_for(&known(Some("  "), Some("Ivanov"), None), ID),
            Naming::Named {
                name: "Ivanov".into(),
                handle: None
            }
        );
    }

    /// `orders.customer_telegram` stores the handle with its `@`; migration 070
    /// had to strip it there and so does this.
    #[test]
    fn a_stored_at_sign_is_not_doubled() {
        assert_eq!(handle(Some("@woody")).as_deref(), Some("woody"));
        assert_eq!(handle(Some("  @woody  ")).as_deref(), Some("woody"));
        assert_eq!(
            one_line(&known(None, None, Some("@woody")), ID, "Друг"),
            "@woody"
        );
    }

    /// An `@` glued to arbitrary text reads as a tappable handle. Anything
    /// outside Telegram's alphabet is dropped rather than dressed up as one.
    #[test]
    fn only_a_real_handle_is_printed_as_a_handle() {
        for bad in [
            "not a handle",
            "woody!",
            "во́ди",
            "@@woody",
            "woody.dev",
            "a-b",
            "@",
            "  @  ",
        ] {
            assert_eq!(handle(Some(bad)), None, "{bad:?} was printed as a handle");
        }
        for good in ["woody", "Woody_Dev", "a", "_", "x9"] {
            assert_eq!(handle(Some(good)).as_deref(), Some(good), "{good:?}");
        }
    }

    /// Telegram's own cap, enforced rather than assumed.
    #[test]
    fn an_overlong_handle_is_not_a_handle() {
        let at_cap = "a".repeat(MAX_HANDLE_LEN);
        assert_eq!(handle(Some(&at_cap)).as_deref(), Some(at_cap.as_str()));
        assert_eq!(handle(Some(&"a".repeat(MAX_HANDLE_LEN + 1))), None);
    }

    /// A name is other people's free text in an 11px row. It is cut on
    /// characters, not bytes, so a Cyrillic name cannot be split mid-letter.
    #[test]
    fn a_long_name_is_cut_without_breaking_a_letter() {
        let long = "Ж".repeat(200);
        let Naming::Named { name, .. } = name_for(&known(Some(&long), None, None), ID) else {
            panic!("a long name stopped being a name");
        };
        assert_eq!(name.chars().count(), MAX_NAME_LEN);
        assert!(name.chars().all(|c| c == 'Ж'), "cut mid-letter: {name}");
    }

    /// Telegram uses negative ids for channels and groups.
    #[test]
    fn a_negative_id_does_not_print_a_minus_sign() {
        assert_eq!(suffix(-6794), "#3206");
        assert!(!suffix(-1).contains('-'));
        assert_eq!(suffix(0), "#0000");
        assert_eq!(suffix(i64::MIN), "#4192");
    }
}
