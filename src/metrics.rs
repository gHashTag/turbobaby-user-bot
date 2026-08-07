/// Custom business metrics for Woody Weed Bot.
///
/// Uses the `metrics` crate which is already wired under the hood by
/// `axum-prometheus`.  All counters declared here are automatically
/// included in the `/metrics` Prometheus scrape endpoint.
use metrics::{counter, gauge};

/// Increment when a new order is successfully persisted in `create_order`.
pub fn order_created() {
    counter!("orders_created_total").increment(1);
}

/// Cycle #9: customer self-service cancellation completed successfully.
pub fn order_cancelled_by_user() {
    counter!("orders_cancelled_by_user_total").increment(1);
}

/// Cycle #9: admin/status worker moved an order to a new milestone. Labelled
/// by terminal status so Grafana can split preparing/ready/delivered/cancelled.
pub fn order_status_changed(status: &str) {
    counter!(
        "orders_status_changed_total",
        "status" => status.to_string()
    )
    .increment(1);
}

/// Cycle #9: user clicked "Track Order" on the success screen.
pub fn order_tracked() {
    counter!("orders_tracked_total").increment(1);
}

/// Cycle #9: user clicked the share/referral prompt from the success screen.
pub fn referral_prompt_clicked(source: &str) {
    counter!(
        "referral_prompt_clicked_total",
        "source" => source.to_string()
    )
    .increment(1);
}

/// Set once at startup to the number of expected catalog columns missing from
/// the live DB (see `Database::missing_critical_columns`). Non-zero means prod
/// is behind on migrations and catalog endpoints (e.g. `/api/sets`) will
/// degrade/500. A **gauge** (not counter) so it reflects current state and
/// clears to 0 once migrations are applied and the service redeploys. Alert on
/// `schema_missing_columns > 0` — the same Prometheus path as the 5xx alerts.
pub fn schema_missing_columns(n: u64) {
    gauge!("schema_missing_columns").set(n as f64);
}

/// Set once at startup per optional capability (e.g. `"ai"`, `"s3"`) to 1
/// (enabled) or 0 (disabled by missing config). A labelled **gauge** so a
/// Grafana panel can show which features are live across environments and
/// replicas over time — the dashboard counterpart to the startup warning in
/// `Config::disabled_capabilities`. Info signal, not an alert (a capability
/// may be intentionally off in some environments).
pub fn capability_enabled(name: &str, enabled: bool) {
    gauge!("capability_enabled", "capability" => name.to_string()).set(if enabled {
        1.0
    } else {
        0.0
    });
}

