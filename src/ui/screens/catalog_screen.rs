//! TurboBaby fleet catalog — the rental / sales list screen.
//!
//! Replaces the cannabis menu grid. Two screens make up the catalog: this
//! list, and [`BikeDetail`](crate::ui::screens::bike_detail::BikeDetail),
//! which this screen opens as a full-screen overlay (and which can be mounted
//! on its own route later without a change).
//!
//! # The one money rule (D9, D11)
//!
//! `rate_day_thb`, `deposit_thb`, `monthly_low_season_thb` and
//! `sale_price_thb` are nullable from the column to the pixel. A number this
//! shop has not published is rendered by [`money_thb`] as [`MONEY_DASH`] and
//! by nothing else — there is no `unwrap_or(0.0)`, no `unwrap_or_default()`
//! and no average in this file or in `bike_detail.rs`, because `฿0` on a
//! rental card reads as *free*.
//!
//! Four constructs in this tree manufacture that zero. Three are named in D9
//! (the `clamp` closure in `db::strains`, `NOT NULL DEFAULT 0` in SQL, and
//! the fail-open `try_get_warn!` macro). The fourth is
//! [`crate::trios::pricing::format_baht`] itself: its private
//! `sanitize_money` turns NaN and negatives into a confident `0.0`. That is
//! why [`finite_money`] filters every value *before* it reaches
//! `format_baht`, and why the filter — not the formatter — decides whether a
//! price exists at all.
//!
//! # The class discount (D11)
//!
//! `base_rate_thb_day` is the **pre**-class-discount published tariff.
//! Showing it as "the price" overstates every scooter by 33% and every
//! motorcycle by 18%. So the big number on a card is always the client price:
//! `rate_day_thb` when the API carries the door's number, otherwise the
//! published tariff with the published class discount applied and rounded
//! half-up — the arithmetic the owner's own quote sheet uses, reconciled to
//! the baht on six models in `data/fleet_seed.json` (`reconciliation.checks`).
//! The pre-discount tariff appears only on a line that says it is
//! pre-discount, and never alone.
//!
//! When neither number exists the screen says a manager quotes this price and
//! emits no number at all — D11 forbids invention and silence equally.
//!
//! # What is not here (D6, D14)
//!
//! No service or maintenance state (D6: admin only — two units read `overdue`
//! and a "serviced" badge we cannot keep accurate is exactly the number the
//! data-honesty rule forbids). No renter, handle, phone, debt, key code, TAX
//! or insurance date, purchase cost, or plate: the wire structs below have no
//! field to carry any of it (D14).

use crate::trios::core::Lang;
use crate::trios::i18n::{
    t, tf, Key, T_BIKE_AVAILABILITY, T_BIKE_AVAILABILITY_UNKNOWN, T_BIKE_BOOK_BLOCKED_NO_RATE,
    T_BIKE_BOOK_BLOCKED_NO_UNITS, T_BIKE_BOOK_BLOCKED_NOT_OFFERED,
    T_BIKE_BOOK_BLOCKED_NOT_WIRED, T_BIKE_BOOK_BLOCKED_UNKNOWN_AVAILABILITY, T_BIKE_CATALOG_DESC,
    T_BIKE_CATALOG_TITLE, T_BIKE_CC, T_BIKE_CLASS_DISCOUNT, T_BIKE_CLASS_MOTORCYCLE,
    T_BIKE_CLASS_SCOOTER, T_BIKE_DEPOSIT, T_BIKE_DETAILS, T_BIKE_FILTER_FREE_NOW,
    T_BIKE_FILTER_MOTORCYCLE, T_BIKE_FILTER_SCOOTER, T_BIKE_NOT_OFFERED_ALTERNATIVES,
    T_BIKE_NOT_OFFERED_TITLE, T_BIKE_NO_RESULTS, T_BIKE_PER_DAY, T_BIKE_PRICE_ON_REQUEST,
    T_BIKE_QUOTE_NOTE, T_BIKE_SORT_DEFAULT, T_BIKE_SORT_PRICE_ASC, T_BIKE_SORT_PRICE_DESC,
    T_BIKE_TARIFF_BEFORE_DISCOUNT, T_FILTER_ALL, T_SEARCH_PLACEHOLDER,
};
use crate::ui::api::context::api_base_url;
use crate::ui::components::bottom_nav::BottomNav;
use crate::ui::components::card_media::CardMedia;
use crate::ui::components::skeleton::{Skeleton, SkeletonShape};
use crate::ui::routes::Route;
use crate::ui::screens::bike_detail::BikeDetail;
use crate::ui::state::Cart;
use dioxus::prelude::*;
use serde::Deserialize;

// ─────────────────────────────────────────────────────────────────────────────
// Wire types
//
// Every money field is `Option<f64>` and every count is `Option<i64>`, with
// `#[serde(default)]` so a field the API has not shipped yet arrives as
// "absent" rather than failing the whole catalog — and "absent" renders as a
// dash, never as zero. Identity fields (`key`, `brand`, `model`, `class`) are
// required: a row that cannot say which bike it is has nothing to render.
// ─────────────────────────────────────────────────────────────────────────────

