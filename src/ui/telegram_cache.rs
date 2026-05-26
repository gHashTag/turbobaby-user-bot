// Telegram Cloud Storage for Mini Apps
use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

const CACHE_TTL_MS: u64 = 5 * 60 * 1000; // 5 минут

/// Access Telegram Cloud Storage
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = Telegram, js_name = WebApp)]
    type TelegramWebApp;

    #[wasm_bindgen(static_method_of = TelegramWebApp, js_name = initData)]
    fn init_data() -> JsValue;

    #[wasm_bindgen(method, js_name = CloudStorage)]
    fn cloud_storage(this: &TelegramWebApp) -> CloudStorage;

    #[wasm_bindgen(js_namespace = Telegram, js_name = WebApp)]
    static WebApp: TelegramWebApp;
}

#[wasm_bindgen]
extern "C" {
    type CloudStorage;

    #[wasm_bindgen(method, catch)]
    async fn set_item(this: &CloudStorage, key: &str, value: &str) -> Result<(), JsValue>;

    #[wasm_bindgen(method, catch)]
    async fn get_item(this: &CloudStorage, key: &str) -> Result<JsValue, JsValue>;

    #[wasm_bindgen(method, catch)]
    async fn remove_item(this: &CloudStorage, key: &str) -> Result<(), JsValue>;

    #[wasm_bindgen(method, catch)]
    async fn get_keys(this: &CloudStorage) -> Result<JsValue, JsValue>;
}

#[derive(Clone, Serialize, Deserialize)]
struct CachedData<T> {
    version: String,
    timestamp_ms: u64,
    data: T,
}

/// Telegram Cloud Storage cache manager
#[derive(Clone, Default)]
pub struct TelegramCache;

impl TelegramCache {
    /// Check if Telegram WebApp is available
    pub fn is_available() -> bool {
        // Check if window.Telegram exists
        if let Ok(window) = web_sys::window() {
            if let Some(telegram) = window.get("Telegram") {
                return !telegram.is_undefined();
            }
        }
        false
    }

    /// Set item in Telegram Cloud Storage
    pub async fn set_item(key: &str, value: &str) -> Result<(), String> {
        if !Self::is_available() {
            return Err("Telegram WebApp not available".to_string());
        }

        if let Ok(storage) = Self::get_storage() {
            storage.set_item(key, value).await
                .map_err(|e| format!("Storage error: {:?}", e))?;
        }
        Ok(())
    }

    /// Get item from Telegram Cloud Storage
    pub async fn get_item(key: &str) -> Result<Option<String>, String> {
        if !Self::is_available() {
            return Ok(None);
        }

        if let Ok(storage) = Self::get_storage() {
            let value = storage.get_item(key).await;
            match value {
                Ok(js_val) => {
                    if js_val.is_undefined() || js_val.is_null() {
                        Ok(None)
                    } else if let Some(s) = js_val.as_string() {
                        Ok(Some(s))
                    } else {
                        Ok(None)
                    }
                }
                Err(_) => Ok(None),
            }
        } else {
            Ok(None)
        }
    }

    /// Get cached strains if valid
    pub async fn get_strains() -> Option<Vec<crate::ui::screens::menu_screen::ApiStrain>>
    where
        crate::ui::screens::menu_screen::ApiStrain: serde::de::DeserializeOwned,
    {
        let key = "cache_strains_v1";

        match Self::get_item(key).await {
            Ok(Some(data_str)) => {
                if let Ok(cached) = serde_json::from_str::<CachedData<Vec<crate::ui::screens::menu_screen::ApiStrain>>>(&data_str) {
                    let now = Self::now_ms();
                    if cached.version == "v1" && (now - cached.timestamp_ms) < CACHE_TTL_MS {
                        return Some(cached.data);
                    }
                }
                None
            }
            _ => None,
        }
    }

    /// Store strains in cache
    pub async fn set_strains(data: Vec<crate::ui::screens::menu_screen::ApiStrain>)
    where
        crate::ui::screens::menu_screen::ApiStrain: Serialize,
    {
        let cached = CachedData {
            version: "v1".to_string(),
            timestamp_ms: Self::now_ms(),
            data,
        };

        if let Ok(data_str) = serde_json::to_string(&cached) {
            let _ = Self::set_item("cache_strains_v1", &data_str).await;
        }
    }

    /// Get current timestamp in ms
    fn now_ms() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64
    }

    /// Get Telegram Cloud Storage instance
    fn get_storage() -> Result<CloudStorage, JsValue> {
        #[wasm_bindgen]
        extern "C" {
            type Telegram;
            #[wasm_bindgen(static_method_of = Telegram)]
            fn WebApp() -> JsValue;
        }

        let tg_webapp = js_sys::Reflect::get(&Telegram::WebApp(), &JsValue::from_str("WebApp"))
            .unwrap_or_else(|_| JsValue::NULL);

        let storage = js_sys::Reflect::get(&tg_webapp, &JsValue::from_str("CloudStorage"))
            .unwrap_or_else(|_| JsValue::NULL);

        if storage.is_undefined() || storage.is_null() {
            return Err(JsValue::from_str("CloudStorage not available"));
        }
        Ok(storage.unchecked_into::<CloudStorage>())
    }
}