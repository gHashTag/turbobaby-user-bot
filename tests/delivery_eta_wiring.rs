//! No delivery time in minutes reaches a customer unless a zone row carries one (2026-09-24).
//!
//! Owner, 2026-09-24: the shop publishes a delivery WINDOW, never a travel time, so the checkout
//! shows the fee only and the hard-coded "30-45" fallback goes. Migration 087 stores no ETA on
//! any zone row and the API serves `null` for both minute fields.
//!
//! The two screens that used to print minutes compile only for wasm32, so they are guarded here
//! as text, the way `tests/order_presentation_wiring.rs` guards the order screens. Until this
//! change the checkout formatted `"{min}-{max}"` from whatever the zone row held and printed
//! `"30-45"` when the zone list was empty, and the success screen fell back to the ETA the
//! checkout had stored in local storage and then to the same literal. The server half is
//! host-compiled and unit-tested (`served_eta` and `zone_eta_min`/`zone_eta_max` in `src/delivery.rs`,
//! `created_zone_eta` in `src/api/admin.rs`); this file checks that both endpoints go through it.

const CHECKOUT: &str = include_str!("../src/ui/screens/checkout_screen.rs");
const SUCCESS: &str = include_str!("../src/ui/screens/success_screen.rs");
const TYPES: &str = include_str!("../src/ui/api/types.rs");
const ORDERS: &str = include_str!("../src/api/orders.rs");
const ADMIN: &str = include_str!("../src/api/admin.rs");

/// Lines that are code rather than prose. Comments may name the literal they replaced; the
/// guard is about what the program does.
fn code_lines(source: &str) -> Vec<&str> {
    source
        .lines()
        .filter(|line| {
            let trimmed = line.trim_start();
            !(trimmed.starts_with("//") || trimmed.starts_with("/*") || trimmed.starts_with('*'))
        })
        .collect()
}

fn code(source: &str) -> String {
    code_lines(source).join("\n")
}

/// The body of `pub struct <name> {`, line endings normalised so a CRLF checkout reads the same.
fn struct_body(source: &str, name: &str) -> String {
    let source = source.replace("\r\n", "\n");
    let open = format!("pub struct {name} {{");
    let start = source
        .find(&open)
        .unwrap_or_else(|| panic!("`{open}` is not in the file any more"));
    let end = source[start..]
        .find("\n}")
        .unwrap_or_else(|| panic!("`{open}` has no closing brace"));
    source[start..start + end].to_string()
}

#[test]
fn neither_screen_prints_a_literal_eta_range() {
    for (path, source) in [
        ("src/ui/screens/checkout_screen.rs", CHECKOUT),
        ("src/ui/screens/success_screen.rs", SUCCESS),
    ] {
        // Witness (D16): the scan reads real code, so a zero below is a measurement.
        assert!(
            code_lines(source).len() > 300,
            "{path}: the code scan found almost nothing to read"
        );
        let offending: Vec<&str> = code_lines(source)
            .into_iter()
            .filter(|line| line.contains("30-45"))
            .collect();
        assert!(
            offending.is_empty(),
            "{path} still prints a minute range nobody published: {offending:?}"
        );
    }
}

#[test]
fn the_checkout_formats_an_eta_only_from_two_present_minutes() {
    let code = code(CHECKOUT);
    assert!(
        code.contains("let delivery_eta_text: Option<String>"),
        "the ETA text is an Option, so an absent pair produces no line"
    );
    assert!(
        code.contains("z.min_eta_minutes?") && code.contains("z.max_eta_minutes?"),
        "each edge is taken with `?`: a missing edge ends the ETA, it is not filled in"
    );
    assert!(
        code.contains("if let Some(eta) = delivery_eta_text"),
        "the ETA line is rendered only when there is one"
    );
    assert!(
        !code.contains("format!(\"{}-{}\", z.min_eta_minutes"),
        "the unconditional `min-max` format is back"
    );
    // Witness: the fee line the checkout keeps is still built.
    assert!(code.contains("T_DELIVERY_FEE"), "the fee line is gone");
}

