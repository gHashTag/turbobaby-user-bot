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

/// Keep only a client-facing day rate returned by the authoritative door.
///
/// Absence and invalid money stay absent. This helper deliberately accepts no
/// base tariff or class discount, so a silent door has no path to a computed
/// client price (D11) and the same boundary is shared by native and WASM code
/// (D15).
pub fn authoritative_door_rate(client_rate_thb_day: Option<f64>) -> Option<f64> {
    client_rate_thb_day.filter(|rate| rate.is_finite() && *rate > 0.0)
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
        // The spec's invariant of the same name (pricing_honesty.t27:398).
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
}
