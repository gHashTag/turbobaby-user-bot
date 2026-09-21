//! Tiny HTTP helper for UI fetch sites.
//!
//! Uses `gloo-net` instead of `reqwest`, saving ≈300 KB in the resulting
//! `.wasm` bundle. Gloo-net is a thin wrapper around the browser's native
//! `fetch` via `web_sys`, so it doesn't ship a generic HTTP stack the
//! browser already provides.
//!
//! Surface intentionally tiny — just what the existing `use_resource`
//! call sites need:
//!   - `fetch_text(url)` → GET, return body as String
//!   - `fetch_text_authed(url, init_data)` → GET with X-Telegram-Init-Data
//!   - `post_json(url, body)` → POST application/json, return response body
//!   - `post_json_authed(url, init_data, body)` → same with auth header
//!
//! Errors are stringified so they pass through `use_resource`'s
//! `Result<T, String>` shape unchanged.

use gloo_net::http::Request;
use serde::{Deserialize, Serialize};

use crate::ui::api::types::ServerCartItem;
use crate::ui::state::{Cart, CartItem, CartItemType};

/// GET `url`, return the response body as text. Non-2xx responses surface
/// as `Err(format!("HTTP {}: …"))` so callers can render them directly.
pub async fn fetch_text(url: &str) -> Result<String, String> {
    let resp = Request::get(url)
        .send()
        .await
        .map_err(|e| format!("Network error: {e}"))?;
    if !resp.ok() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!(
            "HTTP {status}: {}",
            body.chars().take(120).collect::<String>()
        ));
    }
    resp.text().await.map_err(|e| format!("Read error: {e}"))
}

/// GET with `X-Telegram-Init-Data` header attached.
pub async fn fetch_text_authed(url: &str, init_data: &str) -> Result<String, String> {
    let resp = Request::get(url)
        .header("x-telegram-init-data", init_data)
        .header(
            "x-telegram-init-data-reconstructed",
            if crate::ui::telegram::TelegramApp::init().is_reconstructed_init_data() {
                "1"
            } else {
                "0"
            },
        )
        .send()
        .await
        .map_err(|e| format!("Network error: {e}"))?;
    if !resp.ok() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!(
            "HTTP {status}: {}",
            body.chars().take(240).collect::<String>()
        ));
    }
    resp.text().await.map_err(|e| format!("Read error: {e}"))
}

/// GET `url`, surface full `(status, body)` so callers can route through a
/// shared `friendly_response_error(lang, status)` helper (cycle #74). Mirror
/// of [`post_json_authed_idempotent_full`] for the GET side — added so
/// `menu` / `ar_hunt` / `location_quest` screens stop rendering raw err
/// strings.
pub async fn fetch_text_full(url: &str) -> Result<(u16, String), String> {
    let resp = Request::get(url)
        .send()
        .await
        .map_err(|e| format!("Network error: {e}"))?;
    let status = resp.status();
    let body = resp.text().await.map_err(|e| format!("Read error: {e}"))?;
    Ok((status, body))
}

/// GET with `X-Telegram-Init-Data`, returns `(status, body)`. See
/// [`fetch_text_full`] for rationale.
pub async fn fetch_text_authed_full(url: &str, init_data: &str) -> Result<(u16, String), String> {
    let resp = Request::get(url)
        .header("x-telegram-init-data", init_data)
        .header(
            "x-telegram-init-data-reconstructed",
            if crate::ui::telegram::TelegramApp::init().is_reconstructed_init_data() {
                "1"
            } else {
                "0"
            },
        )
        .send()
        .await
        .map_err(|e| format!("Network error: {e}"))?;
    let status = resp.status();
    let body = resp.text().await.map_err(|e| format!("Read error: {e}"))?;
    Ok((status, body))
}

/// POST `body` as JSON and return only the HTTP status. Telemetry calls
/// (e.g. `/api/client-errors`) don't need the body and may not be
/// authenticated, so we avoid sending sensitive headers.
pub async fn post_json_status_only(url: &str, body: &str) -> Result<u16, String> {
    let resp = Request::post(url)
        .header("content-type", "application/json")
        .body(body.to_string())
        .map_err(|e| format!("Build error: {e}"))?
        .send()
        .await
        .map_err(|e| format!("Network error: {e}"))?;
    Ok(resp.status())
}