/// One bike **family** (a model the shop rents), not a physical unit.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ApiBike {
    /// Family key, e.g. `nmax-155`. This is the catalog id a cart line
    /// carries (D8): a customer books a model and the shop assigns the unit.
    pub key: String,
    pub brand: String,
    pub model: String,
    /// Set only where two tariffs share a model name, e.g. `NEW 2023+`.
    #[serde(default)]
    pub variant_label: Option<String>,
    /// `scooter` | `motorcycle`. Drives the class discount, so an unknown
    /// value is rendered raw and never mapped to a guess.
    pub class: String,
    /// Body style as the fleet register writes it (`naked`, `maxi-scooter`…).
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default)]
    pub displacement_cc: Option<i32>,
    /// The client-facing per-day price — the door's number, post class
    /// discount. `None` when the shop publishes none (D11: then a human
    /// quotes it and the bot emits no number).
    #[serde(default)]
    pub rate_day_thb: Option<f64>,
    /// The published tariff **before** the class discount. Never shown alone.
    #[serde(default)]
    pub base_rate_thb_day: Option<f64>,
    /// Published class discount as a fraction (scooter 0.25, motorcycle 0.15).
    #[serde(default)]
    pub class_discount: Option<f64>,
    #[serde(default)]
    pub deposit_thb: Option<f64>,
    #[serde(default)]
    pub monthly_low_season_thb: Option<f64>,
    /// Buy-out price. Never derived from what TurboBaby paid for the unit
    /// (D14) — when the shop publishes no sale price this stays `None`.
    #[serde(default)]
    pub sale_price_thb: Option<f64>,
    /// Whether this family is offered for sale at all. `None` means the API
    /// does not say, and the detail screen falls back to "there is a
    /// published sale price" rather than claiming anything.
    #[serde(default)]
    pub for_sale: Option<bool>,
    /// `Some(false)` closes the family to new rentals (D12: CLICK 125).
    /// `None` means the API does not say — which must not hide a bike, so it
    /// is treated as "offered" for display and still cannot be booked until a
    /// rate and a free unit exist.
    #[serde(default)]
    pub offered: Option<bool>,
    #[serde(default)]
    pub units_total: Option<i64>,
    #[serde(default)]
    pub units_available: Option<i64>,
    /// Display names of the families offered instead of a closed one (D12).
    #[serde(default)]
    pub offer_instead: Vec<String>,
    #[serde(default)]
    pub description_ru: Option<String>,
    #[serde(default)]
    pub description_en: Option<String>,
    #[serde(default)]
    pub image_url: Option<String>,
    #[serde(default)]
    pub sort_order: Option<i32>,
}

/// One physical unit of a family.
///
/// D14: there is deliberately no field here for a renter, a handle, a phone,
/// a debt, a key code, a TAX or insurance date, a purchase cost or a plate.
/// Issue #8 pins the same rule on the API side: the unit payload exposes
/// `status` and nothing about who has the bike.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ApiBikeUnit {
    /// Internal unit code. **Not** a plate number (D14).
    pub unit_code: String,
    #[serde(default)]
    pub model_year: Option<i32>,
    #[serde(default)]
    pub color: Option<String>,
    /// Kilometres travelled **since TurboBaby bought this unit** — not the
    /// odometer. One unit reads 2900 since purchase against 16433 on the
    /// clock, so this must never be labelled odometer or mileage
    /// (`fleet_seed.json:field_notes.km_since_purchase`).
    #[serde(default)]
    pub km_since_purchase: Option<i64>,
    /// `available` | `rented` | `service` | `retired`.
    #[serde(default)]
    pub status: Option<String>,
}

/// One published term band (`week` | `two_weeks` | `month`).
///
/// The discounts are **bands**, not multipliers: KB_faq publishes a range and
/// the exact number comes from a manager (D11).
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ApiRentalTerm {
    pub band: String,
    #[serde(default)]
    pub min_days: Option<i32>,
    #[serde(default)]
    pub max_days: Option<i32>,
    #[serde(default)]
    pub discount_min: Option<f64>,
    #[serde(default)]
    pub discount_max: Option<f64>,
}

/// `GET /api/bikes/:slug` — a family plus its units and its term ladder.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct BikeDetailPayload {
    #[serde(alias = "family")]
    pub bike: ApiBike,
    #[serde(default)]
    pub units: Vec<ApiBikeUnit>,
    #[serde(default)]
    pub terms: Vec<ApiRentalTerm>,
}

#[derive(Debug, Deserialize)]
struct BikesResponse {
    bikes: Vec<ApiBike>,
}

/// Accepts the flat detail shape (`{ key, brand, …, units, terms }`) as well
/// as the wrapped one. Written because the API and this screen are built by
/// two different workers in the same wave: the field *names* are the contract,
/// the envelope is not worth a broken screen.
#[derive(Debug, Deserialize)]
struct FlatBikeDetail {
    #[serde(flatten)]
    bike: ApiBike,
    #[serde(default)]
    units: Vec<ApiBikeUnit>,
    #[serde(default)]
    terms: Vec<ApiRentalTerm>,
}

// ─────────────────────────────────────────────────────────────────────────────
// THE money renderer
//
// Every price, deposit, monthly rate and sale price in the catalog and in the
// bike detail screen goes through `money_thb`. Nothing else formats money.
// ─────────────────────────────────────────────────────────────────────────────

/// What an absent money value looks like: an em dash. Never `0`, never an
/// average, never a "from" price (D9).
pub const MONEY_DASH: &str = "—";

/// Keeps only a money value this shop is willing to put on screen.
///
/// `None`, NaN, infinity and negatives all mean the same thing — the source
/// publishes nothing here — and so does `0.0`: a zero rental rate or deposit
/// is not a tariff, it is the `NOT NULL DEFAULT 0` / `try_get_warn!` failure
/// mode D9 names arriving on the wire. All of them become `None` here, and
/// every caller renders `None` as a dash and says a manager quotes the price.
///
/// This is also the guard in front of `format_baht`, whose `sanitize_money`
/// would otherwise turn NaN into a confident `฿0`.
pub fn finite_money(value: Option<f64>) -> Option<f64> {
    match value {
        Some(v) if v.is_finite() && v > 0.0 => Some(v),
        _ => None,
    }
}

