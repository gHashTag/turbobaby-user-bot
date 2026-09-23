//! One bike family in full: specs, the price block, the published term ladder,
//! who confirms availability, and the buy-out block.
//!
//! # Money
//!
//! Not one number is formatted here. Every price, deposit, monthly rate and
//! sale price goes through [`money_thb`](super::catalog_screen::money_thb),
//! the single renderer in `catalog_screen.rs`, which returns an em dash for a
//! value the shop has not published (D9). There is no `unwrap_or(0.0)` and no
//! `unwrap_or_default()` in this file. The big number is present only when
//! [`client_day_rate`](super::catalog_screen::client_day_rate) accepts the
//! door's valid client rate; it never computes one from the pre-discount
//! tariff and class discount (D11). That tariff appears only beside a door
//! result, on a line that says it is pre-discount.
//!
//! # What the endpoint actually serves
//!
//! `GET /api/bikes/:key` answers `{ "bike": { …family…, …rollup… } }`. There
//! is no per-unit array and no per-family term ladder in it:
//!
//! - the units arrive **rolled up** — `units_total`, `units_available`,
//!   `colors`, `colors_available`, `model_years` — because `api::bikes`
//!   narrows a unit row to status, colour and model year at the boundary and
//!   publishes no `unit_code` and no `km_since_purchase` (D6, D14). So this
//!   screen shows the family's colours and years, and the customer books the
//!   family; the shop assigns the machine (D8). There is no unit picker,
//!   because there is nothing honest to put in it. Since 2026-09-24 it does
//!   not print *how many* are free either: the count is seeded and
//!   admin-edited, not a live check, so a manager confirms availability
//!   (`availability.t27` `FILE_MAY_CONFIRM = false`).
//! - the ladder comes from `GET /api/rental-terms`, one document for the whole
//!   shop, fetched here alongside the family. A ladder that fails to load
//!   degrades to no ladder section at all rather than to a table of dashes —
//!   the price block and the quote note stand on their own.
//!
//! `unit_status_counts` is parsed by the wire type and deliberately never
//! rendered: how many machines sit in service is shop state a customer cannot
//! act on, and D6 keeps service state out of the public catalog.
//!
//! # Booking
//!
//! `on_book` is optional on purpose. The cart cannot hold a rental line yet —
//! `CartItemType` has no `BikeRental` variant and a cart line's `unit_price`
//! is `NOT NULL CHECK (>= 0)`, so the only way to get a rental past that
//! column today is the `0.0` D9 forbids. Until the orchestrator wires it,
//! this screen renders the Book control **disabled with the reason named**
//! (`BookBlock::NotWired`) rather than a button that looks bookable and does
//! nothing. The same state machine covers the honest refusals: a family
//! that is closed (D12), one with no published rate (D11), and one with no
//! free unit.
//!
//! # What is deliberately absent (D6, D14, issue #9)
//!
//! No service or maintenance history, no per-unit revenue, no purchase cost,
//! no renter, no handle, no plate, no key code.
//!
//! # Standalone or overlay
//!
//! The screen takes its slug as a prop and refetches when it changes, and
//! falls back to `Route::Menu {}` when it has no `on_close`. So the catalog
//! can open it as an overlay today and a `#[route("/bikes/:slug")]` can mount
//! it unchanged tomorrow.