#[test]
fn no_eta_travels_through_local_storage() {
    for (path, source) in [
        ("src/ui/screens/checkout_screen.rs", CHECKOUT),
        ("src/ui/screens/success_screen.rs", SUCCESS),
    ] {
        assert!(
            !code(source).contains("\"woody_last_zone_eta\""),
            "{path} still writes or reads a stored ETA; an old device holds the other island's"
        );
        // Witness: the zone NAME still travels, so the key family is still visible to the scan.
        assert!(
            code(source).contains("\"woody_last_zone_name\""),
            "{path}: the zone name key is gone, so this scan can no longer see the family"
        );
    }
    assert!(
        !code(SUCCESS).contains("zone_eta"),
        "the success screen keeps a local ETA fallback"
    );
}

#[test]
fn the_success_screen_shows_an_eta_row_only_for_a_served_pair() {
    let code = code(SUCCESS);
    let start = code
        .find("let eta_range = ")
        .expect("the success screen still builds an ETA range");
    let statement = &code[start..start + code[start..].find(';').unwrap_or(0)];
    for fallback in ["or_else", "unwrap_or", "zone_eta"] {
        assert!(
            !statement.contains(fallback),
            "the success screen's ETA falls back to something again (`{fallback}`): {statement}"
        );
    }
    assert!(
        code.contains("if let Some(range) = eta_range.as_ref()"),
        "the ETA row is rendered only when the server sent both edges"
    );
    assert!(
        statement.contains("s.min_eta_minutes?") && statement.contains("s.max_eta_minutes?"),
        "the range is formatted only from two served minutes: {statement}"
    );
}

#[test]
fn the_client_reads_an_absent_eta_as_none() {
    let body = struct_body(TYPES, "DeliveryZone");
    for field in ["min_eta_minutes", "max_eta_minutes"] {
        assert!(
            body.contains(&format!(
                "    #[serde(default)]\n    pub {field}: Option<u32>,"
            )),
            "types.rs DeliveryZone.{field} must be an Option carrying #[serde(default)]: {body}"
        );
    }
    assert!(
        body.contains("pub delivery_fee_baht: f64,"),
        "witness: the struct scanned is the zone struct"
    );
}

#[test]
fn both_zone_endpoints_serve_the_eta_through_the_one_rule() {
    let code = code(ORDERS);
    assert!(
        !code.contains("eta_min.max(") && !code.contains("eta_max.max("),
        "an endpoint clamps a stored ETA by hand again, which invents 0 for an absent one"
    );
    assert_eq!(
        code.matches("\"min_eta_minutes\": ").count(),
        2,
        "witness: both endpoints still publish the field"
    );
    assert!(
        code.contains(
            "\"min_eta_minutes\": zone.as_ref().and_then(crate::delivery::zone_eta_min),"
        ) && code.contains(
            "\"max_eta_minutes\": zone.as_ref().and_then(crate::delivery::zone_eta_max),"
        ),
        "the order status endpoint serves the ETA through the rule"
    );
    assert!(
        code.contains("\"min_eta_minutes\": crate::delivery::zone_eta_min(&z),")
            && code.contains("\"max_eta_minutes\": crate::delivery::zone_eta_max(&z),"),
        "the zone list serves the ETA through the rule"
    );
}

#[test]
fn the_admin_write_supplies_no_minutes_of_its_own() {
    let code = code(ADMIN);
    for invented in ["unwrap_or(30)", "unwrap_or(60)"] {
        assert!(
            !code.contains(invented),
            "an admin zone write defaults an ETA again: `{invented}`"
        );
    }
    assert!(
        code.contains("let (eta_min, eta_max) = created_zone_eta(req.eta_min, req.eta_max);"),
        "a created zone stores the ETA it was given, or none"
    );
    assert!(
        code.contains("deserialize_with = \"present_or_null\""),
        "an update can clear a stored ETA with null"
    );
}
