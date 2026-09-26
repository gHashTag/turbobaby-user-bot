//! The money on a customer's own order, as the two order screens render it --
//! guarded as text, because the host cannot compile those screens.
//!
//! Contract: `specs/turbobaby/order_presentation.t27`
//! (turbobaby/order-presentation), the figures section: defects 2 and 6 of
//! `docs/t27-handover.md`. Measured 2026-09-21 and still true at 14b01ac: of
//! the five money figures on these screens one went through the dash filter
//! and four went straight into `format_baht`, and absence took four shapes --
//! a dash, a zero, an omitted row and a lost screen -- depending on which field
//! went missing rather than on any rule.
//!
//! `src/lib.rs` gates `pub mod ui;` on wasm32, so `cargo test` compiles nothing
//! under `src/ui`. The RULE -- which figure is absent, and what an absent one
//! looks like -- is `order_money_text` / `order_total_text` in
//! `src/trios/pricing.rs`, which the host compiles and tests. What this file
//! guards is that both screens reach their figures through that rule and
//! decide nothing about a figure on their own. Every check reads CODE lines
//! only: these files explain removed defects in comments on purpose, and prose
//! about a defect is not the defect.
//!
//! This file does NOT import the rule on purpose. It compiles against any tree,
//! so on a tree where the screens still format bare numbers it fails by
//! CONTENT, which is the failure a reviewer needs to see.

use std::fs;
use std::path::{Path, PathBuf};

const DETAIL_PATH: &str = "src/ui/screens/order_detail_screen.rs";
const LIST_PATH: &str = "src/ui/screens/orders_screen.rs";
const SHARED_RULE_PATH: &str = "src/trios/pricing.rs";
const SERVER_ROW_PATH: &str = "src/db/orders.rs";
const CONTRACT: &str = "specs/turbobaby/order_presentation.t27";

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

/// A tracked file, normalised to LF (a Windows checkout hands the reader CRLF,
/// and an anchor ending in a newline would then match nothing). A missing file
/// is a failure, not an empty string.
fn source(relative: &str) -> String {
    fs::read_to_string(repo_root().join(relative))
        .unwrap_or_else(|e| panic!("{relative} must exist for this guard to mean anything: {e}"))
        .replace("\r\n", "\n")
}

/// Code lines only, each with its 1-based line number.
fn code_lines(body: &str) -> Vec<(usize, &str)> {
    body.lines()
        .enumerate()
        .filter(|(_, l)| {
            let t = l.trim_start();
            !(t.starts_with("//") || t.starts_with("/*") || t.starts_with('*'))
        })
        .map(|(i, l)| (i + 1, l))
        .collect()
}

