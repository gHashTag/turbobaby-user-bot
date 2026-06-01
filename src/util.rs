/// Escape HTML special characters for safe rendering in Telegram HTML parse mode.
pub fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Truncate a string to a maximum number of Unicode scalar values.
pub fn truncate_string(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        s.chars().take(max_len).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::{html_escape, truncate_string};

    #[test]
    fn test_html_escape_basic() {
        assert_eq!(
            html_escape("<script>alert(1)</script>"),
            "&lt;script&gt;alert(1)&lt;/script&gt;"
        );
    }

    #[test]
    fn test_html_escape_ampersand() {
        assert_eq!(html_escape("A & B"), "A &amp; B");
    }

    #[test]
    fn test_html_escape_mixed() {
        assert_eq!(
            html_escape("<div class=\"foo\">A & B"),
            "&lt;div class=\"foo\"&gt;A &amp; B"
        );
    }

    #[test]
    fn test_html_escape_empty() {
        assert_eq!(html_escape(""), "");
    }

    #[test]
    fn test_html_escape_no_special() {
        assert_eq!(html_escape("hello world"), "hello world");
    }

    #[test]
    fn test_truncate_string_shorter() {
        assert_eq!(truncate_string("hello", 10), "hello");
    }

    #[test]
    fn test_truncate_string_exact_bytes() {
        let s = "a".repeat(50);
        assert_eq!(truncate_string(&s, 50), s);
    }

    #[test]
    fn test_truncate_string_longer() {
        let s = "a".repeat(100);
        assert_eq!(truncate_string(&s, 50).len(), 50);
    }

    #[test]
    fn test_truncate_string_unicode() {
        let s = "🔥".repeat(100); // 4 bytes each, 100 chars
        assert_eq!(truncate_string(&s, 50).chars().count(), 50);
        assert_eq!(truncate_string(&s, 50).len(), 50 * 4);
    }

    #[test]
    fn test_truncate_string_empty() {
        assert_eq!(truncate_string("", 10), "");
    }

    #[test]
    fn test_truncate_string_does_not_panic_at_codepoint_boundary() {
        // Regression guard for cycle #75. `String::truncate(N)` (which we
        // are *not* using here) panics when N falls mid-codepoint. We
        // pick max_len values that would land inside a UTF-8 sequence
        // for the given input — emoji are 4 bytes, cyrillic are 2.
        // truncate_string counts chars, so these must all succeed.
        let emoji = "🔥".repeat(20); // 80 bytes
        for n in [1, 3, 5, 7, 11, 15, 19, 20, 21] {
            // would panic at every non-multiple-of-4 byte index
            let _ = truncate_string(&emoji, n);
        }
        let cyrillic = "ё".repeat(20); // 40 bytes
        for n in [1, 3, 5, 7, 9, 11, 13, 19, 20, 21] {
            let _ = truncate_string(&cyrillic, n);
        }
    }
}
