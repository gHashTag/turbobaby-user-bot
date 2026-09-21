//! Three defects on the client side of the wire, guarded as text.
//!
//! `src/lib.rs` gates `pub mod ui;` on `#[cfg(target_arch = "wasm32")]`, so
//! `cargo test` compiles nothing under `src/ui` —
//! `tests/money_is_never_invented.rs:10-16` says so in its own words, and
//! `specs/turbobaby/webapp_bridge.t27` counts the blind spot: 32 `#[test]`
//! functions live under `src/ui` and not one of them is ever compiled. Reading
//! the text is the only instrument that reaches these files from CI, so that is
//! the instrument this uses — and it says so rather than pretending to be
//! something stronger. The *logic* the fixes introduced lives in
//! `src/trios/pricing.rs`, which the host does compile, and is tested there.
//!
//! What the three guards are for:
//!
//! 1. A dropped `unit_price` used to deserialise to `0.0` and render a free
//!    bike. D9 — an absent number never becomes a confident zero.
//! 2. An `initData` longer than the server's cap used to be replaced with an
//!    empty string, and `with_auth` then declined to send an empty header, so a
//!    payload the server would have refused left as an anonymous request.
//! 3. The 409 arm of `submit_order_with_retry` guarded a status
//!    `POST /api/orders` cannot send, and built a recovery URL out of
//!    `telegram_id.unwrap_or(0)` — a request for user 0.
//!
//! The third guard re-takes the measurement the removal rests on instead of
//! trusting it: if `src/api/orders.rs` ever grows a `CONFLICT` outside
//! `cancel_order`, the premise is dead and this test says so, rather than the
//! next worker discovering a customer stuck on an error banner.

const TYPES: &str = include_str!("../src/ui/api/types.rs");
const STATE: &str = include_str!("../src/ui/state.rs");
const CLIENT: &str = include_str!("../src/ui/api/client.rs");
const HTTP: &str = include_str!("../src/ui/api/http.rs");
const CHECKOUT: &str = include_str!("../src/ui/screens/checkout_screen.rs");
const ORDERS: &str = include_str!("../src/api/orders.rs");

/// Every corpus this file reads, with the floor below which the read itself is
/// the thing that is broken.
///
/// A scan whose input came back empty passes every check it makes and reports
/// success. That vacuous-scan shape is this repository's most expensive
/// recurring defect (`tests/money_is_never_invented.rs:18-23`), so the corpus
/// is pinned before any finding is evaluated. The floors are the measured
/// lengths on 2026-09-21 rounded down, not guesses.
const CORPUS: [(&str, &str, usize); 6] = [
    ("src/ui/api/types.rs", TYPES, 400),
    ("src/ui/state.rs", STATE, 120),
    ("src/ui/api/client.rs", CLIENT, 250),
    ("src/ui/api/http.rs", HTTP, 500),
    ("src/ui/screens/checkout_screen.rs", CHECKOUT, 1500),
    ("src/api/orders.rs", ORDERS, 3000),
];

/// The body of an item declared as `<opener>`, up to the closing brace at the
/// indentation the item was declared at. `rustfmt` runs on this tree, so an
/// item's closer is the first line that is exactly its own indent plus `}`.
///
/// The indent is measured rather than assumed: a method inside an `impl` closes
/// at four spaces, and reading it to the next column-0 brace instead swallows
/// every later method in the block. That is not a theoretical hazard — the
/// first draft did exactly that, and reported `String::new()` from the
/// constructor *beside* the one it was reading. A false RED is the lucky half
/// of that mistake; the same slip in the other direction is a false GREEN.
fn item_body<'a>(source: &'a str, opener: &str, label: &str) -> &'a str {
    let start = source
        .find(opener)
        .unwrap_or_else(|| panic!("{label}: `{opener}` is gone — this guard now reads nothing"));
    let line_start = source[..start].rfind('\n').map_or(0, |nl| nl + 1);
    let indent = &source[line_start..start];
    assert!(
        indent.chars().all(|c| c == ' '),
        "{label}: `{opener}` is not the first thing on its line"
    );
    let closer = format!("\n{indent}}}\n");
    let rest = &source[start..];
    let end = rest
        .find(&closer)
        .unwrap_or_else(|| panic!("{label}: `{opener}` never closes at its own indent"));
    &rest[..end]
}

/// Code lines only. Several comments in this tree quote the very defects below
/// on purpose — `unwrap_or(0)`, `String::new()` — and prose about a defect is
/// not the defect.
fn code_lines(body: &str) -> impl Iterator<Item = &str> {
    body.lines().filter(|l| {
        let t = l.trim_start();
        !(t.starts_with("//") || t.starts_with("/*") || t.starts_with('*'))
    })
}