use crate::trios::core::Lang;
use crate::trios::i18n::{
    t, tf, Key, T_BACK, T_BIKE_ASK_MANAGER, T_BIKE_BOOK, T_BIKE_CC, T_BIKE_CLASS_DISCOUNT,
    T_BIKE_COLORS_ALL, T_BIKE_DEPOSIT, T_BIKE_MODEL_YEARS, T_BIKE_MONTHLY_LOW_SEASON,
    T_BIKE_NOT_OFFERED_ALTERNATIVES, T_BIKE_NOT_OFFERED_TITLE, T_BIKE_PER_DAY,
    T_BIKE_PRICE_ON_REQUEST, T_BIKE_PRICE_TITLE, T_BIKE_QUOTE_NOTE, T_BIKE_RATE_PER_DAY,
    T_BIKE_SALE_PRICE, T_BIKE_SALE_TITLE, T_BIKE_TARIFF_BEFORE_DISCOUNT, T_BIKE_TERMS_NOTE,
    T_BIKE_TERMS_TITLE, T_BIKE_TERM_DAYS, T_BIKE_TERM_DAYS_OPEN, T_BIKE_TERM_DISCOUNT_ONE,
    T_BIKE_TERM_DISCOUNT_RANGE, T_BIKE_TERM_MONTH, T_BIKE_TERM_TWO_WEEKS, T_BIKE_TERM_WEEK,
    T_BIKE_UNITS_EMPTY, T_BIKE_UNITS_TITLE,
};
use crate::ui::components::card_media::CardMedia;
use crate::ui::components::skeleton::{Skeleton, SkeletonShape};
use crate::ui::routes::Route;
use crate::ui::screens::catalog_screen::{
    availability_line, book_block, class_emoji, class_label, client_day_rate, description_for,
    discount_percent, display_name, fetch_bike_detail, fetch_rental_terms, finite_money, money_thb,
    offer_instead_labels, usable_image, ApiBike, ApiRentalTerm, BikeDetailPayload,
};
use dioxus::prelude::*;

/// What the screen hands its parent when a rental is actually bookable.
///
/// There is no unit in here, and that is the contract rather than a gap: D8
/// books a **family** (`bikes.key`) and the shop assigns the machine, and the
/// public API publishes no unit identifier for a customer to choose with.
#[derive(Debug, Clone, PartialEq)]
pub struct BikeBookingRequest {
    /// `bikes.key` — the family key a cart line carries (D8).
    pub family_key: String,
    /// The client per-day price shown on screen when the request was made.
    /// Always a published, finite, positive number: the Book control is only
    /// live when [`client_day_rate`](super::catalog_screen::client_day_rate)
    /// returned one.
    pub rate_thb_day: f64,
}

#[derive(Props, PartialEq, Clone)]
pub struct BikeDetailProps {
    /// `bikes.key`, e.g. `nmax-155`.
    pub slug: String,
    /// Called by the back control when the screen is an overlay. Without it
    /// the control navigates to the catalog route instead.
    #[props(default)]
    pub on_close: Option<EventHandler<()>>,
    /// Absent until a bike cart line exists — see the module docs.
    #[props(default)]
    pub on_book: Option<EventHandler<BikeBookingRequest>>,
}

#[component]
pub fn BikeDetail(props: BikeDetailProps) -> Element {
    let slug = props.slug.clone();
    let detail: Resource<Result<BikeDetailPayload, String>> =
        use_resource(use_reactive!(|slug| async move {
            fetch_bike_detail(&slug).await
        }));

    // The ladder is shop-wide, so it does not depend on the slug and is not
    // refetched when the slug changes.
    let terms: Resource<Result<Vec<ApiRentalTerm>, String>> =
        use_resource(|| async move { fetch_rental_terms().await });

    // A ladder that is still loading or failed to load is *no ladder*, not a
    // ladder of zeros: the section simply does not render. Written out rather
    // than `unwrap_or_default()` so the degradation is visible.
    let term_bands: Vec<ApiRentalTerm> = match &*terms.read() {
        Some(Ok(bands)) => bands.clone(),
        Some(Err(_)) | None => Vec::new(),
    };

    let lang = crate::ui::lang::current_lang();
    let on_close = props.on_close;
    let on_book = props.on_book;

    rsx! {
        div { style: "min-height:100vh;background:#0f0f1a;color:#e8e8e8;padding-bottom:calc(96px + env(safe-area-inset-bottom));",

            div { style: "padding:16px 16px 8px;",
                button {
                    style: "
                        font-size:14px;font-weight:700;
                        padding:8px 14px;background:transparent;color:#e8e8e8;
                        border:3px solid #2a2a4a;cursor:pointer;
                    ",
                    onclick: move |_| match on_close {
                        Some(handler) => handler.call(()),
                        // No parent to close to: this is a route mount.
                        None => {
                            let _ = navigator().push(Route::Menu {});
                        }
                    },
                    {t(lang, T_BACK)}
                }
            }

            {match &*detail.read() {
                Some(Ok(payload)) => render_detail(
                    payload.bike.clone(),
                    term_bands.clone(),
                    on_book,
                    lang,
                ),
                Some(Err(e)) => {
                    // Localised copy straight from friendly_response_error.
                    let err_msg = e.clone();
                    rsx! {
                        div { style: "text-align:center;padding:48px 16px;",
                            p { style: "font-size:20px;margin-bottom:12px;", "⚠️" }
                            p { style: "font-size:15px;color:#ff4757;", "{err_msg}" }
                        }
                    }
                }
                None => rsx! {
                    div { style: "padding:0 16px;display:flex;flex-direction:column;gap:12px;",
                        Skeleton { shape: SkeletonShape::Sotd }
                        Skeleton { shape: SkeletonShape::Title }
                        Skeleton { shape: SkeletonShape::Text }
                        Skeleton { shape: SkeletonShape::Text }
                        Skeleton { shape: SkeletonShape::Card }
                    }
                },
            }}
        }
    }
}