/// POST `body` as JSON, return response body as text.
pub async fn post_json(url: &str, body: &str) -> Result<String, String> {
    let resp = Request::post(url)
        .header("content-type", "application/json")
        .body(body.to_string())
        .map_err(|e| format!("Build error: {e}"))?
        .send()
        .await
        .map_err(|e| format!("Network error: {e}"))?;
    if !resp.ok() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!(
            "HTTP {status}: {}",
            body.chars().take(120).collect::<String>()
        ));
    }
    resp.text().await.map_err(|e| format!("Read error: {e}"))
}

/// POST with `X-Telegram-Init-Data` header attached.
pub async fn post_json_authed(url: &str, init_data: &str, body: &str) -> Result<String, String> {
    let resp = Request::post(url)
        .header("content-type", "application/json")
        .header("x-telegram-init-data", init_data)
        .header(
            "x-telegram-init-data-reconstructed",
            if crate::ui::telegram::TelegramApp::init().is_reconstructed_init_data() {
                "1"
            } else {
                "0"
            },
        )
        .body(body.to_string())
        .map_err(|e| format!("Build error: {e}"))?
        .send()
        .await
        .map_err(|e| format!("Network error: {e}"))?;
    if !resp.ok() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!(
            "HTTP {status}: {}",
            body.chars().take(120).collect::<String>()
        ));
    }
    resp.text().await.map_err(|e| format!("Read error: {e}"))
}

/// POST with `X-Telegram-Init-Data` + `X-Idempotency-Key`. The key should be a
/// stable UUID v4 per logical submit — retries of the same submit must reuse
/// the same key so the server collapses them into one order.
pub async fn post_json_authed_idempotent(
    url: &str,
    init_data: &str,
    idempotency_key: &str,
    body: &str,
) -> Result<String, String> {
    let resp = Request::post(url)
        .header("content-type", "application/json")
        .header("x-telegram-init-data", init_data)
        .header(
            "x-telegram-init-data-reconstructed",
            if crate::ui::telegram::TelegramApp::init().is_reconstructed_init_data() {
                "1"
            } else {
                "0"
            },
        )
        .header("x-idempotency-key", idempotency_key)
        .body(body.to_string())
        .map_err(|e| format!("Build error: {e}"))?
        .send()
        .await
        .map_err(|e| format!("Network error: {e}"))?;
    if !resp.ok() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!(
            "HTTP {status}: {}",
            body.chars().take(120).collect::<String>()
        ));
    }
    resp.text().await.map_err(|e| format!("Read error: {e}"))
}

/// Same as [`post_json_authed_idempotent`] but surfaces the full `(status, body)`
/// on any HTTP response (including non-2xx). Network and build errors still
/// come back as `Err`. Used by checkout to parse server-side error codes like
/// `{ "code": "unavailable", "catalog": ..., "id": ... }` and turn them into
/// friendly UI messages (cycle #42).
pub async fn post_json_authed_idempotent_full(
    url: &str,
    init_data: &str,
    idempotency_key: &str,
    body: &str,
) -> Result<(u16, String), String> {
    let resp = Request::post(url)
        .header("content-type", "application/json")
        .header("x-telegram-init-data", init_data)
        .header(
            "x-telegram-init-data-reconstructed",
            if crate::ui::telegram::TelegramApp::init().is_reconstructed_init_data() {
                "1"
            } else {
                "0"
            },
        )
        .header("x-idempotency-key", idempotency_key)
        .body(body.to_string())
        .map_err(|e| format!("Build error: {e}"))?
        .send()
        .await
        .map_err(|e| format!("Network error: {e}"))?;
    let status = resp.status();
    let body = resp.text().await.map_err(|e| format!("Read error: {e}"))?;
    Ok((status, body))
}