/// The name of the `fn` an offset falls inside: the nearest `fn ` at or before
/// it, which for this file's flat corpus is the enclosing function.
fn enclosing_fn(source: &str, index: usize) -> &str {
    let head = &source[..index];
    let fn_at = head
        .rfind("fn ")
        .unwrap_or_else(|| panic!("offset {index} is not inside any fn"));
    let after = &source[fn_at + 3..];
    let end = after
        .find(|c: char| !(c.is_alphanumeric() || c == '_'))
        .unwrap_or(after.len());
    &after[..end]
}

#[test]
fn the_guard_reads_the_files_it_claims_to_read() {
    for (label, source, floor) in CORPUS {
        let lines = source.lines().count();
        assert!(
            lines >= floor,
            "{label} read only {lines} lines (floor {floor}) — a shrunken corpus \
             passes every check below by default, so the reader fails here \
             instead of the code passing there"
        );
    }
    // The extractor must find something that is really there, AND stop where
    // the item stops. `ApiClient::new` is followed by `ApiClient::anonymous`,
    // which legitimately contains `String::new()`: an extractor that runs past
    // the method boundary reports its neighbour's code as its own.
    assert!(
        item_body(TYPES, "pub struct ServerCartItem {", "types.rs").contains("catalog_id"),
        "the item extractor no longer finds a field it is standing on"
    );
    assert!(
        !item_body(CLIENT, "pub fn new(", "client.rs").contains("pub fn anonymous("),
        "the item extractor runs past a method's closing brace into the next one"
    );
}

#[test]
fn an_absent_unit_price_stays_absent_on_the_wire() {
    // `#[serde(default)]` on a bare `f64` writes the type's Default and nothing
    // anywhere notices: the server omits a price, serde supplies 0.0, and a
    // screen renders a free bike (D9). On an `Option` the same attribute writes
    // `None`, which is still an absence and still renderable as one.
    for (label, source, opener) in [
        ("src/ui/api/types.rs", TYPES, "pub struct ServerCartItem {"),
        ("src/ui/api/http.rs", HTTP, "pub struct MergeCartItemDto {"),
    ] {
        let body = item_body(source, opener, label);
        assert!(
            body.contains("unit_price: Option<f64>"),
            "{label}: {opener} must carry `unit_price: Option<f64>` so a price \
             the server did not send arrives as an absence, not as a zero"
        );
        assert!(
            !code_lines(body).any(|l| l.contains("unit_price: f64")),
            "{label}: {opener} still declares a bare `unit_price: f64`"
        );
    }
}

#[test]
fn a_cart_line_the_shop_cannot_price_is_not_priced_at_nothing() {
    let item = item_body(STATE, "pub struct CartItem {", "src/ui/state.rs");
    assert!(
        item.contains("pub price: Option<f64>"),
        "src/ui/state.rs: CartItem.price must be `Option<f64>` — a line whose \
         price is absent has to be visibly not-priced, and `f64` has no room \
         to say so"
    );

    let from_server = item_body(STATE, "pub fn from_server(", "src/ui/state.rs");
    assert!(
        !code_lines(from_server).any(|l| l.contains("0.0")),
        "src/ui/state.rs: from_server writes a literal 0.0 again. The non-finite \
         arm used to land there, and 0.0 is the exact value D9 measures as \
         reading FREE"
    );
}

#[test]
fn a_cart_holding_an_unpriced_line_has_no_total() {
    let cart = item_body(STATE, "pub struct Cart {", "src/ui/state.rs");
    assert!(
        cart.contains("pub total: Option<f64>"),
        "src/ui/state.rs: Cart.total must be `Option<f64>`. Summing an unpriced \
         line as zero understates the cart and presents the understatement as \
         a measured figure"
    );

    let recalc = item_body(STATE, "pub fn recalculate_total(", "src/ui/state.rs");
    assert!(
        !code_lines(recalc).any(|l| l.contains("unwrap_or")),
        "src/ui/state.rs: recalculate_total flattens an absent price again"
    );
}

