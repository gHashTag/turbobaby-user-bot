//! Source-level guard on what a price box may publish when there is no price.
//!
//! `src/lib.rs` gates `pub mod ui;` on `#[cfg(target_arch = "wasm32")]`, so
//! nothing under `src/ui` is compiled by `cargo test` — an assertion written
//! beside either screen would never run. That is how the six assertions in
//! `catalog_screen.rs` came to state issue #25's acceptance criteria and prove
//! nothing; their own module doc admits it. These checks read the two price
//! boxes as text instead, which is the pattern `customer_surface_wiring.rs`
//! already uses and CI already executes.
//!
//! What they guard (DECISIONS.md D11): the deposit, the monthly low-season
//! figure and the class-discount percentage come from `data/fleet_seed.json`,
//! not from the door. Beside a door price they are context. Where a price is
//! absent they become the price — ฿9,900 a month divided by thirty is an
//! averaged per-day number, and D11's `must_not_emit` names an average
//! outright. Its fifth entry is *silence*, so the human-quote line must survive
//! the same gate that removes the figures.

const BIKE_DETAIL: &str = include_str!("../src/ui/screens/bike_detail.rs");
const CATALOG: &str = include_str!("../src/ui/screens/catalog_screen.rs");

/// The two source files and the label each uses for the price box, so a failure
/// names the screen rather than an offset.
const PRICE_BOXES: [(&str, &str); 2] = [
    ("src/ui/screens/bike_detail.rs", BIKE_DETAIL),
    ("src/ui/screens/catalog_screen.rs", CATALOG),
];

/// Every token that renders file-sourced reference money. `discount_line` is
/// the binding, not a translation key, because the percentage reaches the DOM
/// through it.
const REFERENCE_MONEY_TOKENS: [&str; 3] = [
    "T_BIKE_MONTHLY_LOW_SEASON",
    "T_BIKE_DEPOSIT",
    "discount_line",
];

/// The `rsx!` region of a file: from the first `rsx! {` to the end. Everything
/// before it is imports and helpers, where naming a token cannot render it.
fn render_region<'a>(source: &'a str, label: &str) -> &'a str {
    let start = source
        .find("rsx! {")
        .unwrap_or_else(|| panic!("{label}: no rsx! block"));
    &source[start..]
}

/// The whole source line an offset falls on.
fn line_at(source: &str, index: usize) -> &str {
    let start = source[..index].rfind('\n').map_or(0, |nl| nl + 1);
    let end = source[index..]
        .find('\n')
        .map_or(source.len(), |nl| index + nl);
    &source[start..end]
}

/// Does this line name the token without rendering it? A `let` binding
/// computes the value, a `use` line imports the key, a comment discusses it.
/// None of the three puts a figure in front of a customer.
fn is_not_a_render_site(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with("let ")
        || trimmed.starts_with("use ")
        || trimmed.starts_with("//")
        || trimmed.starts_with('*')
}

#[test]
fn a_price_box_with_no_price_publishes_no_reference_money() {
    for (label, source) in PRICE_BOXES {
        let rendered = render_region(source, label);
        for token in REFERENCE_MONEY_TOKENS {
            for (index, _) in rendered.match_indices(token) {
                if is_not_a_render_site(line_at(rendered, index)) {
                    continue;
                }
                // Walk back to the gate that must dominate this render site.
                // Anchoring on `may_show_reference` rather than on indentation
                // is deliberate: `customer_surface_wiring.rs` pins a literal
                // block of leading spaces and goes red on any reindentation.
                let preceding = &rendered[..index];
                assert!(
                    preceding.contains("may_show_reference.then("),
                    "{label}: `{token}` renders without a may_show_reference gate \
                     above it — file reference money must not stand where a door \
                     price is absent (D11)"
                );
            }
        }
    }
}

#[test]
fn both_price_boxes_use_the_shared_predicate() {
    for (label, source) in PRICE_BOXES {
        assert!(
            source.contains("may_publish_reference_money("),
            "{label}: must call the shared predicate in trios::pricing, not \
             re-derive the door gate locally (D15)"
        );
        // One door gate. A second inline provenance comparison on either screen
        // is how these two last drifted apart over what a zero means.
        assert!(
            !source.contains("client_rate_source =="),
            "{label}: compares client_rate_source inline — route it through \
             trios::pricing instead (D15)"
        );
    }
}

#[test]
fn the_human_quote_line_is_never_conditional_on_the_reference_gate() {
    // D11 forbids silence as firmly as invention: `must_not_emit` has five
    // entries and the fifth is "silence". A change that gated the say-line
    // along with the figures would be a D11 violation wearing a D11 fix's
    // clothes, so the say-line must appear before the first gate.
    for (label, source) in PRICE_BOXES {
        let rendered = render_region(source, label);
        let say = rendered
            .find("T_BIKE_PRICE_ON_REQUEST")
            .unwrap_or_else(|| panic!("{label}: no human-quote line in the price box"));
        let gate = rendered
            .find("may_show_reference.then(")
            .unwrap_or_else(|| panic!("{label}: no may_show_reference gate"));
        assert!(
            say < gate,
            "{label}: the human-quote line sits inside the reference-money gate \
             — a silent door would then emit nothing at all"
        );
    }
}
