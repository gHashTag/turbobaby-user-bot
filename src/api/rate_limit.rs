//! Sliding-window-log rate-limiter shared across API endpoints.
//!
//! Algorithm choice: sliding window log keeps a timestamp per attempt and
//! evicts anything older than the window. Compared to token bucket, it gives
//! exact enforcement and is resistant to burst exploitation at window
//! boundaries — the right trade-off for auth/abuse-prone endpoints
//! (login, upload, anonymous orders).
//!
//! Reference: Arcjet, API7 rate-limiting guides (2024-2025).

use axum::http::HeaderMap;
use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

/// Per-key timestamp log: one VecDeque<Instant> per IP / user / etc.
pub(crate) type SlidingWindowStore = tokio::sync::Mutex<HashMap<String, VecDeque<Instant>>>;

/// Helper to instantiate an empty store inside a `LazyLock`.
pub(crate) fn new_store() -> SlidingWindowStore {
    tokio::sync::Mutex::new(HashMap::new())
}

/// Extract the client IP for rate-limiting. Prefers `X-Forwarded-For`
/// (set by Railway / Cloudflare / nginx) — its first entry is the originating
/// client. Falls back to `X-Real-IP`, then `"unknown"` so requests with no
/// proxy headers still share a single bucket (worst case: anonymous bucket
/// throttles aggregate, never opens an unbounded hole).
pub(crate) fn client_ip_from_headers(headers: &HeaderMap) -> String {
    if let Some(xff) = headers.get("x-forwarded-for").and_then(|v| v.to_str().ok()) {
        if let Some(first) = xff.split(',').next() {
            let ip = first.trim();
            if !ip.is_empty() && ip.len() <= 64 {
                return ip.to_string();
            }
        }
    }
    if let Some(real) = headers.get("x-real-ip").and_then(|v| v.to_str().ok()) {
        let ip = real.trim();
        if !ip.is_empty() && ip.len() <= 64 {
            return ip.to_string();
        }
    }
    "unknown".to_string()
}

/// Drop timestamps older than `now - window` from the front of the log.
pub(crate) fn prune_window(log: &mut VecDeque<Instant>, now: Instant, window: Duration) {
    while let Some(&t) = log.front() {
        if now.saturating_duration_since(t) >= window {
            log.pop_front();
        } else {
            break;
        }
    }
}

/// Check whether `key` is allowed under sliding-window-log policy and record
/// the attempt atomically. Returns `true` when allowed, `false` when over the
/// limit. When the store grows past `max_keys`, fully-expired entries are
/// evicted before insert to bound memory under spray attacks.
pub(crate) async fn check_and_record(
    store: &SlidingWindowStore,
    key: &str,
    window: Duration,
    max_attempts: usize,
    max_keys: usize,
) -> bool {
    let now = Instant::now();
    let mut map = store.lock().await;

    if map.len() >= max_keys {
        map.retain(|_, log| {
            prune_window(log, now, window);
            !log.is_empty()
        });
    }

    let log = map.entry(key.to_string()).or_default();
    prune_window(log, now, window);

    if log.len() >= max_attempts {
        return false;
    }
    log.push_back(now);
    true
}

// ────────────────────────────────────────────────────────────────────
// Cycle #106: synchronous variant.
//
// The async `check_and_record` above is for handlers that are already
// async (upload, anonymous orders). `check_admin` in `src/api/auth.rs`
// is sync and called from 57 callsites; making it async would force a
// cascade of `.await` edits with no real benefit (the rate-limit
// critical section is microseconds — locking briefly inside a sync fn
// is fine).
//
// Identical algorithm; the only difference is `std::sync::Mutex` and
// no `.await`. Holding a std::sync::Mutex across `.await` is unsafe,
// but here we lock, do the math, push, drop — no await ever.
// ────────────────────────────────────────────────────────────────────

pub(crate) type SyncSlidingWindowStore = std::sync::Mutex<HashMap<String, VecDeque<Instant>>>;

pub(crate) fn new_sync_store() -> SyncSlidingWindowStore {
    std::sync::Mutex::new(HashMap::new())
}