/// POST JSON with optional admin token header. Returns response status code
/// as `u16` so callers can branch. Body may be empty (`""`) for actions that
/// take no payload (e.g. `/tech-tree/nodes/:id/complete`).
#[allow(dead_code)]
pub async fn post_admin_token(
    url: &str,
    admin_token: Option<&str>,
    body: &str,
) -> Result<(u16, String), String> {
    let mut req = Request::post(url).header("content-type", "application/json");
    if let Some(t) = admin_token {
        req = req.header("x-admin-token", t);
    }
    let resp = req
        .body(body.to_string())
        .map_err(|e| format!("Build error: {e}"))?
        .send()
        .await
        .map_err(|e| format!("Network error: {e}"))?;
    let status = resp.status();
    let body = resp.text().await.map_err(|e| format!("Read error: {e}"))?;
    Ok((status, body))
}

/// DELETE with `X-Telegram-Init-Data` header attached.
pub async fn delete_authed(url: &str, init_data: &str) -> Result<String, String> {
    let resp = Request::delete(url)
        .header("x-telegram-init-data", init_data)
        .header(
            "x-telegram-init-data-reconstructed",
            if crate::ui::telegram::TelegramApp::init().is_reconstructed_init_data() {
                "1"
            } else {
                "0"
            },
        )
        .send()
        .await
        .map_err(|e| format!("Network error: {e}"))?;
    if !resp.ok() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!(
            "HTTP {status}: {}",
            body.chars().take(120).collect::<String>()
        ));
    }
    resp.text().await.map_err(|e| format!("Read error: {e}"))
}

/// PUT JSON with Telegram initData. Returns response status code as `u16`.
pub async fn put_json_authed(
    url: &str,
    init_data: &str,
    body: &str,
) -> Result<(u16, String), String> {
    let resp = Request::put(url)
        .header("content-type", "application/json")
        .header("x-telegram-init-data", init_data)
        .header(
            "x-telegram-init-data-reconstructed",
            if crate::ui::telegram::TelegramApp::init().is_reconstructed_init_data() {
                "1"
            } else {
                "0"
            },
        )
        .body(body.to_string())
        .map_err(|e| format!("Build error: {e}"))?
        .send()
        .await
        .map_err(|e| format!("Network error: {e}"))?;
    let status = resp.status();
    let body = resp.text().await.map_err(|e| format!("Read error: {e}"))?;
    Ok((status, body))
}

/// PUT JSON with optional admin token header. Returns response status code as
/// `u16` so callers can branch (admin endpoints often return 401/404 with
/// meaningful bodies).
#[allow(dead_code)]
pub async fn put_json_admin(
    url: &str,
    admin_token: Option<&str>,
    body: &str,
) -> Result<(u16, String), String> {
    let mut req = Request::put(url).header("content-type", "application/json");
    if let Some(t) = admin_token {
        req = req.header("x-admin-token", t);
    }
    let resp = req
        .body(body.to_string())
        .map_err(|e| format!("Build error: {e}"))?
        .send()
        .await
        .map_err(|e| format!("Network error: {e}"))?;
    let status = resp.status();
    let body = resp.text().await.map_err(|e| format!("Read error: {e}"))?;
    Ok((status, body))
}

/// DELETE with optional admin token header. Returns response status code.
#[allow(dead_code)]
pub async fn delete_admin(url: &str, admin_token: Option<&str>) -> Result<(u16, String), String> {
    let mut req = Request::delete(url);
    if let Some(t) = admin_token {
        req = req.header("x-admin-token", t);
    }
    let resp = req
        .send()
        .await
        .map_err(|e| format!("Network error: {e}"))?;
    let status = resp.status();
    let body = resp.text().await.map_err(|e| format!("Read error: {e}"))?;
    Ok((status, body))
}

/// Admin auth context — three headers commonly attached together in the
/// admin screen.
#[derive(Clone, Default)]
#[allow(dead_code)]
pub struct AdminAuth<'a> {
    pub init_data: &'a str,
    pub token: &'a str,
    pub telegram_id: &'a str,
}

impl<'a> AdminAuth<'a> {
    #[allow(dead_code)]
    fn apply(&self, mut req: gloo_net::http::RequestBuilder) -> gloo_net::http::RequestBuilder {
        if !self.init_data.is_empty() {
            req = req.header("x-telegram-init-data", self.init_data);
        }
        if !self.token.is_empty() {
            req = req.header("x-admin-token", self.token);
        }
        if !self.telegram_id.is_empty() {
            req = req.header("x-admin-telegram-id", self.telegram_id);
        }
        req
    }
}

