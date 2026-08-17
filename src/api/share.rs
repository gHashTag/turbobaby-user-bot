//! Rich product sharing into Telegram DMs (Bot API 8.0 prepared inline
//! messages).
//!
//! The old flow opened `t.me/share/url?url=<deep link>`, which sends a plain
//! link. Telegram then renders a preview of that link — and the link points at
//! the *bot*, so the recipient saw the bot's own profile card, never the
//! product. Photo, price and description could not be shown that way at all.
//!
//! This endpoint builds the real card server-side and hands the Mini App a
//! `prepared_message_id`. The client passes it to `WebApp.shareMessage()`,
//! Telegram sends a genuine photo message with the product's picture, name,
//! price and description, plus an "Open product" button carrying the
//! `?start=p_<kind>_<id>` deep link back into this exact card.
//!
//! Prices and text come from the database, never from the client: the caller
//! only says *which* product, so a shared card cannot advertise a price the
//! shop does not actually charge.

use axum::{extract::State, http::HeaderMap, http::StatusCode, routing::post, Json, Router};
use sea_orm::{ConnectionTrait, DbBackend, Statement};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::AppState;

pub(crate) fn routes() -> Router<AppState> {
    Router::new().route("/share/prepare", post(prepare_share))
}

#[derive(Debug, Deserialize)]
pub(crate) struct PrepareShareRequest {
    pub kind: String,
    pub id: String,
    /// Who the prepared message is for.
    ///
    /// Needed because `savePreparedInlineMessage` is scoped to one user, and
    /// because this endpoint now authenticates the way the other twenty-one do
    /// — `check_owner_lenient`, which takes the id it is asked to confirm.
    pub telegram_id: i64,
}

/// The product data a shared card is built from. Always DB-sourced.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ShareCard {
    pub title: String,
    pub description: String,
    pub price_baht: Option<f64>,
    pub image_url: Option<String>,
}

/// Kinds that can be shared. The wire names match the `p_<kind>_` deep-link
/// prefixes in `ui::share::ProductKind`, so a card and the link it carries can
/// never disagree about what was shared.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ShareKind {
    Strain,
    Accessory,
    Set,
    Tea,
    Event,
}

impl ShareKind {
    pub(crate) fn parse(raw: &str) -> Option<Self> {
        match raw {
            "strain" => Some(Self::Strain),
            "accessory" | "acc" => Some(Self::Accessory),
            "set" => Some(Self::Set),
            "tea" => Some(Self::Tea),
            "event" => Some(Self::Event),
            _ => None,
        }
    }

    /// Deep-link payload prefix — must mirror `ui::share::ProductKind`.
    pub(crate) fn payload_prefix(self) -> &'static str {
        match self {
            Self::Strain => "p_strain",
            Self::Accessory => "p_acc",
            Self::Set => "p_set",
            Self::Tea => "p_tea",
            Self::Event => "p_event",
        }
    }
}

/// Trim a description to something that reads as a caption rather than an
/// essay. Telegram caps captions at 1024 characters; we stay far below that so
/// the title and price never get pushed out.
pub(crate) fn short_description(raw: &str, max_chars: usize) -> String {
    let cleaned = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    if cleaned.chars().count() <= max_chars {
        return cleaned;
    }
    // Cut on a word boundary so the ellipsis doesn't land mid-word.
    let truncated: String = cleaned.chars().take(max_chars).collect();
    let cut = truncated.rfind(' ').unwrap_or(truncated.len());
    format!("{}…", truncated[..cut].trim_end())
}

/// Build the HTML caption shown under the product photo.
///
/// Escaped because product names and descriptions are owner-authored free
/// text — an unescaped `<` would break the message or, worse, be interpreted
/// as markup.
pub(crate) fn build_caption(card: &ShareCard) -> String {
    let mut out = format!("<b>{}</b>", crate::util::html_escape(&card.title));
    if let Some(price) = card.price_baht.filter(|p| p.is_finite() && *p > 0.0) {
        out.push_str(&format!("\n💸 {:.0} ฿", price));
    }
    let desc = short_description(&card.description, 180);
    if !desc.is_empty() {
        out.push_str(&format!("\n\n{}", crate::util::html_escape(&desc)));
    }
    out
}