/// The loaded body. A free function so the `for`-loop over term bands contains
/// no hooks.
fn render_detail(
    bike: ApiBike,
    terms: Vec<ApiRentalTerm>,
    on_book: Option<EventHandler<BikeBookingRequest>>,
    lang: Lang,
) -> Element {
    let name = display_name(&bike);
    let emoji = class_emoji(&bike.class);
    let is_offered = bike.offered != Some(false);
    let description = description_for(&bike);

    let rate = client_day_rate(&bike);
    let rate_str = money_thb(rate);
    let tariff_before = finite_money(bike.base_rate_thb_day);
    // Worth a line only when it is a genuinely different number; otherwise it
    // reads as a second, competing price.
    let show_tariff_before = match (rate, tariff_before) {
        (Some(client), Some(published)) => (published - client).abs() >= 1.0,
        _ => false,
    };
    let discount_line = discount_percent(bike.class_discount)
        .map(|pct| tf(lang, T_BIKE_CLASS_DISCOUNT, &[pct.to_string()]));
    // The deposit, the monthly low-season figure and the class-discount
    // percentage are the file's *reference* money. Beside a door price they are
    // context; where a price is absent they become the price — ฿9,900 a month
    // divided by thirty is an averaged per-day number, which D11 forbids by
    // name. `show_tariff_before` above already applies exactly this gate via
    // its `_ => false` arm; this is the same rule, applied to its neighbours.
    let may_show_reference = crate::trios::pricing::may_publish_reference_money(
        bike.client_rate_thb_day,
        bike.client_rate_source.as_deref(),
    );

    let alternatives = offer_instead_labels(&bike);
    // Always "a manager confirms": the seeded count may not confirm (see
    // `availability_line`).
    let availability = availability_line(lang);
    // "None free" is a fact the API stated; it is not inferred from a missing
    // count. A zero may rule a family out (`FILE_MAY_RULE_OUT`), so this line
    // stays while the count itself is no longer printed.
    let none_free = bike.units_available == Some(0);
    // The colours on record across the family's units. Until 2026-09-24 the
    // row preferred the colours of the units seeded as free, labelled "free
    // colours" — the seeded count again, as a promise. Now it is the whole
    // family's list under the whole-family label, or no row when none was
    // served.
    let colors_row: Option<(Key, String)> =
        join_labels(&bike.colors).map(|all| (T_BIKE_COLORS_ALL, all));
    let years = join_labels(
        &bike
            .model_years
            .iter()
            .map(|year| year.to_string())
            .collect::<Vec<String>>(),
    );

    // D14: the sale block never derives a price from what the unit cost. A
    // family flagged for sale with no published price shows a dash and the
    // manager line — the enquiry action below is a real Telegram link, not a
    // button with nothing behind it (issue #11).
    let sale_price = finite_money(bike.sale_price_thb);
    let show_sale_block = bike.for_sale == Some(true) || sale_price.is_some();

    let block = book_block(&bike, on_book.is_some());
    let family_key = bike.key.clone();
    let manager = manager_link();

    rsx! {
        div { style: "padding:0 16px;",

            div { style: "border:4px solid #2a2a4a;box-shadow:4px 4px 0 #000;background:#16213e;overflow:hidden;",
                CardMedia {
                    image_url: usable_image(bike.image_url.as_deref()),
                    video_url: None,
                    emoji: emoji.to_string(),
                    alt: name.clone(),
                    aspect_ratio: Some("4/3".to_string()),
                    {bike.variant_label.clone().map(|label| rsx! {
                        span { style: "
                            position:absolute;top:8px;left:8px;z-index:2;
                            font-size:13px;font-weight:700;background:#00e5ff;color:#000;
                            padding:4px 8px;box-shadow:2px 2px 0 #000;
                        ", "{label}" }
                    })}
                }
                div { style: "padding:14px;",
                    h1 { style: "font-size:22px;font-weight:800;color:#fff;text-shadow:3px 3px 0 #000;line-height:1.2;margin:0 0 8px;",
                        "{name}"
                    }
                    div { style: "display:flex;gap:8px;flex-wrap:wrap;align-items:center;",
                        span { style: "font-size:13px;color:#39ff14;border:2px solid #39ff14;padding:2px 8px;",
                            {class_label(&bike.class, lang)}
                        }
                        {bike.displacement_cc.map(|cc| rsx! {
                            span { style: "font-size:13px;color:#00e5ff;border:2px solid #00e5ff;padding:2px 8px;",
                                {tf(lang, T_BIKE_CC, &[cc.to_string()])}
                            }
                        })}
                        {bike.body.clone().map(|body| rsx! {
                            span { style: "font-size:13px;color:#aaa;border:2px solid #2a2a4a;padding:2px 8px;", "{body}" }
                        })}
                    }
                    {(!description.trim().is_empty()).then(|| rsx! {
                        p { style: "font-size:14px;color:#c8c8d8;line-height:1.5;margin:10px 0 0;", "{description}" }
                    })}
                }
            }

            {(!is_offered).then(|| rsx! {
                // D12.
                div { style: "margin-top:12px;border:4px solid #ff4757;background:rgba(255,71,87,0.08);padding:12px;",
                    div { style: "font-size:15px;font-weight:800;color:#ff4757;margin-bottom:6px;",
                        {t(lang, T_BIKE_NOT_OFFERED_TITLE)}
                    }
                    {(!alternatives.is_empty()).then(|| rsx! {
                        div { style: "font-size:14px;color:#e8e8e8;line-height:1.45;",
                            {tf(lang, T_BIKE_NOT_OFFERED_ALTERNATIVES, &[alternatives.join(" · ")])}
                        }
                    })}
                }
            })}

            // ── Price ────────────────────────────────────────────────────────
            {is_offered.then(|| rsx! {
                div { style: "margin-top:16px;",
                    h2 { style: section_title_style(), {t(lang, T_BIKE_PRICE_TITLE)} }
                    div { style: "border:4px solid #2a2a4a;background:#16213e;padding:12px;",
                        {if rate.is_some() {
                            rsx! {
                                div { style: "display:flex;gap:8px;align-items:baseline;flex-wrap:wrap;",
                                    span { style: "font-size:13px;color:#888;", {t(lang, T_BIKE_RATE_PER_DAY)} }
                                    span { style: "font-size:26px;font-weight:800;color:#ffe600;text-shadow:2px 2px 0 #000;", "{rate_str}" }
                                    span { style: "font-size:13px;color:#888;", {t(lang, T_BIKE_PER_DAY)} }
                                }
                            }
                        } else {
                            // D11: a human quotes it, and no number is emitted.
                            rsx! {
                                div { style: "font-size:15px;color:#888;font-style:italic;",
                                    {t(lang, T_BIKE_PRICE_ON_REQUEST)}
                                }
                            }
                        }}
                        {show_tariff_before.then(|| rsx! {
                            div { style: detail_row_style(),
                                {tf(lang, T_BIKE_TARIFF_BEFORE_DISCOUNT, &[money_thb(bike.base_rate_thb_day)])}
                            }
                        })}
                        {may_show_reference.then(|| rsx! {
                            {discount_line.clone().map(|line| rsx! {
                                div { style: "font-size:13px;color:#39ff14;margin-top:6px;", "{line}" }
                            })}
                            div { style: detail_row_style(),
                                {t(lang, T_BIKE_DEPOSIT)}
                                " "
                                {money_thb(bike.deposit_thb)}
                            }
                            div { style: detail_row_style(),
                                {t(lang, T_BIKE_MONTHLY_LOW_SEASON)}
                                " "
                                {money_thb(bike.monthly_low_season_thb)}
                            }
                        })}
                        // Carries no figure, and D11 forbids silence as firmly
                        // as invention — so this line stays outside the gate.
                        p { style: "font-size:13px;color:#8b8b9e;line-height:1.4;margin:10px 0 0;",
                            {t(lang, T_BIKE_QUOTE_NOTE)}
                        }
                    }
                }
            })}

            // ── Term ladder ──────────────────────────────────────────────────
            {(is_offered && !terms.is_empty()).then(|| {
                let rows = terms.clone();
                rsx! {
                    div { style: "margin-top:16px;",
                        h2 { style: section_title_style(), {t(lang, T_BIKE_TERMS_TITLE)} }
                        div { style: "border:4px solid #2a2a4a;background:#16213e;padding:4px 12px;",
                            {rows.into_iter().map(|term| {
                                let band = term.band.clone();
                                rsx! {
                                    div {
                                        key: "{band}",
                                        style: "display:flex;justify-content:space-between;gap:8px;align-items:baseline;padding:8px 0;border-bottom:2px solid #2a2a4a;",
                                        div {
                                            div { style: "font-size:14px;font-weight:700;color:#e8e8e8;", {band_label(&term.band, lang)} }
                                            {term_days_label(&term, lang).map(|days| rsx! {
                                                div { style: "font-size:13px;color:#888;", "{days}" }
                                            })}
                                        }
                                        div { style: "font-size:15px;font-weight:800;color:#39ff14;white-space:nowrap;",
                                            {term_discount_label(&term, lang)}
                                        }
                                    }
                                }
                            })}
                            p { style: "font-size:13px;color:#8b8b9e;line-height:1.4;margin:10px 0;",
                                {t(lang, T_BIKE_TERMS_NOTE)}
                            }
                        }
                    }
                }
            })}

            // ── Availability ─────────────────────────────────────────────────
            // The rollup, not a unit list: who confirms, colours, model years.
            // Nothing here identifies a machine (D14) or says a word about
            // service state (D6).
            {is_offered.then(|| rsx! {
                div { style: "margin-top:16px;",
                    h2 { style: section_title_style(), {t(lang, T_BIKE_UNITS_TITLE)} }
                    div { style: "border:4px solid #2a2a4a;background:#16213e;padding:12px;",
                        div { style: "font-size:15px;font-weight:700;color:#e8e8e8;", "{availability}" }
                        {none_free.then(|| rsx! {
                            div { style: "font-size:13px;color:#888;margin-top:6px;",
                                {t(lang, T_BIKE_UNITS_EMPTY)}
                            }
                        })}
                        {colors_row.clone().map(|(label, list)| rsx! {
                            div { style: detail_row_style(),
                                {t(lang, label)}
                                " "
                                "{list}"
                            }
                        })}
                        {years.clone().map(|list| rsx! {
                            div { style: detail_row_style(),
                                {t(lang, T_BIKE_MODEL_YEARS)}
                                " "
                                "{list}"
                            }
                        })}
                    }
                }
            })}

            // ── Buy-out ──────────────────────────────────────────────────────
            {show_sale_block.then(|| rsx! {
                div { style: "margin-top:16px;",
                    h2 { style: section_title_style(), {t(lang, T_BIKE_SALE_TITLE)} }
                    div { style: "border:4px solid #2a2a4a;background:#16213e;padding:12px;",
                        div { style: "display:flex;gap:8px;align-items:baseline;flex-wrap:wrap;",
                            span { style: "font-size:13px;color:#888;", {t(lang, T_BIKE_SALE_PRICE)} }
                            span { style: "font-size:20px;font-weight:800;color:#ffe600;text-shadow:2px 2px 0 #000;",
                                {money_thb(bike.sale_price_thb)}
                            }
                        }
                        {sale_price.is_none().then(|| rsx! {
                            div { style: "font-size:13px;color:#888;font-style:italic;margin-top:6px;",
                                {t(lang, T_BIKE_PRICE_ON_REQUEST)}
                            }
                        })}
                        // Rendered only when a real contact exists behind it.
                        {manager.clone().map(|url| rsx! {
                            button {
                                style: "
                                    margin-top:10px;width:100%;
                                    font-size:15px;font-weight:700;padding:12px;
                                    background:#00e5ff;color:#000;
                                    border:4px solid #000;box-shadow:3px 3px 0 #000;cursor:pointer;
                                ",
                                onclick: move |_| crate::ui::share::open_telegram_link(&url),
                                {t(lang, T_BIKE_ASK_MANAGER)}
                            }
                        })}
                    }
                }
            })}

            // ── Book ─────────────────────────────────────────────────────────
            div { style: "margin-top:20px;",
                {match block {
                    Some(reason) => rsx! {
                        button {
                            disabled: true,
                            style: "
                                width:100%;font-size:16px;font-weight:800;padding:14px;
                                background:#2a2a4a;color:#8b8b9e;
                                border:4px solid #2a2a4a;cursor:not-allowed;
                            ",
                            {t(lang, T_BIKE_BOOK)}
                        }
                        // The reason is never left implicit (issue #9).
                        p { style: "font-size:13px;color:#8b8b9e;line-height:1.4;margin:8px 0 0;",
                            {t(lang, reason.reason_key())}
                        }
                        {manager.clone().map(|url| rsx! {
                            button {
                                style: "
                                    margin-top:10px;width:100%;
                                    font-size:15px;font-weight:700;padding:12px;
                                    background:transparent;color:#00e5ff;
                                    border:4px solid #00e5ff;box-shadow:3px 3px 0 #000;cursor:pointer;
                                ",
                                onclick: move |_| crate::ui::share::open_telegram_link(&url),
                                {t(lang, T_BIKE_ASK_MANAGER)}
                            }
                        })}
                    },
                    None => rsx! {
                        button {
                            style: "
                                width:100%;font-size:16px;font-weight:800;padding:14px;
                                background:#39ff14;color:#000;
                                border:4px solid #000;box-shadow:4px 4px 0 #000;cursor:pointer;
                            ",
                            onclick: move |_| {
                                // Both are Some in this arm by construction:
                                // `book_block` returned None, which requires a
                                // published rate and a wired handler.
                                if let (Some(handler), Some(rate_value)) = (on_book, rate) {
                                    handler.call(BikeBookingRequest {
                                        family_key: family_key.clone(),
                                        rate_thb_day: rate_value,
                                    });
                                }
                            },
                            {t(lang, T_BIKE_BOOK)}
                        }
                    },
                }}
            }
        }
    }
}

/// `"красный · чёрный"`, or `None` when the list is empty or all blank.
///
/// `None` rather than an empty string on purpose: an empty label row would
/// render as a colour heading with nothing after it, which reads as "no
/// colours" rather than "not published".
fn join_labels(values: &[String]) -> Option<String> {
    let kept: Vec<String> = values
        .iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect();
    if kept.is_empty() {
        return None;
    }
    Some(kept.join(" · "))
}

fn section_title_style() -> String {
    "font-size:15px;font-weight:800;color:#00e5ff;text-transform:uppercase;letter-spacing:1px;text-shadow:2px 2px 0 #000;margin:0 0 8px;".to_string()
}

fn detail_row_style() -> String {
    "font-size:14px;color:#aaa;margin-top:6px;".to_string()
}

/// A band the API sent but this build does not know is shown raw rather than
/// mapped to the wrong ladder rung.
fn band_label(band: &str, lang: Lang) -> String {
    match band {
        "week" => t(lang, T_BIKE_TERM_WEEK).to_string(),
        "two_weeks" => t(lang, T_BIKE_TERM_TWO_WEEKS).to_string(),
        "month" => t(lang, T_BIKE_TERM_MONTH).to_string(),
        other => other.to_string(),
    }
}

/// `7–13` / `от 30`, or nothing at all when the band publishes no day range.
fn term_days_label(term: &ApiRentalTerm, lang: Lang) -> Option<String> {
    match (term.min_days, term.max_days) {
        (Some(min), Some(max)) => Some(tf(
            lang,
            T_BIKE_TERM_DAYS,
            &[min.to_string(), max.to_string()],
        )),
        (Some(min), None) => Some(tf(lang, T_BIKE_TERM_DAYS_OPEN, &[min.to_string()])),
        _ => None,
    }
}

/// The published discount **band**, not a multiplier: the exact number comes
/// from a manager (D11). A band with no published discount renders as a dash
/// rather than as 0%.
fn term_discount_label(term: &ApiRentalTerm, lang: Lang) -> String {
    let min = discount_percent(term.discount_min);
    let max = discount_percent(term.discount_max);
    match (min, max) {
        (Some(lo), Some(hi)) if lo != hi => tf(
            lang,
            T_BIKE_TERM_DISCOUNT_RANGE,
            &[lo.to_string(), hi.to_string()],
        ),
        (Some(v), _) | (None, Some(v)) => tf(lang, T_BIKE_TERM_DISCOUNT_ONE, &[v.to_string()]),
        (None, None) => crate::ui::screens::catalog_screen::MONEY_DASH.to_string(),
    }
}

/// The bot's own Telegram contact, or `None`.
///
/// `None` is the honest answer when the WebApp cannot tell us the bot's
/// username: issue #11 asks for an enquiry action with something real behind
/// it, so when there is no link there is no button.
fn manager_link() -> Option<String> {
    let name = crate::ui::telegram::TelegramApp::init().bot_username()?;
    let name = name.trim().trim_start_matches('@');
    if name.is_empty() {
        return None;
    }
    Some(format!("https://t.me/{name}"))
}

#[cfg(test)]
mod tests {
    //! As in `catalog_screen.rs`: `src/ui` is wasm-gated, so `cargo test`
    //! never compiles this module. Kept as the executable statement of the
    //! rules these helpers enforce.

    use super::*;

    fn band(
        name: &str,
        min: Option<i32>,
        max: Option<i32>,
        dmin: Option<f64>,
        dmax: Option<f64>,
    ) -> ApiRentalTerm {
        ApiRentalTerm {
            band: name.to_string(),
            min_days: min,
            max_days: max,
            discount_min: dmin,
            discount_max: dmax,
        }
    }

    #[test]
    fn a_band_with_no_published_discount_is_a_dash_not_zero_percent() {
        let lang = Lang::Russian;
        let row = band("week", Some(7), Some(13), None, None);
        assert_eq!(term_discount_label(&row, lang), "—");
    }

    #[test]
    fn an_unknown_band_is_shown_raw_not_mapped_to_a_rung() {
        let lang = Lang::Russian;
        assert_eq!(band_label("season", lang), "season");
        assert_ne!(band_label("season", lang), band_label("month", lang));
    }

    #[test]
    fn a_band_with_no_day_range_renders_no_day_line() {
        let lang = Lang::Russian;
        assert_eq!(
            term_days_label(&band("month", None, None, None, None), lang),
            None
        );
        assert!(term_days_label(&band("month", Some(30), None, None, None), lang).is_some());
    }

    #[test]
    fn an_empty_or_blank_colour_list_is_absent_not_an_empty_row() {
        assert_eq!(join_labels(&[]), None);
        assert_eq!(join_labels(&["  ".to_string()]), None);
        assert_eq!(
            join_labels(&["red".to_string(), " ".to_string(), "black".to_string()]),
            Some("red · black".to_string())
        );
    }
}
