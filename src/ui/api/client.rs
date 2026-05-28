// API Client - Simplified version
use super::types::*;
use reqwest::Client;
use std::sync::Arc;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ApiError {
    #[error("Network error: {0}")]
    Network(String),

    #[error("Parse error: {0}")]
    Parse(String),

    #[error("API error: {status} - {message}")]
    Api { status: u16, message: String },
}

pub type Result<T> = std::result::Result<T, ApiError>;

#[derive(Clone)]
pub struct ApiClient {
    base_url: String,
    client: Arc<Client>,
}

impl ApiClient {
    pub fn new(base_url: String, init_data: String) -> Self {
        let mut headers = reqwest::header::HeaderMap::new();
        if !init_data.is_empty() {
            if let Ok(val) = reqwest::header::HeaderValue::from_str(&init_data) {
                headers.insert("X-Telegram-Init-Data", val);
            }
        }
        let client = Client::builder()
            .default_headers(headers)
            .build()
            .unwrap_or_else(|_| Client::new());
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            client: Arc::new(client),
        }
    }

    /// Build absolute URL: base + path with exactly one '/' between them.
    /// Works for empty base_url (relative URLs from browser origin).
    fn build_url(&self, path: &str) -> String {
        let p = path.trim_start_matches('/');
        if self.base_url.is_empty() {
            format!("/{p}")
        } else {
            format!("{}/{p}", self.base_url)
        }
    }

    async fn get<T: for<'de> Deserialize<'de>>(&self, path: &str) -> Result<T> {
        let url = self.build_url(path);
        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| ApiError::Network(e.to_string()))?;

        if response.status().is_success() {
            response
                .json()
                .await
                .map_err(|e| ApiError::Parse(e.to_string()))
        } else {
            let status = response.status().as_u16();
            let text = response.text().await.unwrap_or_default();
            Err(ApiError::Api { status, message: text })
        }
    }

    async fn post<T: for<'de> Deserialize<'de>, P: Serialize>(
        &self,
        path: &str,
        body: &P,
    ) -> Result<T> {
        let url = self.build_url(path);
        let response = self
            .client
            .post(&url)
            .json(body)
            .send()
            .await
            .map_err(|e| ApiError::Network(e.to_string()))?;

        if response.status().is_success() {
            response
                .json()
                .await
                .map_err(|e| ApiError::Parse(e.to_string()))
        } else {
            let status = response.status().as_u16();
            let text = response.text().await.unwrap_or_default();
            Err(ApiError::Api { status, message: text })
        }
    }

    async fn delete<T: for<'de> Deserialize<'de>>(&self, path: &str) -> Result<T> {
        let url = self.build_url(path);
        let response = self
            .client
            .delete(&url)
            .send()
            .await
            .map_err(|e| ApiError::Network(e.to_string()))?;

        if response.status().is_success() {
            response
                .json()
                .await
                .map_err(|e| ApiError::Parse(e.to_string()))
        } else {
            let status = response.status().as_u16();
            let text = response.text().await.unwrap_or_default();
            Err(ApiError::Api { status, message: text })
        }
    }

    // Strain endpoints
    pub async fn get_strains(&self) -> Result<Vec<Strain>> {
        // API returns {"strains": [...]} wrapper
        #[derive(Deserialize)]
        struct StrainsResponse {
            strains: Vec<Strain>,
        }
        let resp: StrainsResponse = self.get("/api/strains").await?;
        Ok(resp.strains)
    }

    // Catalog endpoints
    pub async fn get_accessories(&self) -> Result<Vec<Accessory>> {
        #[derive(Deserialize)]
        struct AccessoriesResponse {
            accessories: Vec<Accessory>,
        }
        let resp: AccessoriesResponse = self.get("/api/accessories").await?;
        Ok(resp.accessories)
    }

    pub async fn delete_accessory(&self, id: &str) -> Result<Value> {
        self.delete(&format!("/api/accessories/{}", id)).await
    }

    pub async fn create_accessory(&self, req: &AccessoryRequest) -> Result<Value> {
        self.post("/api/accessories", req).await
    }

    pub async fn get_sets(&self) -> Result<Vec<Set>> {
        self.get("/api/sets").await
    }

    pub async fn get_tea_products(&self) -> Result<Vec<TeaProduct>> {
        // API returns {"tea_products": [...], "products": [...]} — use `products` key.
        #[derive(Deserialize)]
        struct TeaResponse {
            #[serde(default)]
            products: Vec<TeaProduct>,
            #[serde(default)]
            tea_products: Vec<TeaProduct>,
        }
        let resp: TeaResponse = self.get("/api/tea-products").await?;
        Ok(if !resp.products.is_empty() { resp.products } else { resp.tea_products })
    }

    // Order endpoints
    pub async fn get_user_orders(&self, telegram_id: i64) -> Result<Vec<Order>> {
        self.get(&format!("/api/orders/user/{}", telegram_id)).await
    }

    pub async fn create_order(&self, order: &CreateOrderRequest) -> Result<Order> {
        self.post("/api/orders", order).await
    }

    // Garden endpoints
    pub async fn get_user_plants(&self, telegram_id: i64) -> Result<Vec<Plant>> {
        self.get(&format!("/api/garden/plants?telegram_id={}", telegram_id))
            .await
    }

    pub async fn plant_seed(
        &self,
        request: &PlantSeedRequest,
    ) -> Result<Plant> {
        self.post("/api/garden/plants", request).await
    }

    pub async fn water_plant(&self, plant_id: &str) -> Result<Plant> {
        self.post(&format!("/api/garden/plants/{}/water", plant_id), &())
            .await
    }

    pub async fn harvest_plant(&self, plant_id: &str) -> Result<Plant> {
        self.post(&format!("/api/garden/plants/{}/harvest", plant_id), &())
            .await
    }

    // Quest endpoints
    pub async fn get_quest_locations(&self) -> Result<Vec<QuestLocation>> {
        self.get("/api/quest/locations").await
    }

    pub async fn scan_qr_code(&self, code: &str) -> Result<ScanResponse> {
        self.post("/api/quest/scan", &ScanRequest { code: code.to_string() })
            .await
    }

    // Loyalty endpoints
    pub async fn get_loyalty_profile(&self, telegram_id: i64) -> Result<LoyaltyProfile> {
        self.get(&format!("/api/loyalty/{}", telegram_id)).await
    }

    // Strain of day
    pub async fn get_strain_of_day(&self) -> Result<Vec<Strain>> {
        self.get("/api/strains/strain-of-day").await
    }
}

#[derive(Debug, Serialize)]
pub struct CreateOrderRequest {
    pub telegram_id: i64,
    pub items: Vec<OrderItemRequest>,
}

#[derive(Debug, Serialize)]
pub struct OrderItemRequest {
    pub id: String,
    pub name: String,
    pub quantity: u32,
    pub price: f64,
}

#[derive(Debug, Serialize)]
pub struct PlantSeedRequest {
    pub telegram_id: i64,
    pub strain_id: String,
    pub strain_name: String,
}

#[derive(Debug, Serialize)]
pub struct ScanRequest {
    pub code: String,
}

#[derive(Debug, Deserialize)]
pub struct ScanResponse {
    pub success: bool,
    pub message: String,
    pub reward: Option<String>,
}
