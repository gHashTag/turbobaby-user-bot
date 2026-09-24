//! Marketing-flag price math, shared between WASM customer UI and any future
//! backend price-authority check.
//!
//! **Why this lives in `trios/`:** the module compiles for both `backend`
//! (tokio_postgres handlers in `api/`) and `wasm32` (Dioxus `ui/screens/`).
//! Before this module existed (cycle #54), the precedence rules were inlined
//! into `menu_screen.rs::render_strain_card`. Re-deriving them on the server
//! for an order-tampering check would have introduced two copies of the same
//! decision tree — easy to drift, hard to test. Cycle #55 extracted them
//! once with table-driven unit tests so divergence is impossible.
//!
//! ## Precedence (highest to lowest)
//!
//! 1. **Strain of the Day** with a positive `strain_of_day_discount`.
//! 2. **Active sale window** (`sale_active` AND `sale_until` either NULL or
//!    in the future) — within this branch:
//!    a) `sale_price` if set, finite, positive, and lower than base.
//!    b) `discount_percent` if set and positive.
//! 3. **Base** `price_per_gram`.
//!
//! Non-finite inputs are sanitized to 0 to prevent NaN propagation into UI
//! totals or DB writes.

use chrono::{DateTime, Utc};

/// Inputs needed to compute the effective per-gram price.
///
/// Designed as a plain-data struct so callers can construct it from either
/// `db::strains::Strain` (backend) or `ApiStrain` (wasm) without forcing
/// either crate to depend on the other.
#[derive(Debug, Clone)]
pub struct MarketingFlags<'a> {
    pub price_per_gram: f64,
    pub is_strain_of_day: bool,
    pub strain_of_day_discount: f64,
    pub sale_active: bool,
    /// RFC3339 timestamp string; `None` or empty means "no expiry".
    pub sale_until: Option<&'a str>,
    pub sale_price: Option<f64>,
    pub discount_percent: f64,
    pub is_new_arrival: bool,
    pub new_until: Option<&'a str>,
}

/// Is the given RFC3339 expiry timestamp still in the future?
///
/// Returns `true` when:
///   * `until` is `None`, or
///   * `until` is empty, or
///   * `until` parses as RFC3339 and is strictly greater than `now`, or
///   * `until` fails to parse (permissive — better to show a stale badge
///     than to misrepresent the catalog because of a malformed timestamp).
pub fn is_active_until(until: Option<&str>, now: DateTime<Utc>) -> bool {
    let Some(s) = until else { return true };
    if s.is_empty() {
        return true;
    }
    match DateTime::parse_from_rfc3339(s) {
        Ok(t) => t > now,
        Err(_) => true,
    }
}

/// Output of [`effective_strain_price`].
///
/// `price` is the per-gram price the customer actually pays. `has_discount`
/// is a UI hint — when `true`, the customer card renders the strikethrough
/// original price next to the new one.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PricedStrain {
    pub price: f64,
    pub has_discount: bool,
}

/// Apply the precedence rules in the module docs and return the price the
/// customer should pay.
pub fn effective_strain_price(flags: &MarketingFlags<'_>, now: DateTime<Utc>) -> PricedStrain {
    let base = sanitize_money(flags.price_per_gram);
    let sotd_discount = sanitize_pct(flags.strain_of_day_discount);

    // 1. SOTD wins outright when there's a positive discount.
    if flags.is_strain_of_day && sotd_discount > 0.0 {
        return PricedStrain {
            price: (base * (1.0 - sotd_discount / 100.0)).max(0.0),
            has_discount: true,
        };
    }

    // 2. Active sale window — sale_price overrides, then discount_percent.
    let sale_live = flags.sale_active && is_active_until(flags.sale_until, now);
    if sale_live {
        // 2a) explicit sale_price beats base only when it's actually lower.
        if let Some(sp) = flags
            .sale_price
            .filter(|v| v.is_finite() && *v > 0.0 && *v < base)
        {
            return PricedStrain {
                price: sp.max(0.0),
                has_discount: true,
            };
        }
        // 2b) percentage discount.
        let pct = sanitize_pct(flags.discount_percent);
        if pct > 0.0 {
            return PricedStrain {
                price: (base * (1.0 - pct / 100.0)).max(0.0),
                has_discount: true,
            };
        }
    }

    // 3. Base.
    PricedStrain {
        price: base,
        has_discount: false,
    }
}

fn sanitize_money(v: f64) -> f64 {
    if v.is_finite() {
        v.max(0.0)
    } else {
        0.0
    }
}

fn sanitize_pct(v: f64) -> f64 {
    if v.is_finite() {
        v.clamp(0.0, 100.0)
    } else {
        0.0
    }
}

// ── Other catalogs (cycle #58 / C). Accessories and tea products have a
//    flat `price` column (no discount machinery). Sets — `sets`,
//    `accessory_sets`, `tea_sets` — share `(total_price, discount_percent)`.
//    All three set tables use UUID primary keys with no overlap, so the
//    server-side lookup can UNION them into a single map.

/// Sanitised per-unit price for an accessory row. Just clamps non-finite /
/// negative values to 0 — accessories have no sale flags.
pub fn effective_accessory_price(price: f64) -> f64 {
    sanitize_money(price)
}

/// Tea products use the same flat-price model as accessories.
pub fn effective_tea_price(price: f64) -> f64 {
    sanitize_money(price)
}

/// Apply a percentage discount to a set's pre-set `total_price`. Shared by
/// the three set tables (`sets`, `accessory_sets`, `tea_sets`) — schema is
/// identical and customer-facing pricing is, too.
pub fn effective_set_price(total_price: f64, discount_percent: f64) -> f64 {
    let base = sanitize_money(total_price);
    let pct = sanitize_pct(discount_percent);
    (base * (1.0 - pct / 100.0)).max(0.0)
}

/// D9's honesty filter for a PRICE, and the one definition of an absent price.
///
/// `None`, NaN, infinity, a negative and `0.0` all mean the same thing: nobody
/// published this number. Zero is in that list on purpose — D9 measured that
/// "a price of 0 reads as FREE", and `฿0` is the single most misleading string
/// a price box can print.
///
/// It lives here because `src/ui/components/bike_card.rs` is
/// `cfg(target_arch = "wasm32")`: the catalog's copy cannot be linked by the
/// server, by `src/trios`, or by a `cargo test` binary, so a shared boundary
/// had to live where both runtimes compile (D15). `bike_card::published`
/// delegates to this and keeps its name, because `tests/money_is_never_invented.rs`
/// pins that name as the component layer's canonical helper.
pub fn published_money(amount: Option<f64>) -> Option<f64> {
    measured_money(amount).filter(|v| *v > 0.0)
}

