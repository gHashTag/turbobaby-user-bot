/// Custom business metrics for Woody Weed Bot.
///
/// Uses the `metrics` crate which is already wired under the hood by
/// `axum-prometheus`.  All counters declared here are automatically
/// included in the `/metrics` Prometheus scrape endpoint.
use metrics::counter;

/// Increment when a new order is successfully persisted in `create_order`.
pub fn order_created() {
    counter!("orders_created_total").increment(1);
}

/// Increment when a quest entity is created.
///
/// `kind` should be `"place"` (quest_places) or `"treasure"` (treasure_hunts).
pub fn quest_created(kind: &str) {
    counter!("quests_created_total", "kind" => kind.to_string()).increment(1);
}

/// Increment when a new user registers (first `/start` with no prior lang record).
pub fn user_registered() {
    counter!("users_registered_total").increment(1);
}

/// Increment when a garden reward is successfully claimed.
pub fn garden_reward_claimed() {
    counter!("garden_rewards_claimed_total").increment(1);
}

/// Increment when a QR code is scanned in the location quest.
///
/// `is_final` indicates whether this was the last location in the quest.
pub fn qr_scanned(is_final: bool) {
    counter!("qr_scans_total", "final" => is_final.to_string()).increment(1);
}

/// Increment when the sliding-window rate limiter rejects a request.
/// `kind` is the throttle bucket (e.g. `"anon_order"`, `"upload"`, `"login"`).
pub fn rate_limit_blocked(kind: &str) {
    counter!("rate_limit_blocked_total", "kind" => kind.to_string()).increment(1);
}

/// Cycle #141: per-kind counter for auth gate rejections in `check_owner`.
///
/// `kind`:
/// - `"owner_mismatch"` — initData is HMAC-valid but `user.id` ≠ path id.
///   A spike here is the signature of someone probing for IDOR.
/// - `"missing_init_data"` — header absent or empty.
/// - `"invalid_init_data"` — HMAC mismatch, replay, or malformed payload.
///
/// Distinguishing the three lets an alert separate "buggy client" from
/// "active attacker" without trawling logs.
pub fn auth_failure(kind: &str) {
    counter!("auth_failures_total", "kind" => kind.to_string()).increment(1);
}

// Cycle #108: `db_pool_acquire_failed(scope: &str)` was declared here
// to report `pool.get()` failures from the deadpool_postgres path.
// Cycle #96 dropped that pool when the SeaORM migration finished, so
// there was nothing left to call it — only the test kept the helper
// alive. Removed in cycle #108 along with the test.
//
// If a future replacement pool needs the same observability, restore
// the helper *and* wire at least one real call site in the same PR —
// the audit test below now fails on declared-but-unwired metrics.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_order_created_does_not_panic() {
        order_created();
    }

    #[test]
    fn test_quest_created_does_not_panic() {
        quest_created("place");
    }

    #[test]
    fn test_user_registered_does_not_panic() {
        user_registered();
    }

    #[test]
    fn test_garden_reward_claimed_does_not_panic() {
        garden_reward_claimed();
    }

    #[test]
    fn test_qr_scanned_does_not_panic() {
        qr_scanned(true);
        qr_scanned(false);
    }

    #[test]
    fn test_rate_limit_blocked_does_not_panic() {
        rate_limit_blocked("anon_order");
        rate_limit_blocked("upload");
    }

    #[test]
    fn test_auth_failure_does_not_panic() {
        auth_failure("owner_mismatch");
        auth_failure("missing_init_data");
        auth_failure("invalid_init_data");
    }
}

/// Cycle #108: defensive test against declared-but-unwired metric
/// helpers. Cycles #105 (anon_order) and #106 (admin_auth) found two
/// `rate_limit_blocked` labels declared but never incremented because
/// somebody added the helper, planned to wire it, then forgot. Cycle
/// #107 made the same discovery a third time with `db_pool_acquire_failed`
/// (which became orphaned when the SeaORM migration deleted the pool).
///
/// This test walks `src/metrics.rs` to find every `pub fn <name>`,
/// then greps `src/**/*.rs` (excluding `src/metrics.rs` itself) for a
/// `metrics::<name>(` or `crate::metrics::<name>(` reference. **Sound
/// only because no file in this crate does `use crate::metrics::<name>`
/// — every call site uses the fully-qualified path.** Verified at cycle
/// #109 audit time. If that convention changes, broaden the match. Any
/// helper with zero call sites fails the test — forcing the
/// add-helper-and-wire-it dance to land in a single commit.
///
/// Allowlist mechanism mirrors `db::orphan_table_tests` from cycle
/// #103: helpers known to be intentionally unused live in
/// `ALLOWED_UNUSED_METRICS` with an inline rationale.
#[cfg(test)]
mod metric_wiring_tests {
    /// Metric helpers that exist for forward-compatibility but are
    /// not yet wired. Each entry needs a rationale comment.
    const ALLOWED_UNUSED_METRICS: &[&str] = &[
        // (empty — every declared helper is wired as of cycle #108)
    ];

    fn extract_pub_fn_names(source: &str) -> Vec<String> {
        let mut out = Vec::new();
        for line in source.lines() {
            let t = line.trim();
            if let Some(rest) = t.strip_prefix("pub fn ") {
                if let Some(paren) = rest.find('(') {
                    let name = &rest[..paren];
                    if name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
                        && !name.is_empty()
                    {
                        out.push(name.to_string());
                    }
                }
            }
        }
        out
    }

    fn code_corpus_excluding_metrics() -> String {
        let manifest = env!("CARGO_MANIFEST_DIR");
        let src = std::path::Path::new(manifest).join("src");
        let metrics_path = std::path::Path::new(manifest).join("src/metrics.rs");
        let mut buf = String::new();
        fn walk(p: &std::path::Path, skip: &std::path::Path, buf: &mut String) {
            for entry in std::fs::read_dir(p)
                .expect("readable")
                .filter_map(|e| e.ok())
            {
                let path = entry.path();
                if path.is_dir() {
                    walk(&path, skip, buf);
                } else if path == skip {
                    continue;
                } else if path
                    .extension()
                    .and_then(|s| s.to_str())
                    .is_some_and(|s| s == "rs")
                {
                    if let Ok(s) = std::fs::read_to_string(&path) {
                        buf.push_str(&s);
                        buf.push('\n');
                    }
                }
            }
        }
        walk(&src, &metrics_path, &mut buf);
        buf
    }

    #[test]
    fn every_metric_helper_is_wired() {
        let manifest = env!("CARGO_MANIFEST_DIR");
        let metrics_src =
            std::fs::read_to_string(std::path::Path::new(manifest).join("src/metrics.rs"))
                .expect("read src/metrics.rs");
        let helpers = extract_pub_fn_names(&metrics_src);
        assert!(
            !helpers.is_empty(),
            "no pub fn extracted from src/metrics.rs — parser broken?"
        );
        let corpus = code_corpus_excluding_metrics();

        let mut unused = Vec::new();
        for name in &helpers {
            if ALLOWED_UNUSED_METRICS.iter().any(|a| *a == name.as_str()) {
                continue;
            }
            let needle_short = format!("metrics::{}(", name);
            let needle_long = format!("crate::metrics::{}(", name);
            if !corpus.contains(&needle_short) && !corpus.contains(&needle_long) {
                unused.push(name.clone());
            }
        }
        assert!(
            unused.is_empty(),
            "metric helpers declared in src/metrics.rs with zero call sites ({}): {:?}\n\
             Either wire each helper or add to ALLOWED_UNUSED_METRICS with rationale.",
            unused.len(),
            unused
        );
    }
}