/// The one money renderer: a published number, or [`MONEY_DASH`].
///
/// No call site is allowed to reach for `unwrap_or(0.0)` or
/// `unwrap_or_default()` instead — see the module docs.
pub fn money_thb(value: Option<f64>) -> String {
    match finite_money(value) {
        Some(v) => crate::trios::pricing::format_baht(v),
        None => MONEY_DASH.to_string(),
    }
}

/// The integer twin of [`money_thb`] for counts, years, cc and kilometres.
pub fn int_or_dash(value: Option<i64>) -> String {
    match value {
        Some(v) => v.to_string(),
        None => MONEY_DASH.to_string(),
    }
}

/// Keeps only a discount that is a fraction of 1 (0.25 = 25%).
///
/// A value outside `0.0..1.0` means the field is not what this code thinks it
/// is (a percent where a fraction was expected, say), so it is dropped rather
/// than used — a mis-scaled discount would compute a price, and a wrong price
/// is worse than a dash.
pub fn discount_fraction(value: Option<f64>) -> Option<f64> {
    match value {
        Some(v) if v.is_finite() && v > 0.0 && v < 1.0 => Some(v),
        _ => None,
    }
}

/// A discount as whole percent for display, e.g. `Some(0.25)` -> `Some(25)`.
pub fn discount_percent(value: Option<f64>) -> Option<i64> {
    discount_fraction(value).map(|v| (v * 100.0).round() as i64)
}

/// The per-day price a client actually pays, or `None`.
///
/// Preference order:
/// 1. `rate_day_thb` — the door's own client-facing number (D11: the door is
///    the authority and is never recomputed when it can be asked).
/// 2. the published tariff with the published class discount applied, rounded
///    half-up. `f64::round` rounds half away from zero, which for a positive
///    price is half-up, and that reproduces the owner's quote sheet to the
///    baht on all six reconciled models (939 -> 704, 998 -> 749, 690 -> 518,
///    790 -> 593, 449 -> 337, 2788 -> 2091). The rounding has to happen here
///    because `format_baht` truncates, and truncation would print 748 where
///    the owner's sheet says 749.
/// 3. `None` — and then the caller says a manager quotes this price and shows
///    no number at all. It never falls back to the pre-discount tariff.
pub fn client_rate_thb_day(bike: &ApiBike) -> Option<f64> {
    if let Some(from_door) = finite_money(bike.rate_day_thb) {
        return Some(from_door);
    }
    let base = finite_money(bike.base_rate_thb_day)?;
    let discount = discount_fraction(bike.class_discount)?;
    finite_money(Some((base * (1.0 - discount)).round()))
}

// ─────────────────────────────────────────────────────────────────────────────
// Booking eligibility
// ─────────────────────────────────────────────────────────────────────────────

/// Why the Book control is not usable. Every arm names a reason a customer
/// can read (issue #9: a family with no available unit shows the specs and a
/// disabled Book control **with the reason named**).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BookBlock {
    /// D12: the family is closed to new rentals.
    NotOffered,
    /// D11: no published price, so the bot must not take the booking.
    NoPublishedRate,
    /// Every unit is rented, in service or retired.
    NoUnitsFree,
    /// The API did not say how many units are free. Fail closed.
    UnknownAvailability,
    /// Nothing is wrong with the bike: this app has no bike cart line yet.
    NotWired,
}

impl BookBlock {
    /// The i18n key for the reason shown next to the disabled control.
    pub fn reason_key(self) -> Key {
        match self {
            Self::NotOffered => T_BIKE_BOOK_BLOCKED_NOT_OFFERED,
            Self::NoPublishedRate => T_BIKE_BOOK_BLOCKED_NO_RATE,
            Self::NoUnitsFree => T_BIKE_BOOK_BLOCKED_NO_UNITS,
            Self::UnknownAvailability => T_BIKE_BOOK_BLOCKED_UNKNOWN_AVAILABILITY,
            Self::NotWired => T_BIKE_BOOK_BLOCKED_NOT_WIRED,
        }
    }
}

/// `None` when this family can be booked from the app, otherwise the reason
/// it cannot. `booking_wired` is false until a bike cart line exists — see
/// the note on [`BikeDetail`](crate::ui::screens::bike_detail::BikeDetail).
pub fn book_block(bike: &ApiBike, booking_wired: bool) -> Option<BookBlock> {
    if bike.offered == Some(false) {
        return Some(BookBlock::NotOffered);
    }
    if client_rate_thb_day(bike).is_none() {
        return Some(BookBlock::NoPublishedRate);
    }
    match bike.units_available {
        Some(free) if free > 0 => {}
        Some(_) => return Some(BookBlock::NoUnitsFree),
        None => return Some(BookBlock::UnknownAvailability),
    }
    if !booking_wired {
        return Some(BookBlock::NotWired);
    }
    None
}

// ─────────────────────────────────────────────────────────────────────────────
// Display helpers shared with `bike_detail.rs`
// ─────────────────────────────────────────────────────────────────────────────

/// D12 names the three families the shop offers instead of CLICK 125. The API
/// is expected to carry them in `offer_instead`; this is the fallback so the
/// redirect cannot silently vanish if that field is missing, and it is keyed
/// on the one family the decision is about rather than applied to every closed
/// family.
const CLICK_125_KEY: &str = "click-125";
const CLICK_125_ALTERNATIVES: [&str; 3] = ["PCX 150", "ADV 150", "NMAX 155"];

