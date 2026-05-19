// Simplified frontend caching with localStorage
use serde::{Deserialize, Serialize};
use gloo_storage::{LocalStorage, Storage};
use std::time::{Duration, SystemTime};

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
        let storage_key: String = "cache_strains".into();
        if let Ok(data_str) = LocalStorage::get::<String>(storage_key) {
            if let Ok(cached) = serde_json::from_str::<CachedData<Vec<crate::ui::screens::menu_screen::ApiStrain>>>(&data_str) {
                if cached.version == CACHE_VERSION {
                    let now = SystemTime::now()
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis() as u64;

                    if now - cached.timestamp_ms < CACHE_TTL_MS {
                        return Some(cached.data);
                    }
                }
            }
        }
        None
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
            let storage_key: String = "cache_strains".into();
            let _ = LocalStorage::set(storage_key, data_str);
        }
    }
}