/// The code lines of `struct NAME { ... }`, trimmed, or a panic naming what vanished.
fn struct_code(body: &str, name: &str, label: &str) -> Vec<String> {
    let head = format!("struct {name} {{");
    let start = body
        .find(&head)
        .unwrap_or_else(|| panic!("{label}: `{head}` is gone -- the DTO this guard reads moved"));
    let rest = &body[start + head.len()..];
    let end = rest
        .find("\n}")
        .unwrap_or_else(|| panic!("{label}: `{name}` never closes"));
    code_lines(&rest[..end])
        .into_iter()
        .map(|(_, l)| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect()
}

/// Does `first` appear as a code line immediately followed by the code line `second`?
fn adjacent(lines: &[String], first: &str, second: &str) -> bool {
    lines.windows(2).any(|w| w[0] == first && w[1] == second)
}

#[test]
fn the_scan_reads_both_order_screens_and_finds_their_money() {
    let detail = source(DETAIL_PATH);
    let list = source(LIST_PATH);
    for (label, body) in [(DETAIL_PATH, &detail), (LIST_PATH, &list)] {
        let n = body.lines().count();
        assert!(
            n >= 300,
            "{label}: read only {n} lines -- a truncated read passes every check below"
        );
    }
    assert!(
        detail.contains("fn OrderDetailCard("),
        "the detail card is gone"
    );
    assert!(
        list.contains("pub fn OrdersScreen("),
        "the orders list is gone"
    );
    // The witnesses the zero-counts below lean on (D16): a screen that stopped
    // printing money at all would otherwise pass "no bare figure" trivially.
    let calls = |body: &str, needle: &str| code_lines(body).iter().any(|(_, l)| l.contains(needle));
    assert!(
        calls(&detail, "crate::trios::pricing::order_money_text("),
        "{DETAIL_PATH}: the card prints no order money through the shared rule"
    );
    assert!(
        calls(&list, "crate::trios::pricing::order_total_text("),
        "{LIST_PATH}: the list prints no order total through the shared rule"
    );
}

/// Defect 2. `format_baht` takes a bare f64, and its `sanitize_money` turns NaN,
/// infinity and a negative into 0.0 -- a figure nobody could compute printed as
/// a price. On these screens every order figure goes through the shared rule,
/// which hands the formatter a usable amount or answers with a dash.
#[test]
fn no_order_figure_reaches_format_baht_on_either_screen() {
    let mut offenders = Vec::new();
    for path in [DETAIL_PATH, LIST_PATH] {
        let body = source(path);
        for (n, line) in code_lines(&body) {
            if line.contains("format_baht(") {
                offenders.push(format!("{path}:{n}: {}", line.trim()));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "an order figure reaches format_baht without the absence rule (D9):\n  {}",
        offenders.join("\n  ")
    );
}

/// Defect 6, its loudest shape. A figure declared as a bare number fails the
/// whole response when the payload omits it: the detail screen then says the
/// order was not found, and the list loses EVERY order behind one missing total.
#[test]
fn a_payload_missing_a_figure_cannot_lose_either_screen() {
    let detail = source(DETAIL_PATH);
    let detail_dto = struct_code(&detail, "ApiOrderDetail", DETAIL_PATH);
    let mut offences = Vec::new();
    for bare in [
        "subtotal: f64,",
        "bonus_used: f64,",
        "stars_used: i64,",
        "total: f64,",
    ] {
        if detail_dto.iter().any(|l| l == bare) {
            offences.push(format!("ApiOrderDetail declares `{bare}`"));
        }
    }
    if !adjacent(
        &detail_dto,
        "#[serde(flatten)]",
        "money: crate::trios::pricing::OrderMoney,",
    ) {
        offences.push(
            "ApiOrderDetail does not carry its figures as the shared block of Options \
             (`#[serde(flatten)]` directly above `money: crate::trios::pricing::OrderMoney,`)"
                .to_string(),
        );
    }

    let list = source(LIST_PATH);
    let list_dto = struct_code(&list, "ApiOrder", LIST_PATH);
    if list_dto.iter().any(|l| l == "total: f64,") {
        offences.push("ApiOrder declares `total: f64,`".to_string());
    }
    if !adjacent(&list_dto, "#[serde(default)]", "total: Option<f64>,") {
        offences.push(
            "ApiOrder.total is not an Option carrying #[serde(default)] -- one order \
             without a total fails the whole list"
                .to_string(),
        );
    }
    assert!(
        offences.is_empty(),
        "a payload missing a figure still loses a screen:\n  {}",
        offences.join("\n  ")
    );
}

/// Defect 6, the omitted-row shape. `if order.bonus_used > 0.0` hid a negative
/// bonus exactly like a zero one. The rows are now decided by the shared rule:
/// a measured zero omits the row, an absence shows the row with a dash.
#[test]
fn a_discount_row_is_omitted_only_for_a_measured_zero() {
    let detail = source(DETAIL_PATH);
    let code = code_lines(&detail);
    let mut offences = Vec::new();
    for raw in [
        "order.bonus_used > 0.0",
        "order.stars_used > 0",
        "-{order.stars_used}",
        "-{bonus_str}",
    ] {
        for (n, line) in &code {
            if line.contains(raw) {
                offences.push(format!("{DETAIL_PATH}:{n}: `{}`", line.trim()));
            }
        }
    }
    for needed in [
        "if let Some(bonus) = &money.bonus {",
        "if let Some(stars) = &money.stars {",
        "\"{money.subtotal}\"",
        "\"{money.total}\"",
    ] {
        if !code.iter().any(|(_, l)| l.contains(needed)) {
            offences.push(format!("{DETAIL_PATH}: never renders `{needed}`"));
        }
    }
    assert!(
        offences.is_empty(),
        "the detail card decides a figure's row by itself:\n  {}",
        offences.join("\n  ")
    );
}

/// One shape of absence. The line total on the same card already renders an
/// unpriced line as `bike_card::DASH` (through `thb_or_dash`); the order
/// figures must hand the shared rule that same glyph, not a second one.
#[test]
fn both_screens_hand_the_rule_the_lines_own_dash() {
    for (path, call) in [
        (DETAIL_PATH, "crate::trios::pricing::order_money_text("),
        (LIST_PATH, "crate::trios::pricing::order_total_text("),
    ] {
        let body = source(path);
        let code = code_lines(&body);
        let at = code
            .iter()
            .position(|(_, l)| l.contains(call))
            .unwrap_or_else(|| panic!("{path}: never calls `{call}`"));
        // The call and whatever rustfmt wraps onto the next lines, up to its `;`.
        let mut span = String::new();
        for (_, l) in &code[at..] {
            span.push_str(l.trim());
            if l.contains(';') {
                break;
            }
        }
        // Either the glyph is named in the call, or the call passes a local that
        // is bound to it and to nothing else.
        let named_in_the_call = span.contains("crate::ui::components::bike_card::DASH");
        let bound_local = code
            .iter()
            .any(|(_, l)| l.trim() == "let dash = crate::ui::components::bike_card::DASH;");
        let passes_the_local = span.contains(", dash)");
        assert!(
            named_in_the_call || (bound_local && passes_the_local),
            "{path}: the order rule is not handed bike_card::DASH -- `{span}`"
        );
    }
}

/// A named refusal, pinned: which sentence -- if any -- stands beside a dashed
/// order figure is the OWNER'S open question (order_presentation.t27,
/// ORDER_FIGURE_DASH_HAS_A_PUBLISHED_SENTENCE). D9 requires the dash and nothing
/// more; the rate sentence (D11, a silent door) is about a price quote, not a
/// placed order. This guard cannot be red on today's tree -- it pins that the
/// default is not shipped by accident.
#[test]
fn no_sentence_rides_on_an_order_figure_until_the_owner_rules() {
    for path in [DETAIL_PATH, LIST_PATH] {
        let body = source(path);
        for key in [
            "T_BIKE_PRICE_ON_REQUEST",
            "T_BIKE_QUOTE_NOTE",
            "SAY_HUMAN_QUOTES",
        ] {
            assert!(
                !code_lines(&body).iter().any(|(_, l)| l.contains(key)),
                "{path} names {key}: a sentence beside an order figure is the owner's \
                 decision, recorded as open, and nothing publishes one"
            );
        }
    }
}

/// D15, and a tripwire rather than a delegation. The server keeps its own copy
/// of the order-figure filter (`finite_money` in src/db/orders.rs), and this
/// change leaves the server alone. The two must stay one predicate: if either
/// drifts, a figure the server calls usable is one the screens call absent.
#[test]
fn the_servers_order_filter_and_the_shared_one_are_one_predicate() {
    const PREDICATE: &str = ".filter(|v| v.is_finite() && *v >= 0.0)";
    let body_of = |path: &str, head: &str| -> String {
        let text = source(path);
        let at = text
            .find(head)
            .unwrap_or_else(|| panic!("{path}: `{head}` is gone"));
        let rest = &text[at..];
        let end = rest
            .find("\n}")
            .unwrap_or_else(|| panic!("{path}: `{head}` never closes"));
        rest[..end].to_string()
    };
    let server = body_of(
        SERVER_ROW_PATH,
        "pub(crate) fn finite_money(raw: Option<f64>) -> Option<f64> {",
    );
    let shared = body_of(
        SHARED_RULE_PATH,
        "pub fn measured_money(amount: Option<f64>) -> Option<f64> {",
    );
    assert!(
        server.contains(PREDICATE),
        "{SERVER_ROW_PATH}: finite_money no longer reads `{PREDICATE}`:\n{server}"
    );
    assert!(
        shared.contains(PREDICATE),
        "{SHARED_RULE_PATH}: measured_money no longer reads `{PREDICATE}`:\n{shared}"
    );
}

/// Every value of one contract constant: the single string of a `str`, or every
/// string of a `[N]str`, whether the array sits on one line or spans several.
fn spec_values(name: &str) -> Vec<String> {
    let spec = source(CONTRACT);
    let head = format!("pub const {name} : ");
    let mut lines = spec.lines().skip_while(|l| !l.starts_with(&head));
    let first = lines
        .next()
        .unwrap_or_else(|| panic!("`{name}` is no longer declared in {CONTRACT}"));
    let mut text = first.to_string();
    if first.contains("= [") && !first.trim_end().ends_with("];") {
        for line in lines.by_ref() {
            text.push('\n');
            text.push_str(line);
            if line.trim_start().starts_with("];") {
                break;
            }
        }
    }
    let body = &text[text.find('=').unwrap_or(0)..];
    let values: Vec<String> = body
        .split('"')
        .skip(1)
        .step_by(2)
        .map(str::to_string)
        .collect();
    assert!(!values.is_empty(), "`{name}` in {CONTRACT} holds no string");
    values
}

/// The lines one `path:N` or `path:N-M` citation names.
fn cited_lines(site: &str) -> Vec<String> {
    let (path, range) = site
        .rsplit_once(':')
        .unwrap_or_else(|| panic!("`{site}` is not a path:line citation"));
    let (first, last) = range.split_once('-').unwrap_or((range, range));
    let first: usize = first.parse().expect("a first line number");
    let last: usize = last.parse().expect("a last line number");
    let text = source(path);
    let lines: Vec<&str> = text.lines().collect();
    assert!(
        first >= 1 && last >= first && last <= lines.len(),
        "`{site}` is not a range inside {path} ({} lines)",
        lines.len()
    );
    lines[first - 1..last]
        .iter()
        .map(|l| l.to_string())
        .collect()
}

/// Every money site the contract cites lands on the code it names, the server's
/// included: those are not owned here, and a citation nobody re-reads is the
/// claim that goes false first. Sites kept under `_BEFORE_REPAIR` names are
/// commit 14b01ac's lines and under `_BEFORE_THE_WIRE_FIX` names a33e500's;
/// both are history, never read here. Since 2026-09-23 (T27 defect 2) the
/// wire's withholding, the readers that read an absence and the two totals
/// outside the boundary are read here too.
#[test]
fn the_contracts_money_sites_land_on_their_code() {
    let table: [(&str, &[&str]); 14] = [
        (
            "ORDER_FIGURE_RENDER_SITES",
            &[
                "crate::trios::pricing::order_money_text(&order.money, dash)",
                "crate::trios::pricing::order_total_text(o.total,",
            ],
        ),
        (
            "OPTIONAL_FIGURE_SITES",
            &[
                "money: crate::trios::pricing::OrderMoney,",
                "total: Option<f64>,",
            ],
        ),
        (
            "ZERO_FALLBACK_SITE",
            &["fn sanitize_money(v: f64) -> f64 {"],
        ),
        (
            "PUBLISHED_FILTER_SITE",
            &["measured_money(amount).filter(|v| *v > 0.0)"],
        ),
        (
            "WIRE_WITHHOLD_SITE",
            &["withhold_money(&mut withheld, &m.id, field, &v);"],
        ),
        (
            "WIRE_STAR_WITHHOLD_SITE",
            &["withhold_money(&mut withheld, &m.id, \"stars_used\", &m.stars_used);"],
        ),
        (
            "MONEY_WITHHELD_REASON_SITE",
            &["MONEY_WITHHELD_UNUSABLE: &str = \"stored_value_not_finite_non_negative\";"],
        ),
        (
            // `Order::for_customer` runs `Order::from` first (owner, 2026-09-25,
            // answer 3; `tests/legacy_view_wiring.rs` holds it to that).
            "ORDER_ENDPOINTS_SERIALISE_THROUGH_ONE_CONVERSION",
            &["Order::for_customer(model)", "map(Order::for_customer)"],
        ),
        (
            "ADMIN_ENDPOINTS_SERIALISE_THROUGH_THE_SAME_CONVERSION",
            &["map(Order::from)", "Order::from(m)"],
        ),
        (
            "ABSENCE_READING_SITES",
            &[
                "total: Option<f64>,",
                "total: Option<f64>,",
                "pub total: Option<f64>,",
                "stars_used: Option<i64>,",
            ],
        ),
        (
            "ORDER_TOTAL_SURFACES_OUTSIDE_THE_BOUNDARY",
            &["order_total_text(order.total,", "order_total_text(o.total,"],
        ),
        (
            "BIKE_LINE_SUM_SITE",
            &["a bike-only cart", "legitimately claims a subtotal of 0."],
        ),
        (
            "CHECKOUT_SUBMISSION_SITE",
            &["let items_json: Vec<serde_json::Value> = submit_cart_items"],
        ),
        (
            "DASH_FILTERED_SITE",
            &["fn line_total(item: &ApiOrderItem) -> String {"],
        ),
    ];
    let mut wrong = Vec::new();
    let mut checked = 0usize;
    for (name, anchors) in table {
        let values = spec_values(name);
        // One anchor per site, or -- for a single ranged site -- several anchors
        // that must all fall inside the one range.
        let pairs: Vec<(String, &str)> = if values.len() == anchors.len() {
            values
                .iter()
                .cloned()
                .zip(anchors.iter().copied())
                .collect()
        } else {
            assert_eq!(values.len(), 1, "{name}: {values:?} against {anchors:?}");
            anchors.iter().map(|a| (values[0].clone(), *a)).collect()
        };
        for (site, anchor) in pairs {
            if !cited_lines(&site).iter().any(|l| l.contains(anchor)) {
                wrong.push(format!("{name} = {site} does not hold `{anchor}`"));
            }
            checked += 1;
        }
    }
    // D16: the table is the input, and a check of nothing is not a check.
    assert!(checked >= 22, "only {checked} sites were checked");
    assert!(
        wrong.is_empty(),
        "the contract's money sites no longer land on their code:\n  {}",
        wrong.join("\n  ")
    );
}