/// GET with full admin auth (Telegram init data + admin token + admin tg id).
/// Returns (status, body). Used by admin_screen's catalog reads.
#[allow(dead_code)]
pub async fn fetch_text_admin(url: &str, auth: &AdminAuth<'_>) -> Result<(u16, String), String> {
    let req = auth
        .apply(Request::get(url))
        .build()
        .map_err(|e| format!("Build error: {e}"))?;
    let resp = req
        .send()
        .await
        .map_err(|e| format!("Network error: {e}"))?;
    let status = resp.status();
    let body = resp.text().await.map_err(|e| format!("Read error: {e}"))?;
    Ok((status, body))
}

/// Loop #12: DTO for `POST /api/cart/merge`. The server is price-authoritative;
/// it resolves `catalog_id` against current DB prices and returns a fresh cart.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergeCartItemDto {
    pub id: String,
    pub kind: String,
    pub catalog_id: String,
    pub quantity: i32,
    /// `Option` for the same reason `ServerCartItem::unit_price` is: the two
    /// structs describe one wire shape, and a price the server omitted must
    /// arrive as an absence on both roads into `CartItem::from_server` (D9).
    ///
    /// Outbound it is skipped when absent rather than sent as a `0`. The
    /// server ignores this field on merge -- `collapse_merge_items`
    /// (`src/api/cart.rs:626-634`) keeps only kind, catalog_id and quantity and
    /// re-prices every line from the catalog -- so omitting it loses nothing
    /// the server reads, while a fabricated `0` would be a number this client
    /// never measured, written into a price-authoritative request. The server's
    /// own request type requires the field, so such a payload is REFUSED rather
    /// than merged: a visible failure where there is no honest number to send.
    /// No reorder path can produce one -- `reorder_item_to_cart_item` drops a
    /// line it cannot price before it becomes a `CartItem` at all.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit_price: Option<f64>,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_url: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CartRespDto {
    pub telegram_id: i64,
    pub items: Vec<MergeCartItemDto>,
    pub total: f64,
    pub updated_at: Option<String>,
}

impl From<MergeCartItemDto> for ServerCartItem {
    /// The two structs describe one wire shape field-for-field; only their
    /// serde defaults differ. This exists so the merge response and the cart
    /// GET reach [`CartItem::from_server`] by the same road.
    fn from(dto: MergeCartItemDto) -> Self {
        Self {
            id: dto.id,
            kind: dto.kind,
            catalog_id: dto.catalog_id,
            quantity: dto.quantity,
            unit_price: dto.unit_price,
            name: dto.name,
            image_url: dto.image_url,
        }
    }
}

impl From<CartRespDto> for Cart {
    /// Routed through [`CartItem::from_server`] rather than rebuilt here.
    ///
    /// The open-coded version this replaces disagreed with `from_server` on
    /// every line it shared with it. An unrecognised `kind` became a
    /// `CartItemType::Strain` instead of being dropped — so the day the server
    /// speaks a kind this bundle predates, the merge returns it labelled a
    /// cannabis strain and `cart_item_type_to_kind` posts it straight back as
    /// `"strain"` to a *price-authoritative* endpoint. A zero quantity
    /// survived, and a non-finite `unit_price` went into `recalculate_total`
    /// untouched, making the whole cart total NaN.
    ///
    /// `filter_map` is the point: a line this bundle cannot read is dropped,
    /// which is what `from_server` has always done. The merge response is
    /// already documented as possibly smaller than the input.
    fn from(resp: CartRespDto) -> Self {
        let items = resp
            .items
            .into_iter()
            .filter_map(|i| CartItem::from_server(i.into()))
            .collect();
        // `total` is overwritten on the next line; `None` is the honest
        // placeholder for a figure that has not been computed yet, where a
        // `0.0` would be a cart momentarily claiming to be free.
        let mut cart = Cart { items, total: None };
        cart.recalculate_total();
        cart
    }
}