/// Deep link that reopens this exact card in the Mini App.
///
/// `?start=` rather than `?startapp=`: the latter only launches the Mini App
/// when the bot has a Main Mini App configured in BotFather, otherwise
/// Telegram just opens the chat. See `ui::share`.
pub(crate) fn deep_link(bot_username: &str, kind: ShareKind, id: &str) -> String {
    format!(
        "https://t.me/{}?start={}_{}",
        bot_username,
        kind.payload_prefix(),
        id
    )
}

/// Assemble the `InlineQueryResult` handed to `savePreparedInlineMessage`.
///
/// A photo result when the product has a picture, an article otherwise — an
/// `InlineQueryResultPhoto` with a missing/broken `photo_url` is rejected by
/// Telegram, which would turn "no image set" into "sharing is broken".
pub(crate) fn build_inline_result(
    result_id: &str,
    card: &ShareCard,
    open_button_text: &str,
    deep_link: &str,
) -> Value {
    let markup = json!({
        "inline_keyboard": [[{ "text": open_button_text, "url": deep_link }]]
    });
    let caption = build_caption(card);

    match card
        .image_url
        .as_deref()
        .filter(|u| u.starts_with("https://"))
    {
        Some(url) => json!({
            "type": "photo",
            "id": result_id,
            "photo_url": url,
            "thumbnail_url": url,
            "title": card.title,
            "description": short_description(&card.description, 100),
            "caption": caption,
            "parse_mode": "HTML",
            "reply_markup": markup,
        }),
        None => json!({
            "type": "article",
            "id": result_id,
            "title": card.title,
            "description": short_description(&card.description, 100),
            "input_message_content": {
                "message_text": caption,
                "parse_mode": "HTML",
            },
            "reply_markup": markup,
        }),
    }
}

/// Load the card for a product straight from the database.
async fn load_card(
    state: &AppState,
    kind: ShareKind,
    id: &str,
) -> Result<Option<ShareCard>, sea_orm::DbErr> {
    // One statement per kind: the tables have genuinely different column
    // names, and aliasing them here keeps the row-reading code uniform.
    let sql = match kind {
        ShareKind::Strain => {
            "SELECT name AS title, COALESCE(description, '') AS description, \
             price_per_gram::float8 AS price, image_url FROM strains WHERE id = $1"
        }
        ShareKind::Accessory => {
            "SELECT name AS title, COALESCE(description, '') AS description, \
             price::float8 AS price, image_url FROM accessories WHERE id = $1"
        }
        ShareKind::Tea => {
            "SELECT name AS title, COALESCE(description, '') AS description, \
             price::float8 AS price, image_url FROM tea_products WHERE id = $1"
        }
        ShareKind::Set => {
            "SELECT name AS title, COALESCE(description, '') AS description, \
             total_price::float8 AS price, image_url FROM sets WHERE id = $1"
        }
        ShareKind::Event => {
            "SELECT title AS title, COALESCE(description, '') AS description, \
             price_baht::float8 AS price, image_url FROM events WHERE id = $1"
        }
    };
    let row = state
        .db
        .orm
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            sql,
            [id.into()],
        ))
        .await?;
    let Some(row) = row else {
        return Ok(None);
    };
    Ok(Some(ShareCard {
        title: row.try_get::<String>("", "title").unwrap_or_default(),
        description: row.try_get::<String>("", "description").unwrap_or_default(),
        price_baht: row.try_get::<Option<f64>>("", "price").unwrap_or(None),
        image_url: row
            .try_get::<Option<String>>("", "image_url")
            .unwrap_or(None)
            .filter(|u| !u.is_empty()),
    }))
}

