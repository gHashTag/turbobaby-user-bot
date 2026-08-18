//! Can this crate's HTTP client reach an `https://` origin at all?
//!
//! `POST /api/share/prepare` returned 502 in production, in 12ms, with the
//! service healthy and every other request answering 200. The server log said
//! why:
//!
//! ```text
//! savePreparedInlineMessage transport error: reqwest::Error {
//!   kind: Request,
//!   url: https://api.telegram.org/bot…/savePreparedInlineMessage,
//!   source: hyper::Error(Connect, "invalid URL, scheme is not http") }
//! ```
//!
//! That message is the literal signature of hyper's plain-HTTP connector being
//! handed an `https://` URL. `Cargo.toml` had
//! `reqwest = { default-features = false, features = ["json"] }` — no TLS
//! backend — so **every** HTTPS request this crate made was impossible, and had
//! been since the dependency was declared.
//!
//! Nothing caught it because nothing could. Every unit test mocks or avoids the
//! network, and the one other caller (`src/ai.rs`) is disabled in production for
//! want of an API key, so it never failed loudly either.
//!
//! So the test is a real request. It asserts the distinction that matters and
//! that no mock can express: a **transport** failure versus an **HTTP**
//! failure. Reaching Telegram and being told "unauthorized" is success here —
//! it proves the bytes arrived. Not reaching it at all is the defect.
//!
//! Run with:
//! ```sh
//! cargo test --features backend --test https_reaches_telegram -- --ignored
//! ```

#![cfg(feature = "backend")]

/// The exact host `src/api/share.rs` posts to, with a deliberately invalid
/// token: Telegram answers 401 without any side effect on the real bot.
const PROBE: &str = "https://api.telegram.org/bot0:invalid/getMe";

#[tokio::test]
#[ignore = "makes a real network request"]
async fn the_client_this_crate_builds_can_speak_https() {
    let outcome = reqwest::Client::new().get(PROBE).send().await;

    match outcome {
        Ok(resp) => {
            // Any status at all means the connection was made, the TLS
            // handshake completed and Telegram replied. That is the whole
            // claim; the status itself is Telegram's business.
            assert!(
                resp.status().is_client_error() || resp.status().is_success(),
                "reached Telegram but it answered {} — unexpected, though the \
                 transport itself is fine",
                resp.status()
            );
        }
        Err(e) if e.is_connect() => {
            panic!(
                "the client cannot connect to an https:// URL at all: {e:?}\n\n\
                 If this says `invalid URL, scheme is not http`, reqwest has no \
                 TLS backend — check that `rustls-tls` is still in its feature \
                 list in Cargo.toml. Every outbound HTTPS call in this crate is \
                 broken when it is missing, and `POST /api/share/prepare` \
                 answers 502 for every customer who taps share."
            );
        }
        Err(e) if e.is_timeout() => {
            // A slow or blocked network is not this crate's defect, and must
            // not be reported as one.
            eprintln!("network timed out — inconclusive, not a failure: {e}");
        }
        Err(e) => panic!("request failed for a reason worth reading: {e:?}"),
    }
}

/// The same probe through the client `src/ai.rs` builds.
///
/// It is the other caller of this dependency, it is disabled in production for
/// want of an API key, and it would have failed exactly the same way the first
/// time somebody set one. Turning a key on should not be how that is found out.
#[tokio::test]
#[ignore = "makes a real network request"]
async fn the_ai_clients_https_works_too() {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .expect("build a client the way src/ai.rs does");

    match client.get(PROBE).send().await {
        Ok(_) => {}
        Err(e) if e.is_timeout() => eprintln!("network timed out — inconclusive: {e}"),
        Err(e) if e.is_connect() => panic!(
            "a built client cannot speak https either: {e:?} — see the note in \
             Cargo.toml beside the reqwest dependency"
        ),
        Err(e) => panic!("request failed: {e:?}"),
    }
}
