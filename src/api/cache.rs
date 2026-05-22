use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::Hasher;
use std::sync::Arc;
use tokio::sync::RwLock;
use axum::{
    http::{HeaderValue},
};

/// Simple in-memory ETag cache for API responses
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

    /// Compute hash of JSON response using standard hasher
    pub fn compute_hash(data: &str) -> String {
        let mut hasher = DefaultHasher::new();
        hasher.write(data.as_bytes());
        format!("{:x}", hasher.finish())
    }

    /// Get cached hash for a key
    pub async fn get(&self, key: &str) -> Option<String> {
        self.hashes.read().await.get(key).cloned()
    }

    /// Set hash for a key
    pub async fn set(&self, key: &str, data: &str) -> String {
        let hash = Self::compute_hash(data);
        self.hashes.write().await.insert(key.to_string(), hash.clone());
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
    HeaderValue::from_str(&format!("\"{}\"", hash)).unwrap_or_else(|_| HeaderValue::from_static("\"\""))
}