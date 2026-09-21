//! One reader for a server cart line, and an unreadable line is dropped.
//!
//! `src/lib.rs` gates `pub mod ui;` on `#[cfg(target_arch = "wasm32")]`, so
//! nothing under `src/ui` is compiled by `cargo test`. That blackout is how two
//! converters for one wire shape drifted apart unnoticed: `CartItem::from_server`
//! (`src/ui/state.rs`) returned `None` for a kind it did not recognise, while the
//! `From<CartRespDto> for Cart` impl in `src/ui/api/http.rs` mapped the same
//! input to `CartItemType::Strain`.
//!
//! What that cost: `cart_item_type_to_kind` then sends the relabelled line back
//! as `"strain"` to `POST /api/cart/merge`, which the DTO's own doc calls
//! price-authoritative. A rental that round-trips through a merge would come
//! back priced as cannabis. The failure needs no bad actor — only a server that
//! one day speaks a kind the shipped bundle predates, which is precisely what
//! #28 and #10 intend it to do.
//!
//! These checks read `src/` as text, the pattern `customer_surface_wiring.rs`
//! and `price_provenance_wiring.rs` already use and CI already runs.

use std::fs;
use std::path::PathBuf;

/// Every `.rs` file under `src/`. A walk rather than an `include_str!` list
/// because the invariant is *nowhere*, and a fixed list cannot say that about
/// a file somebody adds tomorrow.
fn rust_sources() -> Vec<(String, String)> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut stack = vec![root.clone()];
    let mut out = Vec::new();
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).expect("src/ is readable").flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "rs") {
                // One separator, whatever the host: the expectations below are
                // written with forward slashes, and a backslash here makes every
                // entry miss and reports a drift that is not there.
                let label = path
                    .strip_prefix(root.parent().expect("src has a parent"))
                    .unwrap_or(&path)
                    .display()
                    .to_string()
                    .replace('\\', "/");
                out.push((label, fs::read_to_string(&path).expect("readable")));
            }
        }
    }
    assert!(
        out.len() > 20,
        "the walk found {} files — it is not reaching src/",
        out.len()
    );
    out
}

/// A line with its `//` comment removed, so prose *describing* the defect is
/// never mistaken for the defect. Naive by intent: it does not track string
/// literals, and no line involved here puts `//` inside one.
fn code_of(line: &str) -> &str {
    match line.find("//") {
        Some(i) => &line[..i],
        None => line,
    }
}

/// Does this line open a `Name { ... }` literal for exactly this type?
///
/// The word boundary is the whole point: a plain substring search for
/// `CartItem {` also matches `ServerCartItem {`, which is a different type on
/// the neighbouring line. That near-miss is the same shape as the bug being
/// guarded, so it is worth spending eight lines not to repeat it.
fn opens_literal_of(code: &str, name: &str) -> bool {
    let needle = format!("{name} {{");
    code.match_indices(&needle).any(|(i, _)| {
        code[..i]
            .chars()
            .next_back()
            .is_none_or(|c| !c.is_alphanumeric() && c != '_')
    })
}

#[test]
fn only_one_place_turns_a_wire_kind_into_a_cart_item_type() {
    // `=> CartItemType::` is a string arm producing the enum. The reverse
    // direction reads `CartItemType::Strain =>` and is not matched, which is
    // what leaves `cart_item_type_to_kind` alone.
    let mut homes: Vec<String> = Vec::new();
    for (label, source) in rust_sources() {
        if source
            .lines()
            .any(|l| code_of(l).contains("=> CartItemType::"))
        {
            homes.push(label);
        }
    }
    homes.sort();
    assert_eq!(
        homes,
        vec!["src/ui/state.rs".to_string()],
        "a wire kind is parsed into a CartItemType outside CartItem::from_server \
         — one reader, or they drift again (D15)"
    );
}

#[test]
fn an_unreadable_kind_is_dropped_never_classified() {
    for (label, source) in rust_sources() {
        for (n, line) in source.lines().enumerate() {
            let code = code_of(line);
            assert!(
                !code.contains("_ => CartItemType::"),
                "{label}:{} classifies an unrecognised kind as a concrete \
                 CartItemType. A line this build cannot read must be dropped, \
                 not guessed at — guessing is how a motorbike leaves a merge \
                 labelled a cannabis strain.",
                n + 1
            );
        }
    }
}

#[test]
fn the_merge_response_reads_its_lines_through_from_server() {
    let http =
        fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/ui/api/http.rs"))
            .expect("src/ui/api/http.rs is readable");

    assert!(
        http.contains("CartItem::from_server"),
        "src/ui/api/http.rs no longer routes the merge response through \
         CartItem::from_server"
    );
    // A `CartItem { .. }` literal here is the shape of the bug: rebuilding the
    // struct field-by-field is what let this path disagree with from_server
    // about unknown kinds, zero quantities and a non-finite unit_price — the
    // last of which reached recalculate_total and made the whole total NaN.
    let literal = http
        .lines()
        .enumerate()
        .find(|(_, l)| opens_literal_of(code_of(l), "CartItem"));
    assert!(
        literal.is_none(),
        "src/ui/api/http.rs:{} builds a CartItem by hand instead of calling \
         CartItem::from_server",
        literal.map_or(0, |(n, _)| n + 1)
    );
}