/// Convert a UI [`CartItemType`] into the string kind expected by the backend
/// cart API.
pub fn cart_item_type_to_kind(item_type: &CartItemType) -> &'static str {
    match item_type {
        CartItemType::Strain => "strain",
        CartItemType::Accessory => "accessory",
        CartItemType::Tea => "tea",
        CartItemType::Set => "set",
    }
}

/// Merge local items into the server-side cart and return a price-authoritative
/// [`Cart`]. Unknown/unavailable items are skipped by the server rather than
/// failing the whole merge, so the returned cart may be smaller than the input.
pub async fn merge_server_cart(
    base_url: &str,
    init_data: &str,
    telegram_id: i64,
    items: &[CartItem],
) -> Result<Cart, String> {
    let merge_items: Vec<MergeCartItemDto> = items
        .iter()
        .map(|i| MergeCartItemDto {
            id: i.id.clone(),
            kind: cart_item_type_to_kind(&i.item_type).to_string(),
            catalog_id: i.id.clone(),
            quantity: i.quantity as i32,
            unit_price: i.price,
            name: i.name.clone(),
            image_url: i.image_url.clone(),
        })
        .collect();
    let body = serde_json::json!({
        "telegram_id": telegram_id,
        "items": merge_items,
    })
    .to_string();
    let url = format!("{}/api/cart/merge", base_url);
    let text = post_json_authed(&url, init_data, &body).await?;
    let resp: CartRespDto =
        serde_json::from_str(&text).map_err(|e| format!("Cart merge response JSON error: {e}"))?;
    Ok(resp.into())
}

/// Loop #13: fire a lightweight client event to the backend. Used for
/// conversion attribution (e.g. cart deep-link opened) where the event does not
/// fit the error telemetry shape. Returns the HTTP status; failures are ignored
/// on the client side so analytics can never block the user flow.
pub async fn post_client_event(base_url: &str, event: &str, detail: &str) -> Result<u16, String> {
    let body = serde_json::json!({
        "event": event,
        "detail": detail,
    })
    .to_string();
    let url = format!("{}/api/client-events", base_url);
    let resp = Request::post(&url)
        .header("content-type", "application/json")
        .body(body)
        .map_err(|e| format!("Build error: {e}"))?
        .send()
        .await
        .map_err(|e| format!("Network error: {e}"))?;
    Ok(resp.status())
}

/// PUT JSON with full admin auth. Returns (status, body).
#[allow(dead_code)]
pub async fn put_json_admin_full(
    url: &str,
    auth: &AdminAuth<'_>,
    body: &str,
) -> Result<(u16, String), String> {
    let req = auth
        .apply(Request::put(url).header("content-type", "application/json"))
        .body(body.to_string())
        .map_err(|e| format!("Build error: {e}"))?;
    let resp = req
        .send()
        .await
        .map_err(|e| format!("Network error: {e}"))?;
    let status = resp.status();
    let body = resp.text().await.map_err(|e| format!("Read error: {e}"))?;
    Ok((status, body))
}

/// POST JSON with full admin auth. Returns (status, body).
#[allow(dead_code)]
pub async fn post_json_admin_full(
    url: &str,
    auth: &AdminAuth<'_>,
    body: &str,
) -> Result<(u16, String), String> {
    let req = auth
        .apply(Request::post(url).header("content-type", "application/json"))
        .body(body.to_string())
        .map_err(|e| format!("Build error: {e}"))?;
    let resp = req
        .send()
        .await
        .map_err(|e| format!("Network error: {e}"))?;
    let status = resp.status();
    let body = resp.text().await.map_err(|e| format!("Read error: {e}"))?;
    Ok((status, body))
}

/// DELETE with full admin auth. Returns (status, body).
#[allow(dead_code)]
pub async fn delete_admin_full(url: &str, auth: &AdminAuth<'_>) -> Result<(u16, String), String> {
    let req = auth
        .apply(Request::delete(url))
        .build()
        .map_err(|e| format!("Build error: {e}"))?;
    let resp = req
        .send()
        .await
        .map_err(|e| format!("Network error: {e}"))?;
    let status = resp.status();
    let body = resp.text().await.map_err(|e| format!("Read error: {e}"))?;
    Ok((status, body))
}
