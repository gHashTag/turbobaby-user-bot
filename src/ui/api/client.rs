// Typed API client — gloo-net under the hood (saves ≈300 KB vs reqwest in
// the WASM bundle). Public API unchanged; internals use gloo_net::http::Request
// directly so we can attach the X-Telegram-Init-Data header per call without
// shipping reqwest's generic HTTP stack.

use super::types::*;
use gloo_net::http::Request;
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
    /// Telegram WebApp initData; attached as `X-Telegram-Init-Data` to every
    /// request. Caps inbound size to avoid hostile bloat header attacks.
    init_data: String,
}

impl ApiClient {
    pub fn new(base_url: String, init_data: String) -> Self {
        let init_data = if init_data.len() > 4096 {
            String::new()
        } else {
            init_data
        };
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            init_data,
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

    /// Attach the auth header if init_data is non-empty.
    fn with_auth(&self, req: gloo_net::http::RequestBuilder) -> gloo_net::http::RequestBuilder {
        if self.init_data.is_empty() {
            req
        } else {
            req.header("x-telegram-init-data", &self.init_data)
        }
    }

    /// Run the request, parse body, classify status codes into our ApiError.
    async fn run<T: for<'de> Deserialize<'de>>(req: gloo_net::http::Request) -> Result<T> {
        let resp = req
            .send()
            .await
            .map_err(|e| ApiError::Network(e.to_string()))?;
        let status = resp.status();
        if (200..300).contains(&status) {
            let text = resp
                .text()
                .await
                .map_err(|e| ApiError::Parse(e.to_string()))?;
            serde_json::from_str(&text).map_err(|e| ApiError::Parse(e.to_string()))
        } else {
            let message = resp.text().await.unwrap_or_default();
            Err(ApiError::Api { status, message })
        }
    }

    async fn get<T: for<'de> Deserialize<'de>>(&self, path: &str) -> Result<T> {
        let url = self.build_url(path);
        let req = self
            .with_auth(Request::get(&url))
            .build()
            .map_err(|e| ApiError::Network(e.to_string()))?;
        Self::run(req).await
    }

    async fn post<T: for<'de> Deserialize<'de>, P: Serialize>(
        &self,
        path: &str,
        body: &P,
    ) -> Result<T> {
        let url = self.build_url(path);
        let body_str = serde_json::to_string(body).map_err(|e| ApiError::Parse(e.to_string()))?;
        let req = self
            .with_auth(Request::post(&url).header("content-type", "application/json"))
            .body(body_str)
            .map_err(|e| ApiError::Network(e.to_string()))?;
        Self::run(req).await
    }

    async fn delete<T: for<'de> Deserialize<'de>>(&self, path: &str) -> Result<T> {
        let url = self.build_url(path);
        let req = self
            .with_auth(Request::delete(&url))
            .build()
            .map_err(|e| ApiError::Network(e.to_string()))?;
        Self::run(req).await
    }

    // Strain endpoints
    pub async fn get_strains(&self) -> Result<Vec<Strain>> {
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
        self.delete(&format!("/api/accessories/{}", urlencoding::encode(id)))
            .await
    }

    pub async fn create_accessory(&self, req: &AccessoryRequest) -> Result<Value> {
        self.post("/api/accessories", req).await
    }

    pub async fn get_sets(&self) -> Result<Vec<Set>> {
        #[derive(Deserialize)]
        struct SetsResponse {
            sets: Vec<Set>,
        }
        let resp: SetsResponse = self.get("/api/sets").await?;
        Ok(resp.sets)
    }

    pub async fn get_tea_products(&self) -> Result<Vec<TeaProduct>> {
        #[derive(Deserialize)]
        struct TeaResponse {
            #[serde(default)]
            products: Vec<TeaProduct>,
            #[serde(default)]
            tea_products: Vec<TeaProduct>,
        }
        let resp: TeaResponse = self.get("/api/tea-products").await?;
        Ok(if !resp.products.is_empty() {
            resp.products
        } else {
            resp.tea_products
        })
    }

    // Order endpoints
    pub async fn get_user_orders(&self, telegram_id: i64) -> Result<Vec<Order>> {
        #[derive(Deserialize)]
        struct OrdersResponse {
            orders: Vec<Order>,
        }
        let resp: OrdersResponse = self
            .get(&format!("/api/orders/user/{}", telegram_id))
            .await?;
        Ok(resp.orders)
    }

    pub async fn create_order(&self, order: &CreateOrderRequest) -> Result<Order> {
        self.post("/api/orders", order).await
    }

    // Garden endpoints
    pub async fn get_user_plants(&self, telegram_id: i64) -> Result<Vec<Plant>> {
        #[derive(Deserialize)]
        struct PlantsResponse {
            plants: Vec<Plant>,
        }
        let resp: PlantsResponse = self
            .get(&format!("/api/garden/plants?telegram_id={}", telegram_id))
            .await?;
        Ok(resp.plants)
    }

    // Quest endpoints
    pub async fn get_quest_locations(&self) -> Result<Vec<QuestLocation>> {
        #[derive(Deserialize)]
        struct LocationsResponse {
            locations: Vec<QuestLocation>,
        }
        let resp: LocationsResponse = self.get("/api/quest/locations").await?;
        Ok(resp.locations)
    }

    pub async fn scan_qr_code(&self, code: &str) -> Result<ScanResponse> {
        self.post(
            "/api/quest/scan",
            &ScanRequest {
                code: code.to_string(),
            },
        )
        .await
    }

    // Loyalty endpoints
    pub async fn get_loyalty_profile(&self, telegram_id: i64) -> Result<LoyaltyProfile> {
        #[derive(Deserialize)]
        struct ProfileResponse {
            profile: LoyaltyProfile,
        }
        let resp: ProfileResponse = self.get(&format!("/api/loyalty/{}", telegram_id)).await?;
        Ok(resp.profile)
    }

    // Strain of day
    pub async fn get_strain_of_day(&self) -> Result<Vec<Strain>> {
        #[derive(Deserialize)]
        struct StrainsResponse {
            strains: Vec<Strain>,
        }
        let resp: StrainsResponse = self.get("/api/strains/strain-of-day").await?;
        Ok(resp.strains)
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
pub struct ScanRequest {
    pub code: String,
}

#[derive(Debug, Deserialize)]
pub struct ScanResponse {
    pub success: bool,
    pub message: String,
    pub reward: Option<String>,
}
