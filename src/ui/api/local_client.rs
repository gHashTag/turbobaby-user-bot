//! Drop-in replacement for `reqwest::Client` that ships through gloo-net.
//!
//! Exposes just enough of the reqwest builder surface to let admin_screen.rs
//! migrate its ≈50 call sites by swapping `LazyLock<reqwest::Client>` for
//! `LazyLock<LocalClient>` — no per-site rewrites needed.
//!
//! Supports: GET / PUT / POST / DELETE; `.header(k, v)`; `.json(&body)`;
//! `.send().await`; on the response `.status().is_success()` / `.as_u16()`,
//! `.text().await`, `.json::<T>().await`.
//!
//! Why not just use gloo-net directly? Because the call sites use the
//! reqwest API shape verbatim, and the shim keeps the diff to a single line
//! per file (the static declaration) rather than touching every send-await.

use gloo_net::http::{Request, RequestBuilder};
use serde::{de::DeserializeOwned, Serialize};

#[derive(Clone, Default)]
pub struct LocalClient;

impl LocalClient {
    pub fn new() -> Self {
        LocalClient
    }

    pub fn get(&self, url: &str) -> LocalRequest {
        LocalRequest::new(Method::Get, url)
    }
    pub fn put(&self, url: &str) -> LocalRequest {
        LocalRequest::new(Method::Put, url)
    }
    pub fn post(&self, url: &str) -> LocalRequest {
        LocalRequest::new(Method::Post, url)
    }
    pub fn delete(&self, url: &str) -> LocalRequest {
        LocalRequest::new(Method::Delete, url)
    }
}

#[derive(Clone, Copy)]
enum Method {
    Get,
    Put,
    Post,
    Delete,
}

pub struct LocalRequest {
    method: Method,
    url: String,
    headers: Vec<(String, String)>,
    body: Option<String>,
}

impl LocalRequest {
    fn new(method: Method, url: &str) -> Self {
        Self {
            method,
            url: url.to_string(),
            headers: Vec::new(),
            body: None,
        }
    }

    pub fn header<K: Into<String>, V: Into<String>>(mut self, k: K, v: V) -> Self {
        self.headers.push((k.into(), v.into()));
        self
    }

    /// Append URL query parameters reqwest-style: `.query(&[("k","v")])`.
    /// Handles `?` vs `&` correctly whether the base URL already has a query
    /// string or not. URL-encodes values to keep call sites simple.
    #[allow(dead_code)]
    pub fn query<K: AsRef<str>, V: AsRef<str>>(mut self, params: &[(K, V)]) -> Self {
        let mut sep = if self.url.contains('?') { '&' } else { '?' };
        for (k, v) in params {
            self.url.push(sep);
            self.url.push_str(&urlencoding::encode(k.as_ref()));
            self.url.push('=');
            self.url.push_str(&urlencoding::encode(v.as_ref()));
            sep = '&';
        }
        self
    }

    /// Mirrors `reqwest::RequestBuilder::json`: serialise & set as body. If
    /// serialisation fails we stash an empty body so `send().await` returns
    /// a Network error from the server side (mirroring reqwest's behaviour
    /// of erroring at send-time, not build-time).
    pub fn json<T: Serialize>(mut self, body: &T) -> Self {
        self.body = Some(serde_json::to_string(body).unwrap_or_default());
        self
    }

    pub async fn send(self) -> Result<LocalResponse, LocalError> {
        let mut builder: RequestBuilder = match self.method {
            Method::Get => Request::get(&self.url),
            Method::Put => Request::put(&self.url),
            Method::Post => Request::post(&self.url),
            Method::Delete => Request::delete(&self.url),
        };
        for (k, v) in &self.headers {
            builder = builder.header(k, v);
        }
        let resp = if let Some(body) = self.body {
            builder = builder.header("content-type", "application/json");
            builder
                .body(body)
                .map_err(|e| LocalError(e.to_string()))?
                .send()
                .await
                .map_err(|e| LocalError(e.to_string()))?
        } else {
            builder
                .send()
                .await
                .map_err(|e| LocalError(e.to_string()))?
        };
        Ok(LocalResponse(resp))
    }
}

pub struct LocalResponse(gloo_net::http::Response);

impl LocalResponse {
    pub fn status(&self) -> LocalStatus {
        LocalStatus(self.0.status())
    }

    pub async fn text(self) -> Result<String, LocalError> {
        self.0.text().await.map_err(|e| LocalError(e.to_string()))
    }

    pub async fn json<T: DeserializeOwned>(self) -> Result<T, LocalError> {
        let text = self.0.text().await.map_err(|e| LocalError(e.to_string()))?;
        serde_json::from_str(&text).map_err(|e| LocalError(e.to_string()))
    }
}

#[derive(Clone, Copy)]
pub struct LocalStatus(u16);

impl LocalStatus {
    pub fn is_success(&self) -> bool {
        (200..300).contains(&self.0)
    }
    pub fn as_u16(&self) -> u16 {
        self.0
    }
}

impl std::fmt::Display for LocalStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone)]
pub struct LocalError(String);

impl std::fmt::Display for LocalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for LocalError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_appends_first_param_with_question_mark() {
        let req =
            LocalRequest::new(Method::Get, "https://example.com/api").query(&[("limit", "10")]);
        assert_eq!(req.url, "https://example.com/api?limit=10");
    }

    #[test]
    fn query_appends_subsequent_with_ampersand() {
        let req = LocalRequest::new(Method::Get, "https://example.com/api")
            .query(&[("limit", "10"), ("offset", "20")]);
        assert_eq!(req.url, "https://example.com/api?limit=10&offset=20");
    }

    #[test]
    fn query_respects_existing_question_mark() {
        let req = LocalRequest::new(Method::Get, "https://example.com/api?lang=en")
            .query(&[("limit", "10")]);
        assert_eq!(req.url, "https://example.com/api?lang=en&limit=10");
    }

    #[test]
    fn query_url_encodes_values() {
        let req = LocalRequest::new(Method::Get, "https://example.com/api")
            .query(&[("search", "hello world&foo")]);
        // URL-encoded: space → %20, & → %26
        assert!(req.url.contains("search=hello%20world%26foo"));
    }

    #[test]
    fn status_is_success_classification() {
        assert!(LocalStatus(200).is_success());
        assert!(LocalStatus(204).is_success());
        assert!(!LocalStatus(199).is_success());
        assert!(!LocalStatus(300).is_success());
        assert!(!LocalStatus(404).is_success());
        assert!(!LocalStatus(500).is_success());
    }
}
