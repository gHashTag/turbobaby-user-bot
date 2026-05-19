// Simplified frontend caching with localStorage
use serde::{Deserialize, Serialize};
use gloo_storage::{LocalStorage, Storage};
use std::time::SystemTime;

const CACHE_VERSION: &str = "v1";
const CACHE_TTL_MS: u64 = 5 * 60 * 1000; // 5 минут

#[derive(Clone, Serialize, Deserialize)]
struct CachedData<T> {
    version: String,
    timestamp_ms: u64,
    etag: Option<String>,
    data: T,
}

/// Simple cache manager using localStorage
#[derive(Clone, Default)]
pub struct CacheManager;

impl CacheManager {
    pub fn new() -> Self {
        Self {}
    }

    /// Get cached strains if valid
    pub fn get_strains(&self) -> Option<Vec<crate::ui::screens::menu_screen::ApiStrain>>
    where
        crate::ui::screens::menu_screen::ApiStrain: serde::de::DeserializeOwned,
    {
        use gloo_storage::errors::StorageError;

        let storage_key = "cache_strains";
        let data_str = match LocalStorage::get::<String>(storage_key) {
            Ok(s) => s,
            Err(StorageError::KeyNotFound(_)) => return None,
            Err(_) => return None, // Any storage error, skip cache
        };

        let cached = match serde_json::from_str::<CachedData<Vec<crate::ui::screens::menu_screen::ApiStrain>>>(&data_str) {
            Ok(c) => c,
            Err(_) => return None, // JSON parse error, skip cache
        };

        if cached.version != CACHE_VERSION {
            return None; // Version mismatch, skip cache
        }

        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        if now - cached.timestamp_ms < CACHE_TTL_MS {
            Some(cached.data)
        } else {
            None // Expired
        }
    }

    /// Store strains in cache
    pub fn set_strains(&self, data: Vec<crate::ui::screens::menu_screen::ApiStrain>, etag: Option<String>)
    where
        crate::ui::screens::menu_screen::ApiStrain: Serialize,
    {
        let cached = CachedData {
            version: CACHE_VERSION.to_string(),
            timestamp_ms: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            etag,
            data,
        };

        if let Ok(data_str) = serde_json::to_string(&cached) {
            let _ = LocalStorage::set("cache_strains", data_str);
        }
    }
}