use axum::http::HeaderValue;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Simple in-memory ETag cache for API responses.
///
/// Uses SHA-256 — the whole 32-byte digest, 64 hex chars — so the tag is
/// stable across process restarts (`DefaultHasher` is NOT stable across Rust
/// releases) and so two different bodies cannot share a tag by accident. The
/// digest was truncated to 16 hex chars until 2026-09-21; `compute_hash` says
/// what that cost.
///
/// What it stores is a DIGEST and never a body: the map is key -> tag, and
/// `has_changed` re-hashes the body it is handed on every call, so a served
/// body is always the one just built. What it does NOT have is a bound —
/// no capacity, no expiry, no eviction — and `set` says what is done about
/// that instead of inventing one.
#[derive(Clone)]
pub struct ETagCache {
    hashes: Arc<RwLock<HashMap<String, String>>>,
}

impl ETagCache {
    pub fn new() -> Self {
        Self {
            hashes: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Compute stable hash of JSON response: the whole SHA-256 digest, as 64
    /// lowercase hex characters.
    ///
    /// 2026-09-21: this kept `hash[0..8]` and rendered sixteen hex characters,
    /// throwing away 24 of the 32 bytes. Truncation was the one hole in the
    /// origin's guarantee. `has_changed` re-hashes the current body on every
    /// call, so a stale answer cannot come from a missed invalidation — it can
    /// only come from two different bodies hashing alike, and at 64 bits that
    /// is a birthday collision around 2^32 distinct bodies rather than a
    /// cryptographic impossibility. The client is then told 304 about a body it
    /// has never received, and the screen renders yesterday's catalog with no
    /// error anywhere. Keeping the digest costs 48 bytes per entry and one
    /// `format!` of the same shape, so nothing was bought by the truncation.
    ///
    /// `hex::encode` rather than a hand-rolled loop: it is what
    /// `src/api/auth.rs:242` and nine other sites in this crate already use to
    /// render a digest, and one implementation of a shared conversion is D15.
    pub fn compute_hash(data: &str) -> String {
        hex::encode(Sha256::digest(data.as_bytes()))
    }

    /// Get cached hash for a key
    pub async fn get(&self, key: &str) -> Option<String> {
        self.hashes.read().await.get(key).cloned()
    }

    /// How many distinct keys the map is holding.
    ///
    /// The one number nobody could read before 2026-09-21. It is a
    /// measurement and not a limit: see `set` for why no limit is stated.
    ///
    /// Named `entry_count` and not `len` on purpose. `len` invites
    /// `is_empty`, and clippy's `len_without_is_empty` asks for one; an empty
    /// map is not a question anybody has about this type, and the pair would
    /// dress a measurement up as a container API.
    pub async fn entry_count(&self) -> usize {
        self.hashes.read().await.len()
    }

    /// Set hash for a key, and say so when the map crosses a decade.
    ///
    /// THE GROWTH THIS ANNOUNCES. The map has no capacity, no expiry and no
    /// eviction — an entry's only lifetime is the process (`invalidate_*`
    /// below removes five key names, none of which a live writer ever
    /// creates). Meanwhile the key is caller-shaped: `src/api/bikes.rs:489-499`
    /// folds `BikeFilter::cache_suffix` into it, and that suffix carries the
    /// class plus BOTH displacement bounds (`src/api/bikes.rs:381-395`), whose
    /// parser rejects only a non-integer, a negative value and an inverted
    /// band (`src/api/bikes.rs:309-337`). Every remaining pair of non-negative
    /// integers is a distinct key, so a caller — not the fleet — decides how
    /// many entries this process ends up holding.
    ///
    /// WHY A LOG AND NOT A CAP. No entry ceiling for this map is published
    /// anywhere in this repository: `specs/turbobaby/http_cache.t27` records
    /// the absence with a sentinel rather than a plausible figure, and the
    /// nearest bound on how often a caller may ask lives in
    /// `specs/turbobaby/rate_limit.t27`, which bounds attempts and not
    /// distinct keys. Picking a capacity here would publish a policy the owner
    /// never set, and evicting on it would make a cold key serve a full body
    /// for reasons no reader could reconstruct. So the growth is made visible
    /// instead of guessed at: nothing refuses, nothing is evicted, and the
    /// count appears in the log at each decade. A decade is a reporting
    /// cadence, not a threshold — there is no behaviour on either side of it.
    pub async fn set(&self, key: &str, data: &str) -> String {
        let hash = Self::compute_hash(data);
        let entries = {
            let mut map = self.hashes.write().await;
            let is_new_key = map.insert(key.to_string(), hash.clone()).is_none();
            // Only a NEW key can grow the map; an overwrite that re-announced
            // the same count would cry wolf on every catalog request.
            is_new_key.then(|| map.len())
        };
        if let Some(entries) = entries {
            if is_growth_notice(entries) {
                tracing::warn!(
                    "ETag cache now holds {entries} distinct keys. It has no capacity, no \
                     expiry and no eviction, and the key folds caller-supplied filter values \
                     (src/api/bikes.rs:381-395), so this count is shaped by callers and only \
                     a restart lowers it. No ceiling is published anywhere in the tree; this \
                     line is the measurement, not a limit."
                );
            }
        }
        hash
    }

    /// Check if data has changed based on ETag
    pub async fn has_changed(&self, key: &str, current_data: &str) -> (bool, String) {
        let current_hash = Self::compute_hash(current_data);
        match self.get(key).await {
            Some(cached_hash) if cached_hash == current_hash => (false, current_hash),
            _ => {
                self.set(key, current_data).await;
                (true, current_hash)
            }
        }
    }
}

impl Default for ETagCache {
    fn default() -> Self {
        Self::new()
    }
}

/// True at 10, 100, 1000 … and at no other count.
///
/// Free function rather than a closure inside `set` so it can be tested
/// directly: the branch it guards fires at most nine times in the life of a
/// process, which is exactly the shape of a branch that is never exercised and
/// silently stops working. `checked_mul` because the decade walk would
/// otherwise overflow before it reached `usize::MAX`.
fn is_growth_notice(entries: usize) -> bool {
    let mut decade: usize = 10;
    while decade <= entries {
        if decade == entries {
            return true;
        }
        match decade.checked_mul(10) {
            Some(next) => decade = next,
            None => return false,
        }
    }
    false
}

/// Update ETag cache after data modification. Clears both the public and the
/// admin (`include_hidden`) ETag keys so neither variant serves a stale 304.
pub(crate) async fn invalidate_strains(cache: &ETagCache) {
    let mut h = cache.hashes.write().await;
    h.remove("strains");
    h.remove("strains_admin");
}

// Используется в будущих cache invalidation путях
#[allow(dead_code)]
pub(crate) async fn invalidate_accessories(cache: &ETagCache) {
    cache.hashes.write().await.remove("accessories");
}

// Используется в будущих cache invalidation путях
#[allow(dead_code)]
pub(crate) async fn invalidate_sets(cache: &ETagCache) {
    cache.hashes.write().await.remove("sets");
}

// Используется в будущих cache invalidation путях
#[allow(dead_code)]
pub(crate) async fn invalidate_tea_products(cache: &ETagCache) {
    cache.hashes.write().await.remove("tea_products");
}

/// Create ETag header value
pub(crate) fn make_etag_header(hash: &str) -> HeaderValue {
    HeaderValue::from_str(&format!("\"{}\"", hash))
        .unwrap_or_else(|_| HeaderValue::from_static("\"\""))
}

#[cfg(test)]
mod tests {
    use super::{is_growth_notice, make_etag_header, ETagCache};
    use axum::http::HeaderValue;

    /// The tag is the WHOLE digest, and the anchor is the standard.
    ///
    /// Until 2026-09-21 `compute_hash` kept `hash[0..8]` and rendered sixteen
    /// hex characters, discarding 24 of SHA-256's 32 bytes. That truncation was
    /// the single way the re-hash guarantee could fail: `has_changed` compares
    /// the stored digest against a freshly computed one, so a body that changed
    /// reads as unchanged exactly when the two truncations collide, and the
    /// client is then told 304 about a body it has never seen. 64 bits is a
    /// birthday collision at ~2^32 distinct bodies; the full digest is not.
    ///
    /// The expected values are FIPS 180-4's own worked examples rather than a
    /// second call into `sha2`: a test that computes its subject the way its
    /// subject does would have stayed green through the truncation too.
    #[test]
    fn the_tag_is_the_whole_sha256_digest() {
        assert_eq!(
            ETagCache::compute_hash("abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            ETagCache::compute_hash(""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(ETagCache::compute_hash("{\"bikes\":[]}").len(), 64);
    }

    /// The notice fires on a decade and nowhere else.
    ///
    /// A decade is a reporting cadence, not a capacity: nothing refuses, evicts
    /// or changes behaviour at 10 or at 100. It is written this way because no
    /// capacity for this map is published anywhere in the tree, and a number
    /// invented here would read as a policy the owner never set.
    #[test]
    fn a_growth_notice_fires_on_a_decade_and_nowhere_else() {
        assert!(!is_growth_notice(0));
        assert!(!is_growth_notice(1));
        assert!(!is_growth_notice(9));
        assert!(is_growth_notice(10));
        assert!(!is_growth_notice(11));
        assert!(!is_growth_notice(99));
        assert!(is_growth_notice(100));
        assert!(!is_growth_notice(999));
        assert!(is_growth_notice(1000));
        assert!(is_growth_notice(1_000_000));
        assert!(!is_growth_notice(1_000_001));
    }

    /// The map can be asked how big it has become.
    ///
    /// Growth is the half of this module nothing could see: the key folds
    /// caller-supplied filter values (`src/api/bikes.rs:381-395`), there is no
    /// capacity, no expiry and no eviction, and before this the only way to
    /// learn the size was a debugger. A count that re-counts overwrites as new
    /// entries would announce growth that never happened, so the same-key case
    /// is pinned here.
    #[tokio::test]
    async fn the_cache_reports_how_many_keys_it_holds() {
        let cache = ETagCache::new();
        assert_eq!(cache.entry_count().await, 0);
        cache.set("bikes", "a").await;
        cache.set("bikes|class=scooter|min=*|max=*", "b").await;
        cache.set("bikes", "c").await;
        assert_eq!(cache.entry_count().await, 2);

        // Walk the map past the first decade so the notice branch in `set` is
        // entered at least once by the suite. It fires nine times in the life
        // of a process, which is exactly the shape of a branch nothing ever
        // executes and everyone assumes still works; the keys below are the
        // shape a caller really produces (src/api/bikes.rs:381-395).
        for cc in 0..10 {
            cache
                .set(&format!("bikes|class=*|min={cc}|max=*"), "body")
                .await;
        }
        assert_eq!(cache.entry_count().await, 12);
    }

    #[test]
    fn test_compute_hash_deterministic() {
        let h1 = ETagCache::compute_hash("abc");
        let h2 = ETagCache::compute_hash("abc");
        assert_eq!(h1, h2);
        assert!(!h1.is_empty());
    }

    #[test]
    fn test_compute_hash_different_input() {
        let h1 = ETagCache::compute_hash("abc");
        let h2 = ETagCache::compute_hash("def");
        assert_ne!(h1, h2);
    }

    #[tokio::test]
    async fn test_cache_get_set() {
        let cache = ETagCache::new();
        assert!(cache.get("key1").await.is_none());
        let hash = cache.set("key1", "data").await;
        assert!(!hash.is_empty());
        assert_eq!(cache.get("key1").await, Some(hash));
    }

    #[tokio::test]
    async fn test_cache_has_changed() {
        let cache = ETagCache::new();
        let (changed, hash1) = cache.has_changed("k", "v1").await;
        assert!(changed);
        let (changed2, hash2) = cache.has_changed("k", "v1").await;
        assert!(!changed2);
        assert_eq!(hash1, hash2);
        let (changed3, hash3) = cache.has_changed("k", "v2").await;
        assert!(changed3);
        assert_ne!(hash2, hash3);
    }

    #[test]
    fn test_make_etag_header() {
        let h = make_etag_header("abc123");
        assert_eq!(h, HeaderValue::from_static("\"abc123\""));
    }
}