/// Keep only a client-facing day rate returned by the authoritative door.
///
/// Absence and invalid money stay absent. This helper deliberately accepts no
/// base tariff or class discount, so a silent door has no path to a computed
/// client price (D11) and the same boundary is shared by native and WASM code
/// (D15).
pub fn authoritative_door_rate(client_rate_thb_day: Option<f64>) -> Option<f64> {
    published_money(client_rate_thb_day)
}

/// What one cart line is worth, or `None` when nobody priced it.
///
/// The multiplication is here rather than at each screen because a line total
/// is the one place an absent unit price used to become a visible number: a
/// `0.0` that survived the wire rendered as `฿0` on the line AND added `0` to
/// the cart, so the customer saw a free item and a total that agreed with it.
pub fn cart_line_total(unit_price: Option<f64>, quantity: u32) -> Option<f64> {
    published_money(unit_price).map(|price| price * quantity as f64)
}

/// The total of a cart, or `None` when the cart holds a line nobody priced.
///
/// One unpriced line makes the WHOLE total unknown. Summing the rest and
/// showing the result is not a partial answer, it is a wrong one: the figure
/// understates the cart and presents the understatement as a measured fact,
/// which is D9's confident zero wearing a different hat. A non-finite or
/// negative amount lands in the same `None` — `src/ui/api/http.rs:445` records
/// a non-finite `unit_price` reaching `recalculate_total` and turning the whole
/// cart total into NaN.
///
/// `None` is not a refusal to sell. It is the instruction to say what D11
/// requires and this file already names: a human quotes this price.
/// It is written in terms of [`cart_line_total`] rather than re-deciding what
/// counts as a price, because the first draft of this function did re-decide
/// and got it wrong within the same hour: it admitted a `Some(0.0)` line that
/// `cart_line_total` calls absent, so the line rendered a dash and the total
/// counted it as free — the two-implementations drift D15 exists to stop,
/// reproduced in twelve lines of new code.
pub fn cart_total<I>(lines: I) -> Option<f64>
where
    I: IntoIterator<Item = (Option<f64>, u32)>,
{
    let mut sum = 0.0_f64;
    for (unit_price, quantity) in lines {
        sum += cart_line_total(unit_price, quantity)?;
    }
    sum.is_finite().then_some(sum)
}

/// Resolve the client-facing day rate from its complete wire context.
///
/// `base_rate_thb_day` and `class_discount` are accepted deliberately so the
/// native regression test can prove that populated file inputs never become a
/// fallback. They are reference/audit facts only. A number reaches the client
/// only when the API also identifies it as a door result (D11).
pub fn client_day_rate(
    client_rate_thb_day: Option<f64>,
    client_rate_source: Option<&str>,
    _base_rate_thb_day: Option<f64>,
    _class_discount: Option<f64>,
) -> Option<f64> {
    if client_rate_source != Some("door") {
        return None;
    }
    authoritative_door_rate(client_rate_thb_day)
}

// ── The three-state rate, transcribed from `specs/turbobaby/pricing_honesty.t27`
//    (D11, D15). The spec is executed by the `t27-contracts` CI job, so this is
//    a transcription and not a design: where the two disagree the spec wins and
//    the disagreement is a bug here. It lives in this module rather than beside
//    either screen because `src/lib.rs` gates `pub mod ui;` on `wasm32` — nothing
//    under `src/ui` is compiled by `cargo test`, so an assertion written there
//    proves nothing. These run.

/// The sentence a dash must carry. Mirrors the spec's `MUST_SAY` (:157), which
/// is D11's `on_silence.must_say`. The locale layer does the wording (D13); what
/// is pinned here is that *something* is said.
pub const SAY_HUMAN_QUOTES: &str = "a human quotes this price";

/// What a render may emit. Two shapes and no third, because there is no number
/// to put in one — no "approximately", no "from", no "average". Mirrors
/// `SHAPE_AMOUNT` / `SHAPE_DASH`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RateShape {
    /// A number the door returned. It speaks for itself.
    Amount,
    /// No number. Never travels alone — see [`RenderedRate::say`].
    Dash,
}

/// The render result: a typed triple rather than a formatted string, because
/// the typesetting is locale work. Mirrors the spec's `RenderedRate` (:229).
#[derive(Debug, Clone, PartialEq)]
pub struct RenderedRate {
    pub shape: RateShape,
    /// Present only for [`RateShape::Amount`]. The spec fills the dash's slot
    /// with a poison marker (`NOT_AN_AMOUNT_THB_DAY = -1`) so that code reading
    /// it without checking the tag breaks obviously; Rust's `Option` makes the
    /// same slot unreadable outright, which is the stronger guarantee.
    pub thb_day: Option<f64>,
    /// The obligation travelling with the shape. `None` is the spec's
    /// `SAY_NOTHING_EXTRA` — an amount needs no sentence. `Some` is
    /// `SAY_HUMAN_QUOTES`, and a `Dash` always carries it: the spec's invariant
    /// `the_dash_never_travels_alone` (:398) exists because D11 lists *silence*
    /// alongside invention in `must_not_emit`. Saying nothing is the other half
    /// of the offence, not the safe default.
    pub say: Option<&'static str>,
}

/// Render a client-facing day rate from its complete wire context.
///
/// Delegates the provenance gate to [`client_day_rate`] so there is one door
/// gate in the codebase and not two (D15). Total over its input: every value
/// lands on `Amount` or `Dash`, and absence and invalid money land together —
/// both mean nobody measured a rate, and the client-facing consequence is
/// identical.
pub fn render_client_rate(
    client_rate_thb_day: Option<f64>,
    client_rate_source: Option<&str>,
) -> RenderedRate {
    match client_day_rate(client_rate_thb_day, client_rate_source, None, None) {
        Some(rate) => RenderedRate {
            shape: RateShape::Amount,
            thb_day: Some(rate),
            say: None,
        },
        None => RenderedRate {
            shape: RateShape::Dash,
            thb_day: None,
            say: Some(SAY_HUMAN_QUOTES),
        },
    }
}

/// May a customer-facing price box publish the file's *reference* money — the
/// deposit, the monthly low-season figure, the class-discount percentage?
///
/// Only beside a real door price. Those three are `SOURCE_FILE_REFERENCE` in the
/// spec (:116): retained for audit comparison, never authoritative on their own.
/// Standing beside a door quote they are context. Standing where a price is
/// *absent* they become the price — a customer reading "a manager quotes this
/// price" above "฿9,900 per month" divides by thirty and leaves with an averaged
/// per-day number, which is exactly what D11's `must_not_emit` forbids.
///
/// One predicate, used by both price boxes. The two screens have already drifted
/// once — `money_thb(Some(0.0))` renders a dash in the WASM while
/// `thb_or_dash(Some(0.0))` renders `0 ฿` in the API — so the gate is shared
/// rather than spelled twice (D15).
pub fn may_publish_reference_money(
    client_rate_thb_day: Option<f64>,
    client_rate_source: Option<&str>,
) -> bool {
    client_day_rate(client_rate_thb_day, client_rate_source, None, None).is_some()
}