/// Classic Prometheus **info-metric**: `build_info{version,build} 1`, set once
/// at startup. The value is always 1; the deploy identity lives in the labels.
/// Lets a dashboard/alert answer "which commit is prod running?" and join
/// other metrics to a deploy via PromQL — e.g. to confirm the /api/sets fix
/// actually shipped. Low cardinality (one series per deploy).
pub fn build_info(version: &str, build: &str) {
    gauge!("build_info", "version" => version.to_string(), "build" => build.to_string()).set(1.0);
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

/// Loop #17: a garden water/harvest/expiry reminder was sent by the server sweep.
pub fn garden_reminder_sent(kind: &str) {
    counter!(
        "garden_reminder_sent_total",
        "kind" => kind.to_string()
    )
    .increment(1);
}

/// Loop #17: a garden streak was broken (user missed >48h between waters).
pub fn garden_streak_broken() {
    counter!("garden_streak_broken_total").increment(1);
}

/// Loop #17: a streak milestone was reached (3, 7, 14, 30, …). The streak
/// value is emitted as a label so dashboards can track milestone distribution.
pub fn garden_streak_milestone(streak: i64) {
    counter!(
        "garden_streak_milestone_total",
        "streak" => streak.to_string()
    )
    .increment(1);
}

/// Loop #17: a garden reward expired unused.
pub fn garden_reward_expired() {
    counter!("garden_rewards_expired_total").increment(1);
}

/// Loop #17: a reward expiry FOMO nudge was sent (24h or 4h before expiry).
pub fn garden_reward_expiry_nudge_sent(hours_before: i64) {
    counter!(
        "garden_reward_expiry_nudge_sent_total",
        "hours" => hours_before.to_string()
    )
    .increment(1);
}

/// Loop #17: customer opened the garden screen.
pub fn garden_screen_opened(source: &str) {
    counter!(
        "garden_screen_opened_total",
        "source" => source.to_string()
    )
    .increment(1);
}

/// Loop #17: customer tapped the water CTA in the garden UI.
pub fn garden_water_tapped() {
    counter!("garden_water_tapped_total").increment(1);
}

/// Loop #17: customer tapped the harvest CTA in the garden UI.
pub fn garden_harvest_tapped() {
    counter!("garden_harvest_tapped_total").increment(1);
}

/// Loop #17: customer tapped the choose-product CTA to start a new plant.
pub fn garden_choose_product_tapped() {
    counter!("garden_choose_product_tapped_total").increment(1);
}

/// Loop #17: customer reset their garden plant.
pub fn garden_reset_tapped() {
    counter!("garden_reset_tapped_total").increment(1);
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

/// Cycle #141: per-kind counter for auth gate rejections.
/// Cycle #142: extended to cover the admin gate and block list.
///
/// `kind`:
/// - `"owner_mismatch"` — initData HMAC-valid but `user.id` ≠ path id.
///   A spike here is the signature of someone probing for IDOR.
/// - `"missing_init_data"` — `check_owner` header absent or empty.
/// - `"invalid_init_data"` — `check_owner` HMAC mismatch, replay, or
///   malformed payload.
/// - `"admin_no_valid_auth"` — `check_admin` exhausted both initData
///   and X-Admin-Token paths. Combined with `rate_limit_blocked{kind="admin_auth"}`
///   this distinguishes brute-force throttled vs. just-misconfigured.
/// - `"user_blocked"` — `check_not_blocked` rejected a known-bad
///   actor coming back post-block. A non-trivial rate here suggests
///   the block list is doing real work.
///
/// Distinguishing the kinds lets an alert separate "buggy client" from
/// "active attacker" from "background block-list noise" without
/// trawling logs.
pub fn auth_failure(kind: &str) {
    counter!("auth_failures_total", "kind" => kind.to_string()).increment(1);
}

/// Cycle #170: event-booking lifecycle metrics.
pub fn event_booking_created(status: &str) {
    counter!("event_bookings_created_total", "status" => status.to_string()).increment(1);
}

pub fn event_booking_cancelled() {
    counter!("event_bookings_cancelled_total").increment(1);
}

pub fn event_waitlist_promoted() {
    counter!("event_waitlist_promoted_total").increment(1);
}

/// Loop #12/#13: abandoned-cart reminder sent to a customer. Track volume and
/// A/B variant so we can correlate sends with recovered orders.
pub fn cart_abandonment_reminder_sent(variant: &str) {
    counter!(
        "cart_abandonment_reminder_sent_total",
        "variant" => variant.to_string()
    )
    .increment(1);
}

/// Loop #13: second nudge (24h) sent.
pub fn cart_abandonment_second_nudge_sent(variant: &str) {
    counter!(
        "cart_abandonment_second_nudge_sent_total",
        "variant" => variant.to_string()
    )
    .increment(1);
}

/// Loop #12/#16: customer tapped the one-tap reorder CTA on an order card/detail.
pub fn reorder_clicked(source: &str) {
    counter!("reorder_clicked_total", "source" => source.to_string()).increment(1);
}

/// Loop #16: garden reminder CTA (water/harvest) was shown and tapped on Home.
pub fn garden_reminder_clicked(kind: &str) {
    counter!("garden_reminder_clicked_total", "kind" => kind.to_string()).increment(1);
}

/// Loop #12: customer opened the Mini App via a cart deep-link reminder.
pub fn cart_deep_link_opened(variant: &str) {
    counter!(
        "cart_deep_link_opened_total",
        "variant" => variant.to_string()
    )
    .increment(1);
}

/// Loop #15: funnel instrumentation for the checkout flow.
pub fn checkout_started() {
    counter!("checkout_started_total").increment(1);
}

pub fn checkout_completed() {
    counter!("checkout_completed_total").increment(1);
}

pub fn checkout_error(reason: &str) {
    counter!("checkout_error_total", "reason" => reason.to_string()).increment(1);
}

pub fn bonus_applied(amount: f64) {
    counter!("bonus_applied_total").increment(1);
    gauge!("bonus_applied_amount").set(amount);
}

pub fn stars_applied(amount: i64) {
    counter!("stars_applied_total").increment(1);
    gauge!("stars_applied_amount").set(amount as f64);
}

pub fn garden_reward_applied(discount: f64) {
    counter!("garden_reward_applied_total").increment(1);
    gauge!("garden_reward_discount").set(discount);
}

pub fn event_shared(kind: &str) {
    counter!("events_shared_total", "kind" => kind.to_string()).increment(1);
}

pub fn event_detail_opened(source: &str) {
    counter!("event_details_opened_total", "source" => source.to_string()).increment(1);
}

pub fn event_booking_attempted(cta_variant: &str) {
    counter!(
        "event_booking_attempted_total",
        "cta_variant" => cta_variant.to_string()
    )
    .increment(1);
}

pub fn event_booking_succeeded(cta_variant: &str) {
    counter!(
        "event_booking_succeeded_total",
        "cta_variant" => cta_variant.to_string()
    )
    .increment(1);
}

pub fn event_booking_failed(reason: &str) {
    counter!("event_booking_failed_total", "reason" => reason.to_string()).increment(1);
}

/// Cycle #173 (C3): frontend error telemetry received.
/// `source` is one of `"wasm"`, `"android"`, `"ios"`, `"unknown"`.
pub fn client_error_received(source: &str) {
    counter!("client_errors_received_total", "source" => source.to_string()).increment(1);
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
    fn test_event_metrics_do_not_panic() {
        event_booking_created("confirmed");
        event_booking_created("waitlisted");
        event_booking_cancelled();
        event_waitlist_promoted();
        event_shared("event");
        event_detail_opened("share");
        event_booking_attempted("A");
        event_booking_succeeded("A");
        event_booking_failed("sold_out");
    }

    #[test]
    fn test_client_error_received_does_not_panic() {
        client_error_received("wasm");
        client_error_received("unknown");
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
        // (empty — every declared helper is wired as of cycle #12B)
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

    /// Strip `//` line comments so a sample like `metrics::foo(` in
    /// a doc-comment doesn't fool the textual `contains` check below
    /// into thinking the helper is wired. Mirrors the same defense in
    /// `src/db/mod.rs::orphan_table_tests` and
    /// `src/api/mod.rs::route_wiring_tests` (cycle #145).
    fn strip_line_comments(src: &str) -> String {
        let mut out = String::with_capacity(src.len());
        for line in src.lines() {
            let cut = match line.find("//") {
                Some(i) => &line[..i],
                None => line,
            };
            out.push_str(cut);
            out.push('\n');
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
                        buf.push_str(&strip_line_comments(&s));
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
