//! Request correlation across handlers, downstream calls, and log lines.
//!
//! Pattern (per tower-http / tracing community best practice):
//! - Extract `X-Request-ID` from the incoming request if a proxy / Cloudflare
//!   already attached one. Otherwise mint a UUID v4 so every request still
//!   gets a stable id.
//! - Stash it in `Request::extensions` so handlers can pull it out via the
//!   `RequestId` type when they need to log application-specific fields.
//! - Wrap the rest of the request in an `info_span!` whose `request_id` field
//!   is automatically attached to every `tracing` event the handler emits —
//!   no manual passing required.
//! - Echo the id back on the response so clients (and Telegram-side logs)
//!   can correlate failures with what the server saw.

use axum::{
    body::Body,
    extract::Request,
    http::{HeaderName, HeaderValue},
    middleware::Next,
    response::Response,
};
use tracing::Instrument;

/// Stable request correlation id. Pulled from incoming `X-Request-ID` when
/// present (e.g. set by Cloudflare / Railway / a calling service), else minted
/// fresh as a UUID v4. Handlers can read it via
/// `req.extensions().get::<RequestId>()` to enrich their own logs.
#[derive(Clone, Debug)]
#[allow(dead_code)] // public field is part of the extension contract
pub struct RequestId(pub String);

const REQUEST_ID_HEADER: HeaderName = HeaderName::from_static("x-request-id");
/// Cap: a request_id longer than this is almost certainly garbage / hostile.
/// UUIDs are 36 chars, so 128 leaves room for friendly external IDs.
const MAX_INBOUND_REQUEST_ID_LEN: usize = 128;

/// Parse an incoming request id from headers, returning `None` if it's
/// missing, empty, oversized, or contains invalid characters. We only accept
/// ASCII-printable bytes minus delimiters that would break header re-emission.
fn parse_inbound_request_id(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.len() > MAX_INBOUND_REQUEST_ID_LEN {
        return None;
    }
    if !trimmed.bytes().all(|b| b.is_ascii_graphic()) {
        return None;
    }
    Some(trimmed.to_string())
}

/// Extract or mint a request id for this incoming request.
fn extract_or_generate(headers: &axum::http::HeaderMap) -> String {
    headers
        .get(&REQUEST_ID_HEADER)
        .and_then(|v| v.to_str().ok())
        .and_then(parse_inbound_request_id)
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string())
}

/// Axum middleware: attach a request id to every request and surface it on
/// the response. Wrap the downstream `next.run(req)` in an `info_span!` so
/// every log inside the handler chain inherits the id.
pub async fn request_id_middleware(mut req: Request<Body>, next: Next) -> Response {
    let request_id = extract_or_generate(req.headers());
    req.extensions_mut().insert(RequestId(request_id.clone()));

    let method = req.method().clone();
    let path = req.uri().path().to_string();

    let span = tracing::info_span!(
        "http_request",
        request_id = %request_id,
        method = %method,
        path = %path,
    );

    let mut response = next.run(req).instrument(span).await;

    if let Ok(hv) = HeaderValue::from_str(&request_id) {
        response.headers_mut().insert(REQUEST_ID_HEADER, hv);
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderMap;

    #[test]
    fn parse_accepts_uuid_like() {
        assert_eq!(
            parse_inbound_request_id("a1b2c3d4-e5f6-7890-abcd-ef0123456789").unwrap(),
            "a1b2c3d4-e5f6-7890-abcd-ef0123456789"
        );
    }

    #[test]
    fn parse_accepts_short_ascii_token() {
        assert_eq!(parse_inbound_request_id("trace-42").unwrap(), "trace-42");
    }

    #[test]
    fn parse_trims_whitespace() {
        assert_eq!(parse_inbound_request_id("  trace-42  ").unwrap(), "trace-42");
    }

    #[test]
    fn parse_rejects_empty() {
        assert!(parse_inbound_request_id("").is_none());
        assert!(parse_inbound_request_id("    ").is_none());
    }

    #[test]
    fn parse_rejects_oversized() {
        let huge = "a".repeat(200);
        assert!(parse_inbound_request_id(&huge).is_none());
    }

    #[test]
    fn parse_rejects_control_chars() {
        // \n, \t, space all fail the ascii_graphic check (after trim).
        assert!(parse_inbound_request_id("with\nnewline").is_none());
        assert!(parse_inbound_request_id("with\ttab").is_none());
        assert!(parse_inbound_request_id("with space").is_none());
    }

    #[test]
    fn parse_rejects_non_ascii() {
        assert!(parse_inbound_request_id("héllo").is_none());
        assert!(parse_inbound_request_id("trace-😀").is_none());
    }

    #[test]
    fn extract_uses_incoming_header_when_valid() {
        let mut h = HeaderMap::new();
        h.insert(REQUEST_ID_HEADER, "client-12345".parse().unwrap());
        assert_eq!(extract_or_generate(&h), "client-12345");
    }

    #[test]
    fn extract_generates_uuid_when_missing() {
        let h = HeaderMap::new();
        let id = extract_or_generate(&h);
        // UUID v4 string is 36 chars.
        assert_eq!(id.len(), 36);
        assert!(uuid::Uuid::parse_str(&id).is_ok());
    }

    #[test]
    fn extract_generates_uuid_when_inbound_is_garbage() {
        let mut h = HeaderMap::new();
        let huge = "a".repeat(200);
        h.insert(REQUEST_ID_HEADER, huge.parse().unwrap());
        let id = extract_or_generate(&h);
        // Should fall through to UUID since inbound was rejected.
        assert!(uuid::Uuid::parse_str(&id).is_ok());
    }
}