/// A market's money display profile: the Rust-side instance of the contract in
/// `specs/turbobaby/market_profile.t27` (`turbobaby/market`, D18). The owner's
/// standing instruction is that this agent runs on any market, country and
/// currency; the deployment profile lives in `data/fleet_seed.json` (`market`
/// block), and this struct is the formatter's view of it. A second market is a
/// second instance of this struct — not a second money formatter.
pub struct MarketMoneyFormat {
    /// ISO 4217 alphabetic code (e.g. `THB`). Identifies the profile; the
    /// symbol below is what the customer sees.
    pub currency_code: &'static str,
    /// Currency symbol glyph (e.g. `฿`, U+0E3F). The spec carries this as its
    /// decimal code point so the `.t27` file stays ASCII (L3); the glyph lives
    /// here, where the sources are UTF-8.
    pub symbol: char,
    /// `false` prefixes the symbol (`฿100`), `true` suffixes it (`100€`).
    pub symbol_suffix: bool,
    /// Minor digits to render: 0 renders whole units only (whole-baht
    /// practice), 2 renders cents. Never exceeds the currency's ISO exponent —
    /// the market contract refuses such a profile, and the seed verifier
    /// enforces it against the deployment.
    pub minor_digits_display: u8,
    /// Thousands group separator (`,` for THB).
    pub group_separator: char,
    /// Decimal separator (`.` for THB). Must differ from the group separator;
    /// identical separators make an amount ambiguous.
    pub decimal_separator: char,
}

/// The deployment's market, measured from this repository (2026-09-13): the
/// baht glyph and comma grouping come from `format_baht`, whole-baht display
/// from the cash practice it encodes, and every value mirrors the
/// `specs/turbobaby/market_profile.t27` profile consts and the seed's
/// `market` block.
pub const THB_MARKET: MarketMoneyFormat = MarketMoneyFormat {
    currency_code: "THB",
    symbol: '฿',
    symbol_suffix: false,
    minor_digits_display: 0,
    group_separator: ',',
    decimal_separator: '.',
};

/// Format a money amount under a market profile: clamp NaN/inf/negative to 0,
/// truncate to the profile's minor digits, group the whole units, place the
/// symbol per the profile. Truncation happens on the amount's shortest
/// round-trip decimal form (`format!("{}", f64)`), never on a binary
/// multiply: `1234567.89 * 100.0` is `…88.9999…` in f64, so binary truncation
/// would display a cent the amount visibly has — and rounding is equally
/// wrong for this shop, because whole-baht display pins `350.99 -> 350`.
/// Dropping digits from the decimal the amount displays as is the one rule
/// that keeps both. Whole units go through `i64` (not `i32`) so a
/// large-but-valid total can't saturate at ~2.1B — prices and totals are
/// `i64` in the domain (`ProductPrice.price`, `calculate_cart_total`).
pub fn format_money(amount: f64, market: &MarketMoneyFormat) -> String {
    let sanitized = sanitize_money(amount);
    // Shortest round-trip decimal, e.g. `350.99` (not the exact binary
    // `350.989999999999954525264911353778839111328125`).
    let decimal = format!("{}", sanitized);
    let (whole_str, frac_str) = match decimal.split_once('.') {
        Some((w, f)) => (w, f),
        None => (decimal.as_str(), ""),
    };
    // A whole part beyond i64 keeps the old saturating `as i64` behaviour.
    let whole: i64 = whole_str.parse().unwrap_or(sanitized as i64);
    let mut out = group_digits(whole, market.group_separator);
    if market.minor_digits_display > 0 {
        out.push(market.decimal_separator);
        for i in 0..market.minor_digits_display as usize {
            out.push(frac_str.as_bytes().get(i).copied().unwrap_or(b'0') as char);
        }
    }
    if market.symbol_suffix {
        format!("{}{}", out, market.symbol)
    } else {
        format!("{}{}", market.symbol, out)
    }
}

/// Format a money amount (THB) for customer display: the deployment market's
/// named shortcut, so every existing call site displays the declared market
/// without per-screen edits. Single source of truth: the customer UI
/// previously had three identical `format_price` clones (menu/cart/home) that
/// each narrowed to `i32`.
pub fn format_baht(amount: f64) -> String {
    format_money(amount, &THB_MARKET)
}

/// Group a non-negative integer's digits in threes with the profile's group
/// separator (e.g. `3000000` -> `3,000,000` under THB). `sanitize_money`
/// guarantees the input is `>= 0`, so no sign handling is needed.
fn group_digits(n: i64, sep: char) -> String {
    let digits = n.max(0).to_string();
    let bytes = digits.as_bytes();
    let len = bytes.len();
    let mut out = String::with_capacity(len + len / 3);
    for (i, b) in bytes.iter().enumerate() {
        if i > 0 && (len - i).is_multiple_of(3) {
            out.push(sep);
        }
        out.push(*b as char);
    }
    out
}

// ── Money on a customer's own order (2026-09-22) ──────────────────────────
//    specs/turbobaby/order_presentation.t27 (turbobaby/order-presentation)
//    decides which figure an order card shows; this is the one place both
//    order screens get the answer from, compiled for the host as well as for
//    wasm32 so that the answer is tested (D15). Until that day four of the five
//    figures on those screens went straight into `format_baht`, and absence
//    took four shapes: a dash, a zero, an omitted row and a lost screen.

/// D9's filter for a money figure a stored ORDER carries: finite and `>= 0` is
/// a value; `None`, NaN, infinity and a negative are absent -- never `0.0`.
///
/// Not a second price filter. D15 keeps the honesty filters here "as one pair
/// of functions, not three", and this is the base of that pair:
/// [`published_money`] is written as this filter plus the refusal of zero, so
/// "unusable" is defined once. The two differ at `Some(0.0)` only, and on
/// purpose. A published PRICE of zero reads as free and is refused there. A
/// stored order's zero can be a measurement -- no bonus applied, no stars
/// spent, a total the discounts paid in full (the order-money contract floors
/// its identity at zero) -- and is kept here. The server holds the same rule
/// as `finite_money` in `src/db/orders.rs`; `tests/order_money_wiring.rs`
/// trips if the two stop being one predicate.
pub fn measured_money(amount: Option<f64>) -> Option<f64> {
    amount.filter(|v| v.is_finite() && *v >= 0.0)
}

