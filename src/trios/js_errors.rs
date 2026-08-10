//! Deciding which browser errors are worth showing the customer.
//!
//! This matters more than it looks. The error panel is `position:fixed` across
//! the top half of the screen — while it is open it swallows every tap
//! underneath, including the button that places an order. Orders broke twice
//! because noise opened it: first aborted fetches, then an opaque `onerror`
//! with no message at all. So the question "is this error real?" is directly a
//! question about whether the shop works.
//!
//! Lives here rather than next to the overlay because `src/ui` is
//! `#[cfg(target_arch = "wasm32")]`-gated and tests written there never run.

/// Benign, non-actionable errors that must not raise anything.
///
/// `AbortError` / "operation was aborted" / "play() request was interrupted"
/// come from autoplay `<video>` previews and in-flight fetches: when a card
/// re-renders or the user navigates, the browser cancels the request. That is
/// expected, not a fault. `ResizeObserver loop` is the classic harmless
/// browser warning.
///
/// Real fetch failures surface as TypeError/NetworkError and still show.
pub fn is_benign_js_error(msg: &str) -> bool {
    msg.contains("AbortError")
        || msg.contains("operation was aborted")
        || msg.contains("play() request was interrupted")
        || msg.contains("request is not allowed by the user agent")
        || msg.contains("ResizeObserver loop")
}

/// An `onerror` report that carries no message, no file and no position.
///
/// The browser emits this shape for things it will not describe: a failed
/// resource load (a broken `<img>`/`<video>` on the page) and any error thrown
/// by a cross-origin script, which is opaque for security reasons. There is
/// nothing in it to act on.
///
/// Deliberately narrow — it requires *all* of message, filename and position
/// to be absent. A real error that merely lacks a line number still gets
/// through.
pub fn is_opaque_browser_error(msg: &str, filename: &str, lineno: u32, colno: u32) -> bool {
    (msg.is_empty() || msg == "Unknown error" || msg == "Script error.")
        && filename.is_empty()
        && lineno == 0
        && colno == 0
}

/// Should this `window.onerror` report reach the customer-visible panel?
pub fn onerror_is_reportable(msg: &str, filename: &str, lineno: u32, colno: u32) -> bool {
    !is_benign_js_error(msg) && !is_opaque_browser_error(msg, filename, lineno, colno)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_opaque_report_that_blocked_checkout_is_not_reportable() {
        // Verbatim shape of the 13 errors logged against /cart on 2026-08-10:
        // no message, no file, position 0:0. Opening the panel for these made
        // the order button untappable.
        assert!(!onerror_is_reportable("Unknown error", "", 0, 0));
        assert!(!onerror_is_reportable("", "", 0, 0));
        assert!(!onerror_is_reportable("Script error.", "", 0, 0));
    }

    #[test]
    fn a_real_error_with_a_location_is_reportable() {
        assert!(onerror_is_reportable(
            "TypeError: x is not a function",
            "https://app/main.js",
            42,
            7
        ));
    }

    #[test]
    fn the_filter_is_narrow_enough_not_to_swallow_real_failures() {
        // Each of these differs from the opaque shape in exactly one way, and
        // every one of them must still be reported — a filter that also ate
        // these would hide genuine breakage.
        assert!(
            onerror_is_reportable("Unknown error", "https://app/main.js", 0, 0),
            "a named file means the browser did describe it"
        );
        assert!(
            onerror_is_reportable("Unknown error", "", 12, 0),
            "a line number means there is something to look at"
        );
        assert!(
            onerror_is_reportable("Unknown error", "", 0, 5),
            "a column number likewise"
        );
        assert!(
            onerror_is_reportable("PANIC: index out of bounds", "", 0, 0),
            "a message we can act on must never be filtered"
        );
    }

    #[test]
    fn cancellations_are_not_reportable() {
        for msg in [
            "AbortError: The operation was aborted.",
            "Unhandled: AbortError",
            "The play() request was interrupted",
            "ResizeObserver loop completed with undelivered notifications.",
        ] {
            assert!(
                !onerror_is_reportable(msg, "https://app/main.js", 10, 1),
                "{msg} should be treated as noise"
            );
        }
    }

    #[test]
    fn a_panic_is_always_reportable() {
        // The one class that must never be filtered, whatever its location.
        assert!(onerror_is_reportable("PANIC: boom", "", 0, 1));
        assert!(onerror_is_reportable(
            "RuntimeError: unreachable",
            "f.js",
            1,
            1
        ));
    }
}
