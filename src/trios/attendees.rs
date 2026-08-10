//! Turning an event booking into something the owner can tap to reach a person.
//!
//! The admin attendee list used to be a column of bare `telegram_id` numbers,
//! which identifies nobody and reaches nobody. What the owner needs is a
//! handle they can press to open the chat.
//!
//! Lives here rather than next to the admin screen because `src/ui` is
//! `#[cfg(target_arch = "wasm32")]`-gated and tests written there never run.

/// How one attendee should be shown and where tapping them leads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttendeeLink {
    /// Text to display, e.g. `@vibee_dev`.
    pub label: String,
    /// Where tapping goes.
    pub url: String,
    /// False when we only have a numeric id, so the UI can mark the entry as
    /// less useful instead of pretending it is a normal handle.
    pub reachable_by_handle: bool,
}

/// Strip the decorations people type around a handle.
///
/// Usernames arrive from several places — Telegram initData (bare), older
/// orders (`customer_telegram`, often with a leading `@`), and hand-entered
/// admin data (sometimes a full `t.me/…` URL). All three must collapse to the
/// same handle or the same person appears as two different attendees.
fn normalize_handle(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    let without_scheme = trimmed
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_start_matches("t.me/")
        .trim_start_matches("telegram.me/");
    let handle = without_scheme.trim_start_matches('@').trim();
    if handle.is_empty() {
        return None;
    }
    // Telegram handles are 5..=32 of [A-Za-z0-9_]. Anything else is not a
    // handle, and turning it into a t.me link would produce a dead tap.
    if handle.len() > 32
        || !handle
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        return None;
    }
    Some(handle.to_string())
}

/// Build the display label and link for one attendee.
///
/// Falls back through: username → first name → bare id. The `tg://user?id=`
/// fallback only resolves in clients that already know the user, so it is
/// reported as not reachable-by-handle — the owner should not assume a tap
/// will always open a chat.
pub fn attendee_link(
    username: Option<&str>,
    first_name: Option<&str>,
    telegram_id: i64,
) -> AttendeeLink {
    if let Some(handle) = username.and_then(normalize_handle) {
        return AttendeeLink {
            label: format!("@{handle}"),
            url: format!("https://t.me/{handle}"),
            reachable_by_handle: true,
        };
    }
    let name = first_name
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .map(|n| n.to_string());
    AttendeeLink {
        label: match name {
            Some(n) => format!("{n} · {telegram_id}"),
            None => format!("id {telegram_id}"),
        },
        url: format!("tg://user?id={telegram_id}"),
        reachable_by_handle: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_username_becomes_a_tappable_handle() {
        let a = attendee_link(Some("vibee_dev"), Some("Dmitrii"), 42);
        assert_eq!(a.label, "@vibee_dev");
        assert_eq!(a.url, "https://t.me/vibee_dev");
        assert!(a.reachable_by_handle);
    }

    #[test]
    fn decorations_around_a_handle_are_stripped() {
        // The same person must not appear twice because one source stored the
        // handle with an @ and another as a full link.
        for raw in [
            "@vibee_dev",
            " vibee_dev ",
            "https://t.me/vibee_dev",
            "t.me/vibee_dev",
            "telegram.me/vibee_dev",
        ] {
            assert_eq!(
                attendee_link(Some(raw), None, 42).url,
                "https://t.me/vibee_dev",
                "{raw} should normalise to the same link"
            );
        }
    }

    #[test]
    fn a_missing_username_falls_back_to_the_first_name_and_id() {
        let a = attendee_link(None, Some("Дмитрий"), 8420420131);
        assert_eq!(a.label, "Дмитрий · 8420420131");
        assert_eq!(a.url, "tg://user?id=8420420131");
        assert!(
            !a.reachable_by_handle,
            "an id link does not always open a chat and must not claim to"
        );
    }

    #[test]
    fn with_nothing_but_an_id_the_entry_still_renders() {
        let a = attendee_link(None, None, 7);
        assert_eq!(a.label, "id 7");
        assert_eq!(a.url, "tg://user?id=7");
        assert!(!a.reachable_by_handle);
    }

    #[test]
    fn blank_and_whitespace_values_are_treated_as_absent() {
        for username in [Some(""), Some("   "), Some("@"), None] {
            let a = attendee_link(username, None, 7);
            assert_eq!(
                a.label, "id 7",
                "username {username:?} should be treated as absent"
            );
        }
        assert_eq!(attendee_link(None, Some("  "), 7).label, "id 7");
    }

    #[test]
    fn text_that_cannot_be_a_handle_does_not_become_a_dead_link() {
        // A t.me link built from these would open a "user not found" page,
        // which reads to the owner as "the feature is broken".
        for bad in [
            "not a handle",
            "имя_по-русски",
            "user@example.com",
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", // 35 chars, over the limit
        ] {
            let a = attendee_link(Some(bad), Some("Fallback"), 9);
            assert!(
                !a.reachable_by_handle,
                "{bad:?} must not be turned into a t.me link"
            );
            assert_eq!(a.url, "tg://user?id=9");
        }
    }

    #[test]
    fn a_username_wins_over_a_first_name() {
        // The handle is the thing the owner can actually message.
        let a = attendee_link(Some("handle"), Some("Name"), 1);
        assert_eq!(a.label, "@handle");
    }

    #[test]
    fn labels_are_unique_per_person_even_without_handles() {
        // Two attendees with the same first name must stay distinguishable,
        // otherwise the owner cannot tell which row is which.
        let a = attendee_link(None, Some("Alex"), 1);
        let b = attendee_link(None, Some("Alex"), 2);
        assert_ne!(a.label, b.label);
    }
}