/// `POST /api/share/prepare` — returns a `prepared_message_id` the Mini App
/// feeds to `WebApp.shareMessage()`.
async fn prepare_share(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<PrepareShareRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    // This endpoint used to be the only one in the API on strict
    // `validate_init_data`, and that is why sharing a product has never once
    // produced a product card in production.
    //
    // Every request that reaches this server logs `kind=signed-but-invalid`: the
    // HMAC fails for every real client, and the other twenty-one authenticated
    // endpoints only work because `check_owner_lenient` accepts them anyway.
    // Here there was no fallback, so the call answered 401, the Mini App fell
    // through to `share_product_link_only`, and the recipient got a bare
    // `t.me/<bot>?start=…` whose preview is the bot's own profile card — which
    // is exactly what customers have been sending each other.
    //
    // Using the same authentication as the rest of the API does not widen the
    // hole it inherits. Telegram scopes a `prepared_message_id` to the
    // `user_id` it was created for, so a forged id yields an identifier only
    // that user's client can send; what an attacker gains is the ability to make
    // the bot prepare cards nobody can use. The strict check here was buying
    // nothing and costing the feature.
    //
    // The real fix is the HMAC itself, and it is not this change: see
    // `validate_init_data_debug` and the `/api/debug` handler built for it.
    crate::api::auth::validate_telegram_id_param(req.telegram_id)
        .map_err(|status| (status, Json(json!({ "error": "invalid telegram_id" }))))?;
    let user_id = crate::api::auth::check_owner_lenient(&headers, &state, req.telegram_id, "share")
        .map_err(|status| (status, Json(json!({ "error": "unauthorized" }))))?;

    if req.id.is_empty() || req.id.len() > 200 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "invalid id" })),
        ));
    }
    let kind = ShareKind::parse(&req.kind).ok_or((
        StatusCode::BAD_REQUEST,
        Json(json!({ "error": "invalid kind" })),
    ))?;

    let card = load_card(&state, kind, &req.id)
        .await
        .map_err(|e| {
            tracing::error!("share prepare lookup failed kind={:?}: {:?}", kind, e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "lookup failed" })),
            )
        })?
        .ok_or((
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "product not found" })),
        ))?;

    let link = deep_link(&state.config.bot_username, kind, &req.id);
    let result = build_inline_result(
        &uuid::Uuid::new_v4().to_string(),
        &card,
        // The recipient's client language is unknown here, so the button
        // carries both words rather than guessing wrong.
        "🛒 Открыть товар / Open",
        &link,
    );

    let api_url = format!(
        "https://api.telegram.org/bot{}/savePreparedInlineMessage",
        state.config.bot_token
    );
    let payload = json!({
        "user_id": user_id,
        "result": result,
        "allow_user_chats": true,
        "allow_group_chats": true,
        "allow_channel_chats": true,
    });

    let response = reqwest::Client::new()
        .post(&api_url)
        .json(&payload)
        .send()
        .await
        .map_err(|e| {
            tracing::error!("savePreparedInlineMessage transport error: {:?}", e);
            (
                StatusCode::BAD_GATEWAY,
                Json(json!({ "error": "telegram unreachable" })),
            )
        })?;

    let body: Value = response.json().await.map_err(|e| {
        tracing::error!("savePreparedInlineMessage decode error: {:?}", e);
        (
            StatusCode::BAD_GATEWAY,
            Json(json!({ "error": "telegram bad response" })),
        )
    })?;

    let prepared_id = extract_prepared_message_id(&body).ok_or_else(|| {
        // Telegram's own description is the only thing that explains a refusal
        // (unsupported client, bad photo URL, …), so keep it in the log.
        tracing::error!("savePreparedInlineMessage refused: {}", body);
        (
            StatusCode::BAD_GATEWAY,
            Json(json!({ "error": "telegram refused" })),
        )
    })?;

    Ok(Json(json!({ "prepared_message_id": prepared_id })))
}

