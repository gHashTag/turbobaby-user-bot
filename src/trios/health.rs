//! Formatting the server-health digest the bot sends to admins.
//!
//! Front-end errors used to be write-only — posted by the app, stored, and
//! readable only with direct database access. During the order outage on
//! 2026-08-10 that meant the one record of what actually broke was
//! unreachable. This turns them into something an owner can read in Telegram.

/// One distinct front-end failure, already grouped by message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErrorGroup {
    pub message: String,
    pub source: String,
    pub url_path: Option<String>,
    pub occurrences: i64,
    pub affected_users: i64,
}

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Trim a message to something readable in a chat bubble.
fn shorten(s: &str, max_chars: usize) -> String {
    let one_line = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if one_line.chars().count() <= max_chars {
        return one_line;
    }
    let cut: String = one_line.chars().take(max_chars).collect();
    format!("{cut}…")
}

/// Render the digest.
///
/// `window_hours` is stated explicitly: "no errors" means nothing over a
/// known period, not "we did not look". Silence with no window is the kind of
/// report that gets misread as healthy.
pub fn format_health_digest(window_hours: i64, groups: &[ErrorGroup]) -> String {
    if groups.is_empty() {
        return format!(
            "✅ <b>Сервер в порядке</b>\nЗа последние {window_hours} ч ошибок в приложении не было."
        );
    }
    let total: i64 = groups.iter().map(|g| g.occurrences).sum();
    let mut out = format!(
        "⚠️ <b>Ошибки в приложении</b>\nЗа {window_hours} ч: {} шт., {} видов\n\n",
        total,
        groups.len()
    );
    for (i, g) in groups.iter().enumerate().take(10) {
        out.push_str(&format!(
            "{}. <b>{}×</b> · {} польз.\n<code>{}</code>\n",
            i + 1,
            g.occurrences,
            g.affected_users,
            escape_html(&shorten(&g.message, 200))
        ));
        if let Some(path) = g.url_path.as_deref().filter(|p| !p.is_empty()) {
            out.push_str(&format!("↳ {}\n", escape_html(&shorten(path, 80))));
        }
        out.push('\n');
    }
    if groups.len() > 10 {
        out.push_str(&format!("…и ещё {} видов\n", groups.len() - 10));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn g(message: &str, occurrences: i64, users: i64) -> ErrorGroup {
        ErrorGroup {
            message: message.to_string(),
            source: "window.onerror".to_string(),
            url_path: Some("/checkout".to_string()),
            occurrences,
            affected_users: users,
        }
    }

    #[test]
    fn a_clean_window_says_so_and_names_the_period() {
        // "No errors" without a stated window reads as "we did not look".
        let msg = format_health_digest(6, &[]);
        assert!(msg.contains("в порядке"), "got: {msg}");
        assert!(msg.contains("6 ч"), "the window must be stated: {msg}");
    }

    #[test]
    fn the_header_totals_occurrences_and_kinds_separately() {
        // One bug hitting 100 users and 100 distinct bugs need different
        // reactions, so the digest must not collapse them into one number.
        let msg = format_health_digest(6, &[g("A", 90, 40), g("B", 10, 3)]);
        assert!(msg.contains("100 шт."), "occurrence total missing: {msg}");
        assert!(msg.contains("2 видов"), "kind count missing: {msg}");
    }

    #[test]
    fn each_group_shows_its_reach() {
        let msg = format_health_digest(6, &[g("boom", 12, 7)]);
        assert!(msg.contains("12×"), "occurrences missing: {msg}");
        assert!(msg.contains("7 польз."), "affected users missing: {msg}");
        assert!(msg.contains("boom"), "message missing: {msg}");
    }

    #[test]
    fn messages_are_escaped() {
        // Telegram rejects malformed HTML outright — an unescaped "<" in a
        // stack trace would mean the digest never arrives at all.
        let msg = format_health_digest(6, &[g("TypeError: <null> & undefined", 1, 1)]);
        assert!(!msg.contains("<null>"), "not escaped: {msg}");
        assert!(msg.contains("&amp;"), "ampersand not escaped: {msg}");
    }

    #[test]
    fn a_long_message_is_trimmed_without_panicking_on_multibyte() {
        let long = "Ошибка ".repeat(200);
        let msg = format_health_digest(6, &[g(&long, 1, 1)]);
        assert!(msg.contains('…'), "expected trimming: {}", &msg[..80]);
    }

    #[test]
    fn only_the_worst_ten_are_listed_and_the_rest_are_counted() {
        // A digest that dumps 200 groups is not read by anyone.
        let groups: Vec<ErrorGroup> = (0..12).map(|i| g(&format!("err{i}"), 1, 1)).collect();
        let msg = format_health_digest(6, &groups);
        assert!(
            msg.contains("err9"),
            "the tenth group should be shown: {msg}"
        );
        assert!(
            !msg.contains("err10"),
            "the eleventh should be summarised: {msg}"
        );
        assert!(
            msg.contains("ещё 2"),
            "the remainder must be counted: {msg}"
        );
    }
}