/// The four money figures of an order, as the order endpoints send them.
///
/// Every figure is an `Option` carrying `#[serde(default)]`, the shape the
/// webapp-bridge contract counts as neither breaking the client when the
/// field is missing nor inventing a value for it. A payload that omits a
/// figure or sends `null` therefore reads as an absence and renders as a dash,
/// instead of failing the whole response. The detail screen flattens this
/// block into its order DTO.
#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize)]
pub struct OrderMoney {
    #[serde(default)]
    pub subtotal: Option<f64>,
    #[serde(default)]
    pub bonus_used: Option<f64>,
    #[serde(default)]
    pub stars_used: Option<i64>,
    #[serde(default)]
    pub total: Option<f64>,
}

/// Every money string an order detail card prints, decided once.
#[derive(Debug, Clone, PartialEq)]
pub struct OrderMoneyText {
    /// The sum of the order's line prices, or the dash.
    pub subtotal: String,
    /// `None` omits the row: no bonus was applied, a measured zero.
    /// `Some(dash)` is an absence, and the row stays to say so.
    pub bonus: Option<String>,
    /// The star discount, a count and not an amount; the same row rule.
    pub stars: Option<String>,
    /// The order's total, or the dash.
    pub total: String,
}

/// The total an order card prints: the stored figure, a measured zero
/// included, or the dash. The orders list prints only this, and the detail
/// card prints its total through it, so the two cannot disagree.
///
/// `dash` is the component layer's glyph (`bike_card::DASH`, the one the line
/// total on the same card already renders), passed in because that module is
/// wasm-only: a second glyph defined here would be a second shape of absence.
pub fn order_total_text(total: Option<f64>, dash: &str) -> String {
    match measured_money(total) {
        Some(amount) => format_baht(amount),
        None => dash.to_string(),
    }
}