#[test]
fn an_oversize_init_data_is_refused_and_not_downgraded() {
    let new_fn = item_body(CLIENT, "pub fn new(", "src/ui/api/client.rs");
    assert!(
        !code_lines(new_fn).any(|l| l.contains("String::new()")),
        "src/ui/api/client.rs: ApiClient::new still blanks an oversize \
         initData. `with_auth` then declines to send an empty header, so a \
         payload the server would have refused leaves as an anonymous request \
         — a downgrade where a refusal belongs"
    );
    assert!(
        new_fn.contains("Err(ApiError::"),
        "src/ui/api/client.rs: ApiClient::new must hand the caller an error it \
         can see when the payload is over the cap"
    );
    // The cap itself is request_identity.t27's, mirrored once. Naming the
    // shared constant rather than a second literal is what keeps the client
    // from drifting above the server's admission rule.
    assert!(
        new_fn.contains("INIT_DATA_MAX_BYTES"),
        "src/ui/api/client.rs: the cap must be the shared constant, not a \
         second copy of the number"
    );
}

#[test]
fn the_client_mirror_of_the_cap_is_the_servers_own_number() {
    // `src/api/auth.rs` is behind the `backend` feature and the browser cannot
    // link it, so the client compiles against a mirror in `trios::validation`.
    // A mirror nothing compares is a second opinion: read both and insist they
    // are one number. The server's is the authority —
    // `scripts/verify_t27_against_source.py` binds THAT literal to
    // `specs/turbobaby/request_identity.t27`, and this test never reaches into
    // the spec, so the two gates check different edges of the same fact.
    const AUTH: &str = include_str!("../src/api/auth.rs");
    const TRIOS: &str = include_str!("../src/trios/validation.rs");

    let needle = "if init_data.len() > ";
    let at = AUTH
        .find(needle)
        .expect("src/api/auth.rs no longer caps init_data before parsing");
    let server_cap: usize = AUTH[at + needle.len()..]
        .chars()
        .take_while(char::is_ascii_digit)
        .collect::<String>()
        .parse()
        .expect("the server's cap is not a literal number any more");

    assert_eq!(
        server_cap,
        turbobaby_bot::trios::validation::INIT_DATA_MAX_BYTES,
        "the client would refuse at {} bytes while the server refuses above {server_cap} — \
         a client cap BELOW the server's is a self-inflicted outage, and one ABOVE it \
         sends payloads that can only come back refused",
        turbobaby_bot::trios::validation::INIT_DATA_MAX_BYTES
    );
    assert!(
        TRIOS.contains("pub fn init_data_size_admits("),
        "the shared admission rule is gone from src/trios/validation.rs"
    );
    // Inclusive, exactly as the server's `>` is: the cap itself is accepted.
    assert!(turbobaby_bot::trios::validation::init_data_size_admits(
        server_cap
    ));
    assert!(!turbobaby_bot::trios::validation::init_data_size_admits(
        server_cap + 1
    ));
}

#[test]
fn no_request_is_built_from_a_fabricated_telegram_id() {
    for needle in ["telegram_id.unwrap_or(0)", "telegram_id_for_retry"] {
        assert!(
            !code_lines(CHECKOUT).any(|l| l.contains(needle)),
            "src/ui/screens/checkout_screen.rs still builds a request from \
             `{needle}` — user 0 is not a customer, and a URL made from one \
             queries a user that does not exist"
        );
    }
}

#[test]
fn the_conflict_arm_is_gone_and_its_premise_still_holds() {
    // The removal rests on a measurement, so the measurement is re-taken here
    // rather than quoted. `POST /api/orders` answers a replay with 200 and a
    // body flag, refuses an unusable key with 400, and the only CONFLICT in the
    // file belongs to `cancel_order`.
    let conflicts: Vec<&str> = ORDERS
        .match_indices("StatusCode::CONFLICT")
        .map(|(i, _)| enclosing_fn(ORDERS, i))
        .collect();
    assert_eq!(
        conflicts,
        vec!["cancel_order"],
        "src/api/orders.rs now returns CONFLICT from {conflicts:?}. If one of \
         those is the create path, this endpoint can answer 409 again and the \
         client needs a REACHABLE and CORRECT arm for it — restore one that \
         matches the idempotency key, not the newest order by position"
    );
    assert!(
        ORDERS.contains("\"idempotent_replay\": true"),
        "src/api/orders.rs no longer flags a replay on the 200 path — the \
         reason 409 is unreachable from create has changed"
    );

    let retry = item_body(
        CHECKOUT,
        "async fn submit_order_with_retry(",
        "src/ui/screens/checkout_screen.rs",
    );
    assert!(
        !code_lines(retry).any(|l| l.contains("409")),
        "src/ui/screens/checkout_screen.rs: submit_order_with_retry branches on \
         409 again, and the endpoint it guards cannot send one. A dead arm that \
         recovers an order by recency is worse than no arm: it reports success \
         for an order nobody proved is ours"
    );
}
