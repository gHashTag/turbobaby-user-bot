use axum::http::HeaderValue;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Simple in-memory ETag cache for API responses.
/// Uses SHA-256 truncated to 16 hex chars so the hash is stable across
/// process restarts (DefaultHasher is NOT stable across Rust releases).
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

    /// Compute stable hash of JSON response (first 64 bits of SHA-256).
    pub fn compute_hash(data: &str) -> String {
        let hash = Sha256::digest(data.as_bytes());
        // Cycle #77: self-documenting expect — SHA-256 digest is fixed
        // at 32 bytes by spec, so [..8] is always exactly 8 bytes and
        // try_into::<[u8; 8]>() always succeeds. If a future refactor
        // swaps in a different hash with shorter output, this panics
        // loudly with the invariant message.
        let arr: [u8; 8] = hash[0..8]
            .try_into()
            .expect("SHA-256 digest is 32 bytes; [..8] is always 8");
        format!("{:016x}", u64::from_be_bytes(arr))
    }

    /// Get cached hash for a key
    pub async fn get(&self, key: &str) -> Option<String> {
        self.hashes.read().await.get(key).cloned()
    }

    /// Set hash for a key
    pub async fn set(&self, key: &str, data: &str) -> String {
        let hash = Self::compute_hash(data);
        self.hashes
            .write()
            .await
            .insert(key.to_string(), hash.clone());
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

/// Update ETag cache after data modification
pub async fn invalidate_strains(cache: &ETagCache) {
    cache.hashes.write().await.remove("strains");
}

// Используется в будущих cache invalidation путях
#[allow(dead_code)]
pub async fn invalidate_accessories(cache: &ETagCache) {
    cache.hashes.write().await.remove("accessories");
}

// Используется в будущих cache invalidation путях
#[allow(dead_code)]
pub async fn invalidate_sets(cache: &ETagCache) {
    cache.hashes.write().await.remove("sets");
}

// Используется в будущих cache invalidation путях
#[allow(dead_code)]
pub async fn invalidate_tea_products(cache: &ETagCache) {
    cache.hashes.write().await.remove("tea_products");
}

/// Create ETag header value
pub fn make_etag_header(hash: &str) -> HeaderValue {
    HeaderValue::from_str(&format!("\"{}\"", hash))
        .unwrap_or_else(|_| HeaderValue::from_static("\"\""))
}

#[cfg(test)]
mod tests {
    use super::{make_etag_header, ETagCache};
    use axum::http::HeaderValue;

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