/// Every figure on an order detail card. One shape of absence, the dash, and
/// a zero printed only where zero is a measurement:
///
/// * the total keeps a measured zero ([`order_total_text`]);
/// * the subtotal is the sum of the order's line prices, and the line total
///   on the same card renders a zero unit price as absent (D9, through
///   [`published_money`]), so a zero subtotal is a sum of absences and
///   renders as one;
/// * a bonus or star discount of zero omits its row, a positive one prints
///   with a minus sign, and an absent or negative one keeps its row with the
///   dash.
///
/// No sentence travels with these dashes: which one -- if any -- a customer
/// reads beside a dashed order figure is the owner's open question in the
/// order-presentation contract, and nothing publishes one.
pub fn order_money_text(money: &OrderMoney, dash: &str) -> OrderMoneyText {
    let subtotal = match published_money(money.subtotal) {
        Some(amount) => format_baht(amount),
        None => dash.to_string(),
    };
    let bonus = match measured_money(money.bonus_used) {
        Some(amount) if amount > 0.0 => Some(format!("-{}", format_baht(amount))),
        Some(_) => None,
        None => Some(dash.to_string()),
    };
    let stars = match money.stars_used {
        Some(count) if count > 0 => Some(format!("-{count} \u{2b50}")),
        Some(0) => None,
        _ => Some(dash.to_string()),
    };
    OrderMoneyText {
        subtotal,
        bonus,
        stars,
        total: order_total_text(money.total, dash),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // The price slot's own words, for the criterion that is about emitted text
    // rather than about shape. Both modules compile for either arch, so unlike
    // an assertion under `src/ui` this one runs.
    use crate::trios::core::Lang;
    use crate::trios::i18n::{t, T_BIKE_PRICE_ON_REQUEST};

    fn now_utc() -> DateTime<Utc> {
        Utc::now()
    }

    #[test]
    fn authoritative_door_rate_preserves_only_a_valid_door_value() {
        assert_eq!(authoritative_door_rate(Some(704.25)), Some(704.25));
        assert_eq!(authoritative_door_rate(None), None);
        assert_eq!(authoritative_door_rate(Some(0.0)), None);
        assert_eq!(authoritative_door_rate(Some(-1.0)), None);
        assert_eq!(authoritative_door_rate(Some(f64::NAN)), None);
        assert_eq!(authoritative_door_rate(Some(f64::INFINITY)), None);
        assert_eq!(authoritative_door_rate(Some(f64::NEG_INFINITY)), None);
    }

    #[test]
    fn client_day_rate_never_falls_back_to_file_inputs() {
        assert_eq!(
            client_day_rate(None, Some("unavailable"), Some(939.0), Some(0.25)),
            None
        );
        assert_eq!(
            client_day_rate(Some(704.25), Some("door"), Some(939.0), Some(0.25)),
            Some(704.25)
        );
        assert_eq!(
            client_day_rate(Some(704.25), None, Some(939.0), Some(0.25)),
            None
        );
        assert_eq!(
            client_day_rate(Some(704.25), Some("unavailable"), Some(939.0), Some(0.25),),
            None
        );
        assert_eq!(
            client_day_rate(Some(f64::NAN), Some("door"), Some(939.0), Some(0.25)),
            None
        );
    }

    #[test]
    fn a_dash_never_travels_alone() {
        // The spec's invariant the_dash_never_travels_alone (pricing_honesty.t27).
        // D11 lists silence in `must_not_emit` beside invention, so a render
        // that emits no number must emit a sentence.
        for silent in [
            render_client_rate(None, Some("unavailable")),
            render_client_rate(None, None),
            render_client_rate(Some(704.25), Some("unavailable")),
            render_client_rate(Some(704.25), None),
            render_client_rate(Some(0.0), Some("door")),
            render_client_rate(Some(-1.0), Some("door")),
            render_client_rate(Some(f64::NAN), Some("door")),
        ] {
            assert_eq!(silent.shape, RateShape::Dash);
            assert_eq!(silent.thb_day, None);
            assert_eq!(silent.say, Some(SAY_HUMAN_QUOTES));
        }
    }

    #[test]
    fn a_door_quote_renders_as_an_amount_that_speaks_for_itself() {
        let quoted = render_client_rate(Some(337.0), Some("door"));
        assert_eq!(quoted.shape, RateShape::Amount);
        assert_eq!(quoted.thb_day, Some(337.0));
        // SAY_NOTHING_EXTRA: the number needs no accompanying sentence.
        assert_eq!(quoted.say, None);
    }

    #[test]
    fn reference_money_may_be_published_only_beside_a_door_price() {
        // The file's deposit / monthly / class-discount figures are context
        // beside a real quote and a price in the absence of one.
        assert!(may_publish_reference_money(Some(337.0), Some("door")));
        assert!(!may_publish_reference_money(None, Some("unavailable")));
        assert!(!may_publish_reference_money(None, None));
        // A file number wearing no door provenance buys nothing.
        assert!(!may_publish_reference_money(Some(939.0), Some("file")));
        assert!(!may_publish_reference_money(Some(939.0), None));
        // Invalid money is not a price, so it licenses nothing either.
        assert!(!may_publish_reference_money(Some(0.0), Some("door")));
        assert!(!may_publish_reference_money(Some(f64::NAN), Some("door")));
    }

    #[test]
    fn published_money_is_the_door_filter_under_another_name() {
        // D15: one implementation. If these two ever disagree, a price the
        // catalog calls absent is a price the cart calls real.
        for probe in [
            Some(449.0),
            Some(0.0),
            Some(-1.0),
            Some(f64::NAN),
            Some(f64::INFINITY),
            Some(f64::NEG_INFINITY),
            None,
        ] {
            assert_eq!(published_money(probe), authoritative_door_rate(probe));
        }
        assert_eq!(published_money(Some(449.0)), Some(449.0));
        assert_eq!(published_money(Some(0.0)), None);
    }

    #[test]
    fn a_cart_line_with_no_price_is_worth_nothing_knowable() {
        assert_eq!(cart_line_total(Some(449.0), 3), Some(1347.0));
        // A quantity of zero is a measured zero, not an absence: the line's
        // price is known and the customer asked for none of it.
        assert_eq!(cart_line_total(Some(449.0), 0), Some(0.0));
        // The four shapes of "nobody priced this". A `0.0` that survived the
        // wire is in the list: before 2026-09-21 it rendered `฿0` on the line
        // and added `0` to the total, so the free item and the total agreed
        // with each other and with nothing else.
        assert_eq!(cart_line_total(None, 3), None);
        assert_eq!(cart_line_total(Some(0.0), 3), None);
        assert_eq!(cart_line_total(Some(-449.0), 3), None);
        assert_eq!(cart_line_total(Some(f64::NAN), 3), None);
    }

    #[test]
    fn one_unpriced_line_makes_the_whole_cart_total_unknown() {
        assert_eq!(
            cart_total([(Some(449.0), 2), (Some(120.0), 1)]),
            Some(1018.0)
        );
        assert_eq!(cart_total(std::iter::empty()), Some(0.0));

        // The defect this function exists to prevent: the priced lines still
        // sum to 898, and showing 898 would tell the customer their cart costs
        // 898 baht. It does not — nobody knows what it costs.
        assert_eq!(cart_total([(Some(449.0), 2), (None, 1)]), None);
        assert_eq!(cart_total([(None, 1), (Some(449.0), 2)]), None);

        // `src/ui/api/http.rs:445` records a non-finite unit_price reaching
        // recalculate_total and making the whole total NaN. NaN formats as a
        // zero through `sanitize_money`, so it read as a free cart.
        assert_eq!(cart_total([(Some(f64::NAN), 1)]), None);
        assert_eq!(cart_total([(Some(f64::INFINITY), 1)]), None);
        assert_eq!(cart_total([(Some(-1.0), 1)]), None);

        // A zero-priced line is unpriced, not free: same rule as the line
        // helper, so the two cannot drift.
        assert_eq!(cart_total([(Some(0.0), 1)]), None);
    }

    #[test]
    fn an_untotalled_cart_still_says_something() {
        // D11 lists silence beside invention in `must_not_emit`. A cart with no
        // total renders through the same dash-plus-sentence pair as a bike with
        // no rate, so the customer is never shown an empty space where a price
        // belongs.
        let silent = render_client_rate(None, Some("door"));
        assert_eq!(silent.shape, RateShape::Dash);
        assert_eq!(silent.say, Some(SAY_HUMAN_QUOTES));
        assert!(!t(Lang::Russian, T_BIKE_PRICE_ON_REQUEST).is_empty());
        assert!(!t(Lang::English, T_BIKE_PRICE_ON_REQUEST).is_empty());
    }

    /// #25's second acceptance criterion: `от` / "from" / an averaged number is
    /// never emitted *as a price*. The scope is the price slot and nothing
    /// wider, and that scoping is what makes the criterion provable at all —
    /// the shipped `T_BIKE_QUOTE_NOTE` reads "…она зависит **от** срока, сезона
    /// и наличия", correct copy that a whole-screen search for `от` would
    /// condemn. That note stands outside the slot and is not tested here.
    ///
    /// The proof is by absence of numerals rather than by a blocklist of words.
    /// A "from" price, a range and an average each need a numeral; where none
    /// can reach the slot, none of the three is expressible in it whatever
    /// words surround it. It is also why the em dash in the Russian wording is
    /// harmless: a dash is a range only when a number stands on each side.
    #[test]
    fn a_rendered_absent_rate_carries_no_from_no_range_and_no_average() {
        let dash = render_client_rate(None, Some("unavailable"));
        assert_eq!(dash.shape, RateShape::Dash);
        // No caller can format a numeral out of a slot it cannot read.
        assert_eq!(dash.thb_day, None);
        assert!(!SAY_HUMAN_QUOTES.chars().any(char::is_numeric));

        // The text that actually stands in the slot, in every locale the
        // dispatcher serves. A translator writing "от 300฿" is the realistic
        // way this criterion gets broken, and `is_numeric` rather than
        // `is_ascii_digit` because Thai ๐–๙ would spend a price just as well.
        for lang in [
            Lang::Russian,
            Lang::English,
            Lang::Thai,
            Lang::Chinese,
            Lang::Hebrew,
            Lang::German,
            Lang::French,
            Lang::Spanish,
        ] {
            let slot = t(lang, T_BIKE_PRICE_ON_REQUEST);
            assert!(
                !slot.chars().any(char::is_numeric),
                "{lang:?}: the on-request line carries a numeral ({slot:?}) — \
                 that is a price with no door behind it (D11)"
            );
        }
    }

    #[test]
    fn the_reference_gate_and_the_render_agree_on_every_input() {
        // Two functions, one door gate (D15). If these ever disagree, one of
        // them has grown a second opinion about what a price is.
        for (rate, source) in [
            (Some(337.0), Some("door")),
            (Some(337.0), Some("unavailable")),
            (Some(0.0), Some("door")),
            (Some(-1.0), Some("door")),
            (Some(f64::INFINITY), Some("door")),
            (None, Some("door")),
            (None, None),
        ] {
            assert_eq!(
                may_publish_reference_money(rate, source),
                render_client_rate(rate, source).shape == RateShape::Amount,
                "disagreement on ({rate:?}, {source:?})"
            );
        }
    }

    #[test]
    fn test_format_baht_basic_and_clamps() {
        assert_eq!(format_baht(350.0), "฿350");
        assert_eq!(format_baht(350.99), "฿350"); // whole-baht (truncates)
        assert_eq!(format_baht(0.0), "฿0");
        // NaN / inf / negative clamp to 0.
        assert_eq!(format_baht(f64::NAN), "฿0");
        assert_eq!(format_baht(f64::INFINITY), "฿0");
        assert_eq!(format_baht(-5.0), "฿0");
    }

    #[test]
    fn test_format_baht_large_value_does_not_saturate_like_i32() {
        // A total above i32::MAX (~2.1B) must render its real value, not the
        // saturated 2147483647 the old `as i32` clones produced.
        let big = 3_000_000_000.0_f64; // > i32::MAX
        assert_eq!(format_baht(big), "฿3,000,000,000");
    }

    /// The proof profile from specs/turbobaby/market_profile.t27 (PROOF_*
    /// consts): a two-minor-digit, suffix-symbol, dot-grouped market. No shop
    /// exists at this profile — it exists so the formatter is proven not
    /// THB-shaped, exactly as the spec's own tests prove the contract.
    fn eur_proof_market() -> MarketMoneyFormat {
        MarketMoneyFormat {
            currency_code: "EUR",
            symbol: '€',
            symbol_suffix: true,
            minor_digits_display: 2,
            group_separator: '.',
            decimal_separator: ',',
        }
    }

    #[test]
    fn a_two_decimal_market_renders_its_own_separators_and_suffix() {
        assert_eq!(
            format_money(1234567.89, &eur_proof_market()),
            "1.234.567,89€"
        );
        assert_eq!(format_money(0.5, &eur_proof_market()), "0,50€");
        assert_eq!(format_money(0.0, &eur_proof_market()), "0,00€");
        // Truncation, not rounding: display never invents a higher price.
        assert_eq!(format_money(2.999, &eur_proof_market()), "2,99€");
        assert_eq!(format_money(-1.0, &eur_proof_market()), "0,00€");
    }

    #[test]
    fn a_zero_minor_digit_market_groups_without_a_decimal_separator() {
        // A whole-unit non-THB market (JPY-shaped): no fraction, and the
        // decimal separator must not appear just because the profile has one.
        let jpy_shaped = MarketMoneyFormat {
            currency_code: "JPY",
            symbol: '¥',
            symbol_suffix: false,
            minor_digits_display: 0,
            group_separator: ',',
            decimal_separator: '.',
        };
        assert_eq!(format_money(1234567.0, &jpy_shaped), "¥1,234,567");
        assert_eq!(format_money(1234567.89, &jpy_shaped), "¥1,234,567");
    }

    #[test]
    fn the_deployment_market_is_thb_and_its_shortcut_agrees() {
        assert_eq!(THB_MARKET.currency_code, "THB");
        assert_eq!(format_baht(1500.0), format_money(1500.0, &THB_MARKET));
        assert_eq!(format_baht(1500.0), "฿1,500");
    }

    #[test]
    fn test_format_baht_groups_thousands() {
        assert_eq!(format_baht(0.0), "฿0");
        assert_eq!(format_baht(999.0), "฿999");
        assert_eq!(format_baht(1000.0), "฿1,000");
        assert_eq!(format_baht(12345.0), "฿12,345");
        assert_eq!(format_baht(1_234_567.0), "฿1,234,567");
    }

    #[test]
    fn test_group_digits_boundaries() {
        // THB's comma grouping; the separators themselves are profile fields.
        assert_eq!(group_digits(0, ','), "0");
        assert_eq!(group_digits(100, ','), "100");
        assert_eq!(group_digits(1000, ','), "1,000");
        assert_eq!(group_digits(1_000_000, ','), "1,000,000");
        // The same digits under the EUR proof profile's group separator.
        assert_eq!(group_digits(1_000_000, '.'), "1.000.000");
    }

    fn base_flags<'a>() -> MarketingFlags<'a> {
        MarketingFlags {
            price_per_gram: 350.0,
            is_strain_of_day: false,
            strain_of_day_discount: 0.0,
            sale_active: false,
            sale_until: None,
            sale_price: None,
            discount_percent: 0.0,
            is_new_arrival: false,
            new_until: None,
        }
    }

    // ── is_active_until ────────────────────────────────────────────

    #[test]
    fn active_until_none_is_open_ended() {
        assert!(is_active_until(None, now_utc()));
    }

    #[test]
    fn active_until_empty_string_is_open_ended() {
        assert!(is_active_until(Some(""), now_utc()));
    }

    #[test]
    fn active_until_future_passes() {
        // 1h in the future is plenty regardless of clock skew at test time.
        let in_future = (Utc::now() + chrono::Duration::hours(1)).to_rfc3339();
        assert!(is_active_until(Some(&in_future), Utc::now()));
    }

    #[test]
    fn active_until_past_fails() {
        let in_past = (Utc::now() - chrono::Duration::hours(1)).to_rfc3339();
        assert!(!is_active_until(Some(&in_past), Utc::now()));
    }

    #[test]
    fn active_until_malformed_is_permissive() {
        // Better to keep the badge visible than to misrepresent the catalog
        // due to a typo in an admin timestamp.
        assert!(is_active_until(Some("not-a-date"), now_utc()));
    }

    // ── effective_strain_price ─────────────────────────────────────

    #[test]
    fn no_flags_returns_base() {
        let priced = effective_strain_price(&base_flags(), now_utc());
        assert!((priced.price - 350.0).abs() < 1e-9);
        assert!(!priced.has_discount);
    }

    #[test]
    fn sotd_with_discount_wins_over_sale() {
        let mut f = base_flags();
        f.is_strain_of_day = true;
        f.strain_of_day_discount = 30.0;
        // Sale below should be ignored.
        f.sale_active = true;
        f.sale_price = Some(50.0);
        let priced = effective_strain_price(&f, now_utc());
        // 350 * 0.7 = 245
        assert!((priced.price - 245.0).abs() < 1e-9);
        assert!(priced.has_discount);
    }

    #[test]
    fn sotd_without_discount_falls_through() {
        // Marking SOTD without setting a discount must NOT lower the price —
        // SOTD is a visibility flag in that case, not a discount.
        let mut f = base_flags();
        f.is_strain_of_day = true;
        f.strain_of_day_discount = 0.0;
        let priced = effective_strain_price(&f, now_utc());
        assert!((priced.price - 350.0).abs() < 1e-9);
        assert!(!priced.has_discount);
    }

    #[test]
    fn sale_price_overrides_discount_percent() {
        let mut f = base_flags();
        f.sale_active = true;
        f.sale_price = Some(199.0);
        f.discount_percent = 50.0; // would give 175 — but sale_price wins
        let priced = effective_strain_price(&f, now_utc());
        assert!((priced.price - 199.0).abs() < 1e-9);
        assert!(priced.has_discount);
    }

    #[test]
    fn sale_price_ignored_when_not_strictly_lower() {
        // Admin typo: sale_price = base price. UI must not display a fake
        // "discount" that doesn't actually lower the price.
        let mut f = base_flags();
        f.sale_active = true;
        f.sale_price = Some(350.0);
        f.discount_percent = 0.0;
        let priced = effective_strain_price(&f, now_utc());
        assert!((priced.price - 350.0).abs() < 1e-9);
        assert!(!priced.has_discount);
    }

    #[test]
    fn sale_uses_discount_percent_when_no_sale_price() {
        let mut f = base_flags();
        f.sale_active = true;
        f.discount_percent = 25.0;
        let priced = effective_strain_price(&f, now_utc());
        // 350 * 0.75 = 262.5
        assert!((priced.price - 262.5).abs() < 1e-9);
        assert!(priced.has_discount);
    }

    #[test]
    fn expired_sale_falls_through_to_base() {
        let in_past = (Utc::now() - chrono::Duration::hours(1)).to_rfc3339();
        let mut f = base_flags();
        f.sale_active = true;
        f.sale_until = Some(&in_past);
        f.sale_price = Some(50.0);
        let priced = effective_strain_price(&f, now_utc());
        assert!((priced.price - 350.0).abs() < 1e-9);
        assert!(!priced.has_discount);
    }

    #[test]
    fn inactive_sale_flag_ignores_sale_price() {
        let mut f = base_flags();
        f.sale_active = false; // explicitly off
        f.sale_price = Some(50.0);
        f.discount_percent = 50.0;
        let priced = effective_strain_price(&f, now_utc());
        assert!((priced.price - 350.0).abs() < 1e-9);
        assert!(!priced.has_discount);
    }

    #[test]
    fn nan_base_price_sanitized_to_zero() {
        // Defensive: a row with corrupted price_per_gram must not produce NaN
        // downstream (NaN propagates through DB UPDATEs and breaks aggregates).
        let mut f = base_flags();
        f.price_per_gram = f64::NAN;
        let priced = effective_strain_price(&f, now_utc());
        assert_eq!(priced.price, 0.0);
    }

    #[test]
    fn discount_percent_over_100_is_clamped_not_negative() {
        // Admin typo: enters 150%. Effective price clamps to 0, never negative.
        let mut f = base_flags();
        f.sale_active = true;
        f.discount_percent = 150.0;
        let priced = effective_strain_price(&f, now_utc());
        assert_eq!(priced.price, 0.0);
    }

    // ── Other catalogs ────────────────────────────────────────────

    #[test]
    fn accessory_price_sanitises_negatives_and_nan() {
        assert!((effective_accessory_price(150.0) - 150.0).abs() < 1e-9);
        assert_eq!(effective_accessory_price(-5.0), 0.0);
        assert_eq!(effective_accessory_price(f64::NAN), 0.0);
    }

    #[test]
    fn tea_price_uses_same_sanitisation_as_accessory() {
        // The two helpers are intentionally separate symbols (admin reading
        // call sites benefits from explicit catalog naming) but must behave
        // identically — pin that here.
        for &v in &[0.0, 50.0, -1.0, f64::INFINITY] {
            assert_eq!(effective_accessory_price(v), effective_tea_price(v));
        }
    }

    #[test]
    fn set_price_applies_percentage_discount() {
        // 500 baht with 20% off = 400.
        assert!((effective_set_price(500.0, 20.0) - 400.0).abs() < 1e-9);
    }

    #[test]
    fn set_price_clamps_oversized_discount_to_zero_floor() {
        // Admin typo: enters 200%. Price clamps to 0, never goes negative.
        assert_eq!(effective_set_price(500.0, 200.0), 0.0);
    }

    #[test]
    fn set_price_handles_zero_discount() {
        assert!((effective_set_price(500.0, 0.0) - 500.0).abs() < 1e-9);
    }

    // ── Order money on a customer's own order screens (2026-09-22) ─────────
    //    specs/turbobaby/order_presentation.t27, defects 2 and 6. The two
    //    screens compile only for wasm32; the rule they call lives above, so
    //    this is where it is tested.

    /// The glyph the screens hand in (`bike_card::DASH`, wasm-only).
    const TEST_DASH: &str = "\u{2014}";

    /// Every shape an unusable figure can take once it is an `Option`.
    const UNUSABLE: [Option<f64>; 5] = [
        None,
        Some(-5.0),
        Some(f64::NAN),
        Some(f64::INFINITY),
        Some(f64::NEG_INFINITY),
    ];

    fn order(
        subtotal: Option<f64>,
        bonus_used: Option<f64>,
        stars_used: Option<i64>,
        total: Option<f64>,
    ) -> OrderMoney {
        OrderMoney {
            subtotal,
            bonus_used,
            stars_used,
            total,
        }
    }

    #[test]
    fn measured_money_keeps_a_measured_zero_and_drops_every_absence() {
        assert_eq!(measured_money(Some(0.0)), Some(0.0));
        assert_eq!(measured_money(Some(449.0)), Some(449.0));
        for absent in UNUSABLE {
            assert_eq!(measured_money(absent), None, "{absent:?} must stay absent");
        }
    }

    #[test]
    fn the_price_filter_is_the_order_filter_minus_zero() {
        // D15: one definition of "unusable", and one extra rule for a PRICE.
        for probe in [
            Some(449.0),
            Some(0.5),
            Some(0.0),
            Some(-1.0),
            Some(f64::NAN),
            Some(f64::INFINITY),
            Some(f64::NEG_INFINITY),
            None,
        ] {
            assert_eq!(
                published_money(probe),
                measured_money(probe).filter(|v| *v > 0.0),
                "{probe:?}"
            );
        }
        // They disagree at exactly one input, and that input is the point.
        assert_eq!(measured_money(Some(0.0)), Some(0.0));
        assert_eq!(published_money(Some(0.0)), None);
    }

    #[test]
    fn an_order_money_block_missing_a_figure_is_absent_and_not_an_error() {
        let empty: OrderMoney =
            serde_json::from_str("{}").expect("a block with no figures is four absences");
        assert_eq!(empty, OrderMoney::default());
        let nulls: OrderMoney = serde_json::from_str(
            r#"{"subtotal":null,"bonus_used":null,"stars_used":null,"total":null}"#,
        )
        .expect("a null figure is an absence");
        assert_eq!(nulls, OrderMoney::default());
        let full: OrderMoney = serde_json::from_str(
            r#"{"subtotal":500,"bonus_used":30.5,"stars_used":70,"total":399.5}"#,
        )
        .expect("integer JSON numbers read into the f64 figures");
        assert_eq!(full, order(Some(500.0), Some(30.5), Some(70), Some(399.5)));
    }

    #[test]
    fn a_flattened_money_block_survives_a_payload_missing_its_total() {
        // The detail screen's DTO shape: the block flattened into the order.
        #[derive(Debug, serde::Deserialize)]
        struct DetailShape {
            id: String,
            #[serde(flatten)]
            money: OrderMoney,
            status: String,
        }
        let detail: DetailShape = serde_json::from_str(r#"{"id":"a","subtotal":100,"status":"b"}"#)
            .expect("a missing total is an absence, not a lost screen");
        assert_eq!((detail.id.as_str(), detail.status.as_str()), ("a", "b"));
        assert_eq!(detail.money, order(Some(100.0), None, None, None));

        // The list screen's DTO shape.
        #[derive(Debug, serde::Deserialize)]
        struct ListShape {
            #[serde(default)]
            total: Option<f64>,
        }
        let row: ListShape = serde_json::from_str("{}").expect("one order without a total");
        assert_eq!(row.total, None);

        // The shape both screens had until 2026-09-22: a bare number. The same
        // payload fails, which is the screen a customer lost.
        #[derive(Debug, serde::Deserialize)]
        #[allow(dead_code)]
        struct Before {
            total: f64,
        }
        assert!(serde_json::from_str::<Before>("{}").is_err());
    }

    #[test]
    fn absence_has_one_shape_on_every_order_figure() {
        for absent in UNUSABLE {
            let shown = order_money_text(&order(absent, absent, None, absent), TEST_DASH);
            assert_eq!(shown.subtotal, TEST_DASH, "subtotal {absent:?}");
            assert_eq!(shown.bonus.as_deref(), Some(TEST_DASH), "bonus {absent:?}");
            assert_eq!(shown.stars.as_deref(), Some(TEST_DASH), "stars absent");
            assert_eq!(shown.total, TEST_DASH, "total {absent:?}");
            assert_eq!(order_total_text(absent, TEST_DASH), TEST_DASH);
        }
        // A star count below zero is not a count anybody spent.
        let negative = order_money_text(
            &order(Some(100.0), Some(0.0), Some(-3), Some(100.0)),
            TEST_DASH,
        );
        assert_eq!(negative.stars.as_deref(), Some(TEST_DASH));
    }

    #[test]
    fn no_order_figure_reaches_the_zero_fallback() {
        // format_baht turns NaN, infinity and a negative into 0.0 through
        // sanitize_money; none of them may arrive there from an order card.
        let zero = format_baht(0.0);
        for absent in UNUSABLE {
            assert_ne!(order_total_text(absent, TEST_DASH), zero, "{absent:?}");
            let shown = order_money_text(&order(absent, absent, Some(-1), absent), TEST_DASH);
            for text in [
                Some(shown.subtotal),
                shown.bonus,
                shown.stars,
                Some(shown.total),
            ]
            .into_iter()
            .flatten()
            {
                assert!(
                    !text.contains(&zero),
                    "{absent:?} rendered {text:?}: an absence printed as a price"
                );
            }
        }
    }

    #[test]
    fn a_measured_zero_total_stays_a_zero_and_a_zero_discount_omits_its_row() {
        // Paid in full by the discounts: order-money floors its identity at
        // zero (TOTAL_FLOOR_MINOR), so this zero is a measurement.
        let paid = order_money_text(
            &order(Some(100.0), Some(30.0), Some(70), Some(0.0)),
            TEST_DASH,
        );
        assert_eq!(paid.subtotal, "\u{0e3f}100");
        assert_eq!(paid.bonus.as_deref(), Some("-\u{0e3f}30"));
        assert_eq!(paid.stars.as_deref(), Some("-70 \u{2b50}"));
        assert_eq!(paid.total, "\u{0e3f}0");
        assert_eq!(order_total_text(Some(0.0), TEST_DASH), format_baht(0.0));

        // No bonus and no stars spent: measured zeros, so the rows are
        // omitted -- never dashed, which would call a real "none" unknown.
        let plain = order_money_text(
            &order(Some(100.0), Some(0.0), Some(0), Some(100.0)),
            TEST_DASH,
        );
        assert_eq!(plain.bonus, None);
        assert_eq!(plain.stars, None);
        assert_eq!(plain.total, "\u{0e3f}100");
    }

    #[test]
    fn a_zero_subtotal_is_a_sum_of_absent_prices_and_renders_as_one() {
        // The subtotal is the sum of the order's lines, and the line total on
        // the same card renders a zero unit price as a dash (published_money).
        // A zero sum is therefore theirs: a dash, never a free order.
        let shown = order_money_text(&order(Some(0.0), Some(0.0), Some(0), Some(0.0)), TEST_DASH);
        assert_eq!(shown.subtotal, TEST_DASH);
        assert_eq!(cart_line_total(Some(0.0), 1), None);
        // Measured subtotals still print.
        let priced = order_money_text(&order(Some(0.5), None, None, None), TEST_DASH);
        assert_eq!(priced.subtotal, format_baht(0.5));
    }

    #[test]
    fn the_list_total_and_the_detail_total_are_one_decision() {
        for probe in [Some(0.0), Some(1234.0), Some(350.99)]
            .into_iter()
            .chain(UNUSABLE)
        {
            assert_eq!(
                order_total_text(probe, TEST_DASH),
                order_money_text(&order(None, None, None, probe), TEST_DASH).total,
                "{probe:?}"
            );
        }
    }

    #[test]
    fn a_bike_only_order_prints_its_stored_total_while_the_owner_decides() {
        // The stored sums leave a rental's money out (src/api/orders.rs:495-503),
        // so a bike-only order stores 0 and 0. Which figure such an order should
        // show is an OPEN owner question in order_presentation.t27; until it is
        // answered nothing here decides it: the stored total prints as stored,
        // and the subtotal's zero is a dash like any other zero subtotal. Since
        // 2026-09-23 the server sends null, not a substituted zero, for a stored
        // figure it cannot use (src/db/orders.rs, Order::from): a zero is stored.
        let bike_only =
            order_money_text(&order(Some(0.0), Some(0.0), Some(0), Some(0.0)), TEST_DASH);
        assert_eq!(bike_only.total, format_baht(0.0));
        assert_eq!(bike_only.subtotal, TEST_DASH);
        assert_eq!((bike_only.bonus, bike_only.stars), (None, None));
    }
}
