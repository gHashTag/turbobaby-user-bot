//! LINE Messaging API integration for retention broadcasts.
//!
//! Only the broadcast endpoint is wired: a single text message is sent to
//! every user who has added the LINE Official Account as a friend. This keeps
//! the integration stateless — we do not need to store LINE user IDs.

use serde_json::json;

const LINE_BROADCAST_URL: &str = "https://api.line.me/v2/bot/message/broadcast";
const MAX_BROADCAST_CHARS: usize = 5000;

/// Broadcast a plain text message to all friends of the LINE Official Account.
/// Returns the HTTP status and body text from LINE on completion.
///
/// Errors before the HTTP call (missing token, overlong text) return a
/// descriptive `Err(String)`; network / LINE-side failures are returned as
/// `Ok((status, body))` so the caller can decide whether to retry.
pub(crate) async fn broadcast_message(
    channel_access_token: &str,
    text: &str,
) -> Result<(reqwest::StatusCode, String), String> {
    if text.is_empty() {
        return Err("broadcast text is empty".into());
    }
    if text.len() > MAX_BROADCAST_CHARS {
        return Err(format!(
            "broadcast text too long: {} > {} characters",
            text.len(),
            MAX_BROADCAST_CHARS
        ));
    }
    if channel_access_token.trim().is_empty() {
        return Err("LINE channel access token is empty".into());
    }

    let body = json!({
        "messages": [
            { "type": "text", "text": text }
        ]
    });

    let client = reqwest::Client::new();
    let response = client
        .post(LINE_BROADCAST_URL)
        .header("Authorization", format!("Bearer {channel_access_token}"))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("LINE broadcast request failed: {e}"))?;

    let status = response.status();
    let body_text = response
        .text()
        .await
        .unwrap_or_default();
    Ok((status, body_text))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn broadcast_rejects_empty_text() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let err = rt
            .block_on(broadcast_message("token", ""))
            .unwrap_err();
        assert!(err.contains("empty"));
    }

    #[test]
    fn broadcast_rejects_overlong_text() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let text = "x".repeat(MAX_BROADCAST_CHARS + 1);
        let err = rt
            .block_on(broadcast_message("token", &text))
            .unwrap_err();
        assert!(err.contains("too long"));
    }

    #[test]
    fn broadcast_rejects_empty_token() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let err = rt
            .block_on(broadcast_message("", "hello"))
            .unwrap_err();
        assert!(err.contains("token is empty"));
    }
}