/// `Yamaha NMAX 155`. The brand and model come from the register as data, so
/// they are not translated (same treatment the strain names had).
pub fn display_name(bike: &ApiBike) -> String {
    format!("{} {}", bike.brand.trim(), bike.model.trim())
        .trim()
        .to_string()
}

/// 🛵 for a scooter, 🏍️ for a motorcycle. Used as the media placeholder when
/// a family has no photo yet.
pub fn class_emoji(class: &str) -> &'static str {
    match class {
        "scooter" => "🛵",
        _ => "🏍️",
    }
}

/// Localised class name. An unrecognised class is shown raw rather than
/// guessed into one of the two discount classes.
pub fn class_label(class: &str, lang: Lang) -> String {
    match class {
        "scooter" => t(lang, T_BIKE_CLASS_SCOOTER).to_string(),
        "motorcycle" => t(lang, T_BIKE_CLASS_MOTORCYCLE).to_string(),
        other => other.to_string(),
    }
}

fn class_badge_style(class: &str) -> String {
    let (color, bg) = match class {
        "scooter" => ("#39ff14", "rgba(57,255,20,0.15)"),
        "motorcycle" => ("#ffe600", "rgba(255,230,0,0.15)"),
        _ => ("#888", "rgba(136,136,136,0.1)"),
    };
    format!(
        "font-size:13px;color:{};border:2px solid {};background:{};padding:2px 8px;",
        color, color, bg
    )
}

/// `Свободно 5 из 10`, or "a manager confirms availability" when the API did
/// not say. Never "0 of 0" invented from a missing field.
pub fn availability_line(bike: &ApiBike, lang: Lang) -> String {
    match (bike.units_available, bike.units_total) {
        (Some(free), Some(total)) if free >= 0 && total > 0 => {
            tf(lang, T_BIKE_AVAILABILITY, &[free.to_string(), total.to_string()])
        }
        _ => t(lang, T_BIKE_AVAILABILITY_UNKNOWN).to_string(),
    }
}

/// The families offered instead of a closed one (D12).
pub fn offer_instead_labels(bike: &ApiBike) -> Vec<String> {
    if !bike.offer_instead.is_empty() {
        return bike.offer_instead.clone();
    }
    if bike.key == CLICK_125_KEY {
        return CLICK_125_ALTERNATIVES
            .iter()
            .map(|name| (*name).to_string())
            .collect();
    }
    Vec::new()
}

/// The localised description, falling back to the Russian text (D13: ru is
/// primary and there is no Thai). `localized` reads the language itself.
pub fn description_for(bike: &ApiBike) -> String {
    crate::ui::lang::localized(
        bike.description_ru.as_deref().unwrap_or_default(),
        bike.description_en.as_deref(),
    )
}

/// Only http(s) or root-relative image paths reach `CardMedia` (the same
/// guard the strain cards used).
pub fn usable_image(url: Option<&str>) -> Option<String> {
    let url = url?;
    let ok = url.starts_with("http://")
        || url.starts_with("https://")
        || (url.starts_with('/') && !url.starts_with("//"));
    ok.then(|| url.to_string())
}

