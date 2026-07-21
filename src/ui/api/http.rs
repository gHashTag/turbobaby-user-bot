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
        .send()
        .await
        .map_err(|e| format!("Network error: {e}"))?;
    let status = resp.status();
    let body = resp.text().await.map_err(|e| format!("Read error: {e}"))?;
    Ok((status, body))
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