/// Pull the id out of Telegram's `{"ok":true,"result":{"id":…}}` envelope.
pub(crate) fn extract_prepared_message_id(body: &Value) -> Option<String> {
    if body.get("ok").and_then(|v| v.as_bool()) != Some(true) {
        return None;
    }
    body.get("result")
        .and_then(|r| r.get("id"))
        .and_then(|id| id.as_str())
        .map(|s| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card() -> ShareCard {
        ShareCard {
            title: "KING JUICE".into(),
            description: "King Juice is a premium sativa-dominant hybrid.".into(),
            price_baht: Some(350.0),
            image_url: Some("https://cdn.example/king.png".into()),
        }
    }

    #[test]
    fn kind_wire_names_match_deep_link_prefixes() {
        // A card whose button prefix disagreed with the parser would open the
        // Mini App on the wrong catalog — or on nothing at all.
        for (wire, prefix) in [
            ("strain", "p_strain"),
            ("accessory", "p_acc"),
            ("set", "p_set"),
            ("tea", "p_tea"),
            ("event", "p_event"),
        ] {
            let kind = ShareKind::parse(wire).expect("known kind");
            assert_eq!(kind.payload_prefix(), prefix, "prefix drift for {wire}");
        }
    }

    #[test]
    fn kind_parse_rejects_unknown() {
        assert!(ShareKind::parse("").is_none());
        assert!(ShareKind::parse("garden").is_none());
    }

    #[test]
    fn deep_link_is_a_start_link_not_startapp() {
        // `?startapp=` silently opens the bot chat unless a Main Mini App is
        // configured in BotFather — that is the bug this replaced.
        let link = deep_link("Woody_WeedPecker_bot", ShareKind::Set, "abc-123");
        assert_eq!(
            link,
            "https://t.me/Woody_WeedPecker_bot?start=p_set_abc-123"
        );
        assert!(!link.contains("startapp"));
    }

    #[test]
    fn caption_carries_name_price_and_description() {
        let caption = build_caption(&card());
        assert!(caption.contains("KING JUICE"), "name missing: {caption}");
        assert!(caption.contains("350"), "price missing: {caption}");
        assert!(
            caption.contains("premium sativa-dominant"),
            "description missing: {caption}"
        );
    }

    #[test]
    fn caption_escapes_owner_authored_markup() {
        let mut c = card();
        c.title = "<b>hack</b>".into();
        c.description = "5 < 6 & 7 > 2".into();
        let caption = build_caption(&c);
        assert!(
            !caption.contains("<b>hack</b>"),
            "raw markup survived escaping: {caption}"
        );
        assert!(caption.contains("&lt;"), "expected escaped text: {caption}");
    }

    #[test]
    fn caption_omits_missing_or_nonsensical_price() {
        for price in [None, Some(0.0), Some(f64::NAN)] {
            let mut c = card();
            c.price_baht = price;
            let caption = build_caption(&c);
            assert!(
                !caption.contains('฿'),
                "price {price:?} should not render: {caption}"
            );
        }
    }

    #[test]
    fn short_description_cuts_on_a_word_boundary() {
        let out = short_description("alpha beta gamma delta", 12);
        assert!(out.ends_with('…'), "expected an ellipsis, got {out}");
        assert!(!out.contains("gam"), "cut mid-word: {out}");
    }

    #[test]
    fn short_description_collapses_whitespace_and_keeps_short_text() {
        assert_eq!(short_description("  a   b \n c ", 100), "a b c");
    }

    #[test]
    fn short_description_handles_multibyte_text() {
        // Byte-slicing Cyrillic would panic; this must not.
        let out = short_description(&"привет мир ".repeat(40), 50);
        assert!(out.chars().count() <= 51, "too long: {out}");
    }

    #[test]
    fn photo_result_used_when_an_https_image_exists() {
        let v = build_inline_result("r1", &card(), "Open", "https://t.me/b?start=p_strain_1");
        assert_eq!(v["type"], "photo");
        assert_eq!(v["photo_url"], "https://cdn.example/king.png");
        assert_eq!(v["parse_mode"], "HTML");
    }

    #[test]
    fn article_result_used_when_the_image_is_missing_or_insecure() {
        // An InlineQueryResultPhoto with no usable photo_url is refused by
        // Telegram, which would read to the owner as "sharing is broken".
        for image in [None, Some("http://insecure/x.png".to_string())] {
            let mut c = card();
            c.image_url = image.clone();
            let v = build_inline_result("r1", &c, "Open", "https://t.me/b?start=p_strain_1");
            assert_eq!(v["type"], "article", "image {image:?} should fall back");
            assert!(v["input_message_content"]["message_text"]
                .as_str()
                .unwrap_or_default()
                .contains("KING JUICE"));
        }
    }

    #[test]
    fn every_result_carries_the_open_button_with_the_deep_link() {
        let link = "https://t.me/b?start=p_tea_42";
        for image in [Some("https://cdn.example/a.png".to_string()), None] {
            let mut c = card();
            c.image_url = image;
            let v = build_inline_result("r1", &c, "Открыть", link);
            let button = &v["reply_markup"]["inline_keyboard"][0][0];
            assert_eq!(button["text"], "Открыть");
            assert_eq!(button["url"], link);
        }
    }

    #[test]
    fn prepared_message_id_is_read_from_the_ok_envelope() {
        let body = json!({ "ok": true, "result": { "id": "prep_123" } });
        assert_eq!(
            extract_prepared_message_id(&body).as_deref(),
            Some("prep_123")
        );
    }

    #[test]
    fn prepared_message_id_rejects_failures_and_malformed_bodies() {
        for body in [
            json!({ "ok": false, "description": "BUTTON_URL_INVALID" }),
            json!({ "ok": true, "result": {} }),
            json!({ "ok": true }),
            json!({}),
        ] {
            assert!(
                extract_prepared_message_id(&body).is_none(),
                "must not accept {body}"
            );
        }
    }
}