/// Orders two optional rates, always putting an absent rate **last** — in
/// both directions, because "no published price" is neither cheap nor
/// expensive. Written out rather than `partial_cmp().unwrap()` because the
/// crate warns on `unwrap_used` and a NaN is exactly what would hit it.
fn cmp_rate(a: Option<f64>, b: Option<f64>, descending: bool) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    match (finite_money(a), finite_money(b)) {
        (Some(x), Some(y)) => {
            let ord = if x < y {
                Ordering::Less
            } else if x > y {
                Ordering::Greater
            } else {
                Ordering::Equal
            };
            if descending {
                ord.reverse()
            } else {
                ord
            }
        }
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

/// Default order: offered families first, then the shop's own `sort_order`,
/// then engine size, then model name. `None` sorts last in each position
/// instead of being filled in with a zero.
fn default_sort_key(bike: &ApiBike) -> (bool, bool, Option<i32>, bool, Option<i32>, String) {
    (
        bike.offered == Some(false),
        bike.sort_order.is_none(),
        bike.sort_order,
        bike.displacement_cc.is_none(),
        bike.displacement_cc,
        bike.model.to_lowercase(),
    )
}

/// Free-text search over the fields a customer would type: brand, model,
/// variant, body and engine size.
fn matches_query(bike: &ApiBike, query: &str) -> bool {
    if query.is_empty() {
        return true;
    }
    let mut haystack = format!(
        "{} {} {} {}",
        bike.brand,
        bike.model,
        bike.variant_label.clone().unwrap_or_default(),
        bike.body.clone().unwrap_or_default()
    )
    .to_lowercase();
    if let Some(cc) = bike.displacement_cc {
        haystack.push(' ');
        haystack.push_str(&cc.to_string());
    }
    haystack.contains(query)
}

fn filter_tab_style(is_active: bool) -> String {
    if is_active {
        "font-size:12px;font-weight:600;padding:8px 16px;background:rgba(57,255,20,0.15);color:#39ff14;border:3px solid #39ff14;border-radius:20px;cursor:pointer;white-space:nowrap;".to_string()
    } else {
        "font-size:12px;font-weight:600;padding:8px 16px;background:transparent;color:#888;border:3px solid #2a2a4a;border-radius:20px;cursor:pointer;white-space:nowrap;".to_string()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Fetching
// ─────────────────────────────────────────────────────────────────────────────

/// Localised copy for a failed request. Mirrors the strain catalog: a 5xx/429
/// renders UX copy, not a parse error from an HTML body.
fn fetch_error(status: u16) -> String {
    crate::trios::api_errors::friendly_response_error(crate::ui::lang::current_lang(), status)
}

async fn get_text(url: &str) -> Result<String, String> {
    let response = crate::ui::api::local_client::LocalClient::new()
        .get(url)
        .send()
        .await
        .map_err(|_| fetch_error(0))?;
    let status = response.status().as_u16();
    if !(200..300).contains(&status) {
        return Err(fetch_error(status));
    }
    response.text().await.map_err(|_| fetch_error(0))
}

/// `GET /api/bikes` — the whole offered fleet. Accepts either `{ "bikes": [] }`
/// or a bare array.
pub async fn fetch_bikes() -> Result<Vec<ApiBike>, String> {
    let text = get_text(&format!("{}/api/bikes", api_base_url())).await?;
    if let Ok(resp) = serde_json::from_str::<BikesResponse>(&text) {
        return Ok(resp.bikes);
    }
    serde_json::from_str::<Vec<ApiBike>>(&text).map_err(|_| fetch_error(0))
}

/// `GET /api/bikes/:slug` — one family with its units and term ladder.
pub async fn fetch_bike_detail(slug: &str) -> Result<BikeDetailPayload, String> {
    let text = get_text(&format!("{}/api/bikes/{}", api_base_url(), slug)).await?;
    if let Ok(payload) = serde_json::from_str::<BikeDetailPayload>(&text) {
        return Ok(payload);
    }
    match serde_json::from_str::<FlatBikeDetail>(&text) {
        Ok(flat) => Ok(BikeDetailPayload {
            bike: flat.bike,
            units: flat.units,
            terms: flat.terms,
        }),
        Err(_) => Err(fetch_error(0)),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Screen
// ─────────────────────────────────────────────────────────────────────────────

const FILTER_ALL: &str = "all";
const FILTER_SCOOTER: &str = "scooter";
const FILTER_MOTORCYCLE: &str = "motorcycle";
const SORT_DEFAULT: &str = "default";
const SORT_PRICE_ASC: &str = "price-asc";
const SORT_PRICE_DESC: &str = "price-desc";

/// The fleet list. Mounted at `/menu` through
/// [`MenuScreen`](crate::ui::screens::menu_screen::MenuScreen).
#[component]
pub fn CatalogScreen() -> Element {
    let mut active_class = use_signal(|| FILTER_ALL.to_string());
    let mut only_free = use_signal(|| false);
    let mut active_sort = use_signal(|| SORT_DEFAULT.to_string());
    let mut search_query = use_signal(String::new);

    // Held at screen level, not per card: the cards render inline in a loop
    // where a per-card `use_signal` would break hook ordering the moment the
    // list is re-sorted. Same reason the strain screen kept its selection here.
    let mut selected_key = use_signal(|| None::<String>);

    let cart = use_context::<Signal<Cart>>();
    let cart_count: u32 = cart.read().items.iter().map(|i| i.quantity).sum();

    let bikes_resource: Resource<Result<Vec<ApiBike>, String>> =
        use_resource(move || async move { fetch_bikes().await });

    let lang = crate::ui::lang::current_lang();
    let title = t(lang, T_BIKE_CATALOG_TITLE);
    let subtitle = t(lang, T_BIKE_CATALOG_DESC);

    rsx! {
        div { style: "min-height:100vh;background:#0f0f1a;color:#e8e8e8;padding-bottom:calc(96px + env(safe-area-inset-bottom));",

            div { style: "padding:20px 16px 16px;text-align:center;position:relative;",
                h1 { style: "font-size:24px;font-weight:800;color:#39ff14;text-shadow:3px 3px 0 #000,0 0 10px rgba(57,255,20,0.5);letter-spacing:2px;",
                    "{title}"
                }
                p { style: "font-size:13px;color:#888;margin-top:4px;", "{subtitle}" }
                if cart_count > 0 {
                    Link { to: Route::Cart {},
                        div { style: "
                            position:absolute;top:20px;right:16px;
                            background:#39ff14;color:#000;
                            font-size:13px;font-weight:700;padding:4px 8px;
                            box-shadow:2px 2px 0 #000;cursor:pointer;
                        ",
                            "🛒 {cart_count}"
                        }
                    }
                }
            }

            div { style: "padding:0 16px 12px;",
                input {
                    r#type: "text",
                    placeholder: "{t(lang, T_SEARCH_PLACEHOLDER)}",
                    value: "{search_query()}",
                    style: "
                        width:100%;box-sizing:border-box;
                        font-size:15px;padding:10px 12px;
                        background:#0f0f1a;color:#e8e8e8;
                        border:4px solid #2a2a4a;border-radius:0;
                    ",
                    oninput: move |e| search_query.set(e.value()),
                }
            }

            div { style: "display:flex;gap:6px;padding:0 16px 12px;overflow-x:auto;",
                button {
                    style: filter_tab_style(active_class() == FILTER_ALL),
                    onclick: move |_| active_class.set(FILTER_ALL.to_string()),
                    {t(lang, T_FILTER_ALL)}
                }
                button {
                    style: filter_tab_style(active_class() == FILTER_SCOOTER),
                    onclick: move |_| active_class.set(FILTER_SCOOTER.to_string()),
                    {t(lang, T_BIKE_FILTER_SCOOTER)}
                }
                button {
                    style: filter_tab_style(active_class() == FILTER_MOTORCYCLE),
                    onclick: move |_| active_class.set(FILTER_MOTORCYCLE.to_string()),
                    {t(lang, T_BIKE_FILTER_MOTORCYCLE)}
                }
                button {
                    style: filter_tab_style(only_free()),
                    onclick: move |_| {
                        let next = !only_free();
                        only_free.set(next);
                    },
                    {t(lang, T_BIKE_FILTER_FREE_NOW)}
                }
            }

            div { style: "display:flex;gap:6px;padding:0 16px 12px;overflow-x:auto;",
                button {
                    style: filter_tab_style(active_sort() == SORT_DEFAULT),
                    onclick: move |_| active_sort.set(SORT_DEFAULT.to_string()),
                    {t(lang, T_BIKE_SORT_DEFAULT)}
                }
                button {
                    style: filter_tab_style(active_sort() == SORT_PRICE_ASC),
                    onclick: move |_| active_sort.set(SORT_PRICE_ASC.to_string()),
                    {t(lang, T_BIKE_SORT_PRICE_ASC)}
                }
                button {
                    style: filter_tab_style(active_sort() == SORT_PRICE_DESC),
                    onclick: move |_| active_sort.set(SORT_PRICE_DESC.to_string()),
                    {t(lang, T_BIKE_SORT_PRICE_DESC)}
                }
            }

            {
                match &*bikes_resource.read() {
                    Some(Ok(all_bikes)) => {
                        let class_filter = active_class();
                        let sort_val = active_sort();
                        let free_only = only_free();
                        let query = search_query().to_lowercase();
                        let mut filtered: Vec<ApiBike> = all_bikes
                            .iter()
                            .filter(|b| class_filter == FILTER_ALL || b.class == class_filter)
                            // "Free now" means the API said a unit is free. A
                            // family whose availability is unknown is left out
                            // rather than shown as free.
                            .filter(|b| !free_only || matches!(b.units_available, Some(n) if n > 0))
                            .filter(|b| matches_query(b, &query))
                            .cloned()
                            .collect();
                        match sort_val.as_str() {
                            SORT_PRICE_ASC => filtered.sort_by(|a, b| {
                                cmp_rate(client_rate_thb_day(a), client_rate_thb_day(b), false)
                            }),
                            SORT_PRICE_DESC => filtered.sort_by(|a, b| {
                                cmp_rate(client_rate_thb_day(a), client_rate_thb_day(b), true)
                            }),
                            _ => filtered.sort_by(|a, b| default_sort_key(a).cmp(&default_sort_key(b))),
                        }

                        if filtered.is_empty() {
                            let label = match class_filter.as_str() {
                                FILTER_SCOOTER => t(lang, T_BIKE_FILTER_SCOOTER).to_string(),
                                FILTER_MOTORCYCLE => t(lang, T_BIKE_FILTER_MOTORCYCLE).to_string(),
                                _ => t(lang, T_FILTER_ALL).to_string(),
                            };
                            rsx! {
                                div { style: "text-align:center;padding:48px 16px;",
                                    p { style: "font-size:20px;margin-bottom:12px;", "🔍" }
                                    p { style: "font-size:15px;color:#888;", {tf(lang, T_BIKE_NO_RESULTS, &[label])} }
                                }
                            }
                        } else {
                            rsx! {
                                div { style: "display:grid;grid-template-columns:1fr 1fr;gap:12px;padding:8px 16px 0;",
                                    {filtered.into_iter().map(move |bike| {
                                        let key = bike.key.clone();
                                        render_bike_card(bike, move || selected_key.set(Some(key.clone())), lang)
                                    })}
                                }
                                p { style: "font-size:13px;color:#8b8b9e;padding:16px 16px 0;line-height:1.4;",
                                    {t(lang, T_BIKE_QUOTE_NOTE)}
                                }
                            }
                        }
                    }
                    Some(Err(e)) => {
                        // `e` already carries localised copy from
                        // friendly_response_error — no raw fallback.
                        let err_msg = e.clone();
                        rsx! {
                            div { style: "text-align:center;padding:48px 16px;",
                                p { style: "font-size:20px;margin-bottom:12px;", "⚠️" }
                                p { style: "font-size:15px;color:#ff4757;", "{err_msg}" }
                            }
                        }
                    }
                    None => {
                        rsx! {
                            div { style: "padding:0 16px;display:grid;grid-template-columns:repeat(2,1fr);gap:12px;",
                                Skeleton { shape: SkeletonShape::Card }
                                Skeleton { shape: SkeletonShape::Card }
                                Skeleton { shape: SkeletonShape::Card }
                                Skeleton { shape: SkeletonShape::Card }
                            }
                        }
                    }
                }
            }

            // Detail overlay. `BikeDetail` takes its slug as a prop and refetches
            // when it changes, so it works both here and on a route of its own.
            {selected_key().map(|slug| {
                let slug_key = slug.clone();
                rsx! {
                    div {
                        key: "{slug_key}",
                        style: "position:fixed;inset:0;z-index:1000;background:#0f0f1a;overflow-y:auto;",
                        BikeDetail {
                            slug,
                            on_close: Some(EventHandler::new(move |_| selected_key.set(None))),
                        }
                    }
                }
            })}

            BottomNav { cart_count }
        }
    }
}

/// One catalog card. A free function rather than a component for the same
/// reason the strain card was: the grid re-sorts, and hooks must not live
/// inside the loop.
///
/// The card has no Book control at all — booking needs dates, and a button
/// that cannot do anything is the lie issue #7 is about. Tapping the card
/// opens the detail screen, where the state of the Book control is spelled out
/// with its reason.
fn render_bike_card<S>(bike: ApiBike, mut on_select: S, lang: Lang) -> Element
where
    S: FnMut() + 'static,
{
    let name = display_name(&bike);
    let emoji = class_emoji(&bike.class);
    let is_offered = bike.offered != Some(false);
    let rate = client_rate_thb_day(&bike);
    let rate_str = money_thb(rate);
    let has_rate = rate.is_some();
    let deposit_str = money_thb(bike.deposit_thb);
    let tariff_before = finite_money(bike.base_rate_thb_day);
    // The pre-discount tariff is worth showing only when it is genuinely a
    // different number from the one above it; otherwise it is noise that looks
    // like a second price.
    let show_tariff_before = match (rate, tariff_before) {
        (Some(client), Some(published)) => (published - client).abs() >= 1.0,
        (None, Some(_)) => false,
        _ => false,
    };
    let tariff_line = tf(
        lang,
        T_BIKE_TARIFF_BEFORE_DISCOUNT,
        &[money_thb(bike.base_rate_thb_day)],
    );
    let discount_line = discount_percent(bike.class_discount)
        .map(|pct| tf(lang, T_BIKE_CLASS_DISCOUNT, &[pct.to_string()]));
    let availability = availability_line(&bike, lang);
    let alternatives = offer_instead_labels(&bike);
    let cc_line = bike
        .displacement_cc
        .map(|cc| tf(lang, T_BIKE_CC, &[cc.to_string()]));
    let badge_style = class_badge_style(&bike.class);
    let badge_label = class_label(&bike.class, lang);
    let variant = bike.variant_label.clone();
    let border_color = if !is_offered { "#4a4a5a" } else { "#2a2a4a" };
    let card_style = format!(
        "background:#16213e;border:4px solid {};box-shadow:4px 4px 0 #000;overflow:hidden;position:relative;cursor:pointer;{}",
        border_color,
        if is_offered { "" } else { "opacity:0.75;" }
    );

    rsx! {
        div { key: "{bike.key}", class: "comet-card", style: card_style,
            onclick: move |_| on_select(),
            CardMedia {
                image_url: usable_image(bike.image_url.as_deref()),
                video_url: None,
                emoji: emoji.to_string(),
                alt: name.clone(),
                aspect_ratio: Some("4/3".to_string()),
                div { style: "position:absolute;top:8px;left:8px;display:flex;flex-direction:column;gap:4px;z-index:2;align-items:flex-start;",
                    {variant.clone().map(|label| rsx! {
                        span { style: "
                            font-size:13px;font-weight:700;background:#00e5ff;color:#000;
                            padding:4px 8px;box-shadow:2px 2px 0 #000;
                        ", "{label}" }
                    })}
                    {(!is_offered).then(|| rsx! {
                        span { style: "
                            font-size:13px;font-weight:700;background:#ff4757;color:#fff;
                            padding:4px 8px;box-shadow:2px 2px 0 #000;
                        ", {t(lang, T_BIKE_NOT_OFFERED_TITLE)} }
                    })}
                }
            }
            div { style: "padding:12px;",
                div { style: "font-size:17px;font-weight:700;margin-bottom:6px;color:#fff;line-height:1.2;text-shadow:2px 2px 0 #000;",
                    "{name}"
                }
                div { style: "display:flex;gap:6px;align-items:center;margin-bottom:8px;flex-wrap:wrap;",
                    span { style: badge_style, "{badge_label}" }
                    {cc_line.map(|line| rsx! {
                        span { style: "font-size:13px;color:#39ff14;font-weight:700;", "{line}" }
                    })}
                }

                {if is_offered {
                    rsx! {
                        div { style: "display:flex;gap:6px;align-items:baseline;flex-wrap:wrap;margin-bottom:4px;",
                            {if has_rate {
                                rsx! {
                                    span { style: "font-size:20px;font-weight:800;color:#ffe600;text-shadow:2px 2px 0 #000;", "{rate_str}" }
                                    span { style: "font-size:13px;color:#888;", {t(lang, T_BIKE_PER_DAY)} }
                                }
                            } else {
                                // D11: no number at all, and a line that says a
                                // human quotes it.
                                rsx! {
                                    span { style: "font-size:13px;color:#888;font-style:italic;", {t(lang, T_BIKE_PRICE_ON_REQUEST)} }
                                }
                            }}
                        }
                        {show_tariff_before.then(|| rsx! {
                            div { style: "font-size:13px;color:#8b8b9e;margin-bottom:2px;", "{tariff_line}" }
                        })}
                        {discount_line.clone().map(|line| rsx! {
                            div { style: "font-size:13px;color:#39ff14;margin-bottom:4px;", "{line}" }
                        })}
                        div { style: "font-size:13px;color:#aaa;margin-bottom:2px;",
                            {t(lang, T_BIKE_DEPOSIT)}
                            " "
                            "{deposit_str}"
                        }
                        div { style: "font-size:13px;color:#888;", "{availability}" }
                    }
                } else {
                    // D12: closed to new rentals. No price, no availability, no
                    // Book — just the fact and where to go instead.
                    rsx! {
                        div { style: "font-size:13px;color:#ff4757;font-weight:700;margin-bottom:4px;",
                            {t(lang, T_BIKE_NOT_OFFERED_TITLE)}
                        }
                        {(!alternatives.is_empty()).then(|| rsx! {
                            div { style: "font-size:13px;color:#aaa;line-height:1.35;",
                                {tf(lang, T_BIKE_NOT_OFFERED_ALTERNATIVES, &[alternatives.join(" · ")])}
                            }
                        })}
                    }
                }}
            }
            div { style: "padding:0 12px 12px;",
                button {
                    style: "
                        font-size:14px;font-weight:700;
                        width:100%;padding:12px 20px;
                        background:transparent;color:#39ff14;
                        border:4px solid #39ff14;
                        box-shadow:3px 3px 0 #000;
                        cursor:pointer;
                    ",
                    {t(lang, T_BIKE_DETAILS)}
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    //! These assertions do not run in CI today: `src/ui` is
    //! `#[cfg(target_arch = "wasm32")]`, so `cargo test` never reaches this
    //! module (the same trap `ui/share.rs` documents for its own 22 dead
    //! assertions). They are kept because they are the executable statement of
    //! the money rule, and they travel with the helpers if the helpers move to
    //! `trios::pricing` — which is where they belong and where they would run.

    use super::*;

    fn bike(rate: Option<f64>, base: Option<f64>, discount: Option<f64>) -> ApiBike {
        ApiBike {
            key: "nmax-155".to_string(),
            brand: "Yamaha".to_string(),
            model: "NMAX 155".to_string(),
            variant_label: None,
            class: "scooter".to_string(),
            body: None,
            displacement_cc: Some(155),
            rate_day_thb: rate,
            base_rate_thb_day: base,
            class_discount: discount,
            deposit_thb: None,
            monthly_low_season_thb: None,
            sale_price_thb: None,
            for_sale: None,
            offered: Some(true),
            units_total: Some(10),
            units_available: Some(5),
            offer_instead: Vec::new(),
            description_ru: None,
            description_en: None,
            image_url: None,
            sort_order: None,
        }
    }

    #[test]
    fn absent_money_is_a_dash_and_never_a_zero() {
        assert_eq!(money_thb(None), MONEY_DASH);
        assert_eq!(money_thb(Some(f64::NAN)), MONEY_DASH);
        assert_eq!(money_thb(Some(f64::INFINITY)), MONEY_DASH);
        assert_eq!(money_thb(Some(-1.0)), MONEY_DASH);
        // A zero on the wire is the `NOT NULL DEFAULT 0` failure mode, not a
        // free rental.
        assert_eq!(money_thb(Some(0.0)), MONEY_DASH);
        assert_eq!(money_thb(Some(3000.0)), "฿3,000");
    }

    #[test]
    fn client_rate_applies_the_class_discount_half_up() {
        // The six reconciled models from data/fleet_seed.json.
        for (base, expected) in [
            (939.0, 704.0),
            (998.0, 749.0),
            (690.0, 518.0),
            (790.0, 593.0),
            (449.0, 337.0),
            (2788.0, 2091.0),
        ] {
            let b = bike(None, Some(base), Some(0.25));
            assert_eq!(client_rate_thb_day(&b), Some(expected));
        }
    }

    #[test]
    fn the_door_wins_and_a_missing_discount_does_not_fall_back_to_the_tariff() {
        assert_eq!(client_rate_thb_day(&bike(Some(700.0), Some(939.0), Some(0.25))), Some(700.0));
        // No class discount published => no computed price, and the
        // pre-discount tariff is NOT used as the price.
        assert_eq!(client_rate_thb_day(&bike(None, Some(939.0), None)), None);
        assert_eq!(client_rate_thb_day(&bike(None, None, Some(0.25))), None);
    }

    #[test]
    fn a_closed_family_and_a_priceless_one_cannot_be_booked() {
        let mut closed = bike(Some(187.0), None, None);
        closed.offered = Some(false);
        assert_eq!(book_block(&closed, true), Some(BookBlock::NotOffered));

        let priceless = bike(None, None, None);
        assert_eq!(book_block(&priceless, true), Some(BookBlock::NoPublishedRate));

        let mut none_free = bike(Some(337.0), None, None);
        none_free.units_available = Some(0);
        assert_eq!(book_block(&none_free, true), Some(BookBlock::NoUnitsFree));

        let mut unknown = bike(Some(337.0), None, None);
        unknown.units_available = None;
        assert_eq!(book_block(&unknown, true), Some(BookBlock::UnknownAvailability));

        assert_eq!(book_block(&bike(Some(337.0), None, None), false), Some(BookBlock::NotWired));
        assert_eq!(book_block(&bike(Some(337.0), None, None), true), None);
    }

    #[test]
    fn click_125_keeps_its_redirect_even_without_the_api_field() {
        let mut click = bike(None, None, None);
        click.key = "click-125".to_string();
        click.offered = Some(false);
        assert_eq!(
            offer_instead_labels(&click),
            vec![
                "PCX 150".to_string(),
                "ADV 150".to_string(),
                "NMAX 155".to_string()
            ]
        );
        // The fallback belongs to that one decision, not to every closed family.
        let mut other = bike(None, None, None);
        other.key = "forza-300".to_string();
        other.offered = Some(false);
        assert!(offer_instead_labels(&other).is_empty());
    }

    #[test]
    fn an_absent_rate_sorts_last_in_both_directions() {
        use std::cmp::Ordering;
        assert_eq!(cmp_rate(Some(100.0), None, false), Ordering::Less);
        assert_eq!(cmp_rate(Some(100.0), None, true), Ordering::Less);
        assert_eq!(cmp_rate(None, Some(100.0), true), Ordering::Greater);
    }
}