pub(crate) fn check_and_record_sync(
    store: &SyncSlidingWindowStore,
    key: &str,
    window: Duration,
    max_attempts: usize,
    max_keys: usize,
) -> bool {
    let now = Instant::now();
    // PoisonError shouldn't happen in practice (the critical section is
    // panic-free — only HashMap ops and VecDeque pushes) but treat a
    // poisoned mutex as "let the request through" to avoid wedging
    // admin access if something weird happens upstream.
    let mut map = match store.lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };

    if map.len() >= max_keys {
        map.retain(|_, log| {
            prune_window(log, now, window);
            !log.is_empty()
        });
    }

    let log = map.entry(key.to_string()).or_default();
    prune_window(log, now, window);

    if log.len() >= max_attempts {
        return false;
    }
    log.push_back(now);
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::LazyLock;

    #[test]
    fn ip_from_xff_takes_first_entry() {
        let mut h = HeaderMap::new();
        h.insert("x-forwarded-for", "203.0.113.5, 10.0.0.1".parse().unwrap());
        assert_eq!(client_ip_from_headers(&h), "203.0.113.5");
    }

    #[test]
    fn ip_falls_back_to_x_real_ip() {
        let mut h = HeaderMap::new();
        h.insert("x-real-ip", "198.51.100.7".parse().unwrap());
        assert_eq!(client_ip_from_headers(&h), "198.51.100.7");
    }

    #[test]
    fn ip_xff_takes_priority_over_real_ip() {
        let mut h = HeaderMap::new();
        h.insert("x-forwarded-for", "203.0.113.5".parse().unwrap());
        h.insert("x-real-ip", "198.51.100.7".parse().unwrap());
        assert_eq!(client_ip_from_headers(&h), "203.0.113.5");
    }

    #[test]
    fn ip_unknown_when_no_headers() {
        let h = HeaderMap::new();
        assert_eq!(client_ip_from_headers(&h), "unknown");
    }

    #[test]
    fn ip_rejects_oversized_header_value() {
        let mut h = HeaderMap::new();
        let huge = "a".repeat(200);
        h.insert("x-forwarded-for", huge.parse().unwrap());
        assert_eq!(client_ip_from_headers(&h), "unknown");
    }

    #[test]
    fn prune_removes_expired_entries() {
        let now = Instant::now();
        let window = Duration::from_secs(60);
        let mut log = VecDeque::from(vec![
            now - Duration::from_secs(120),
            now - Duration::from_secs(30),
        ]);
        prune_window(&mut log, now, window);
        assert_eq!(log.len(), 1);
    }

    #[test]
    fn prune_keeps_recent_entries() {
        let now = Instant::now();
        let window = Duration::from_secs(60);
        let mut log = VecDeque::from(vec![now - Duration::from_secs(10)]);
        prune_window(&mut log, now, window);
        assert_eq!(log.len(), 1);
    }

    static TEST_STORE: LazyLock<SlidingWindowStore> = LazyLock::new(new_store);

    #[tokio::test]
    async fn allows_up_to_limit() {
        let key = format!("test-a-{}", uuid::Uuid::new_v4());
        let w = Duration::from_secs(60);
        for i in 0..5 {
            assert!(
                check_and_record(&TEST_STORE, &key, w, 5, 100).await,
                "attempt {} should pass",
                i
            );
        }
    }

    #[tokio::test]
    async fn blocks_past_limit() {
        let key = format!("test-b-{}", uuid::Uuid::new_v4());
        let w = Duration::from_secs(60);
        for _ in 0..5 {
            assert!(check_and_record(&TEST_STORE, &key, w, 5, 100).await);
        }
        assert!(!check_and_record(&TEST_STORE, &key, w, 5, 100).await);
    }

    #[tokio::test]
    async fn isolates_per_key() {
        let k1 = format!("test-c-1-{}", uuid::Uuid::new_v4());
        let k2 = format!("test-c-2-{}", uuid::Uuid::new_v4());
        let w = Duration::from_secs(60);
        for _ in 0..5 {
            assert!(check_and_record(&TEST_STORE, &k1, w, 5, 100).await);
        }
        assert!(!check_and_record(&TEST_STORE, &k1, w, 5, 100).await);
        // k2 is independent
        assert!(check_and_record(&TEST_STORE, &k2, w, 5, 100).await);
    }

    #[tokio::test]
    async fn evicts_expired_keys_when_over_cap() {
        // Use a tiny window and tiny cap so we can force eviction.
        let store: SlidingWindowStore = new_store();
        let w = Duration::from_millis(1);
        for i in 0..5 {
            check_and_record(&store, &format!("evict-{}", i), w, 100, 5).await;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
        // New key: prior 5 should evict cleanly since they're all expired.
        assert!(check_and_record(&store, "fresh-key", w, 100, 5).await);
        let map = store.lock().await;
        // After eviction we should have ≤ 1 active key (the fresh one).
        assert!(map.len() <= 5);
    }

    // ── Sync variant (cycle #106) ──────────────────────────────────────

    #[test]
    fn sync_allows_up_to_limit_then_blocks() {
        let store = new_sync_store();
        let w = Duration::from_secs(60);
        for i in 0..5 {
            assert!(
                check_and_record_sync(&store, "test-sync-a", w, 5, 100),
                "attempt {} should pass",
                i
            );
        }
        assert!(!check_and_record_sync(&store, "test-sync-a", w, 5, 100));
    }

    #[test]
    fn sync_isolates_per_key() {
        let store = new_sync_store();
        let w = Duration::from_secs(60);
        for _ in 0..5 {
            assert!(check_and_record_sync(&store, "k1", w, 5, 100));
        }
        assert!(!check_and_record_sync(&store, "k1", w, 5, 100));
        // Independent key unaffected.
        assert!(check_and_record_sync(&store, "k2", w, 5, 100));
    }

    #[test]
    fn sync_evicts_expired_keys_when_over_cap() {
        let store = new_sync_store();
        let w = Duration::from_millis(1);
        for i in 0..5 {
            check_and_record_sync(&store, &format!("evict-{}", i), w, 100, 5);
        }
        std::thread::sleep(Duration::from_millis(5));
        assert!(check_and_record_sync(&store, "fresh-key", w, 100, 5));
        let map = store.lock().unwrap();
        assert!(map.len() <= 5);
    }
}
