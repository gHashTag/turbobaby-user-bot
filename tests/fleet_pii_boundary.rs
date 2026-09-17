//! The D14 boundary, enforced instead of described.
//!
//! The internal fleet register and the rental-history sheet this catalog was
//! built from carry renter names, Telegram handles, phone numbers, outstanding
//! debts, physical key codes, TAX dates and per-unit purchase costs. Issue #6
//! makes their absence an acceptance criterion — "no column in either table
//! contains a personal name or phone number" — and until this file existed the
//! only thing enforcing it was a paragraph of prose at the top of
//! `migrations/078_bike_units.sql` and a `$comment` in the seed. Prose does not
//! fail a build.
//!
//! Two things are checked, because the leak has two shapes. A **column** named
//! for a person invites the data in at the next admin-panel change. A **value**
//! already in the committed seed is a leak that has already happened. The first
//! is a schema question, the second a grep, and neither covers the other.
//!
//! Both are paired with a corpus assertion. A "contains none of these words"
//! test over a file the parser failed to read passes for the worst possible
//! reason, and that failure mode is silent — so each check first proves it is
//! looking at something.

use std::collections::BTreeSet;
use std::path::Path;

fn read(path: impl AsRef<Path>) -> String {
    let path = path.as_ref();
    std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

/// The fleet tables. 080 is included because a service record is written by a
/// mechanic about a machine, which is exactly where a customer's name and phone
/// number would feel natural to add.
const FLEET_MIGRATIONS: [&str; 3] = [
    "migrations/077_bikes.sql",
    "migrations/078_bike_units.sql",
    "migrations/080_bike_service_records.sql",
];

/// Column names declared by the fleet migrations.
///
/// Deliberately a crude parser — a line inside a `CREATE TABLE` whose first
/// token is a lowercase identifier followed by an uppercase type word. It
/// cannot be fooled into *missing* a column, which is the only direction that
/// matters here: a column it fails to parse would be an undetected leak, so
/// [`the_parser_actually_sees_the_fleet_schema`] pins the count it finds.
fn fleet_columns() -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    for file in FLEET_MIGRATIONS {
        let sql = read(file);
        let mut inside = false;
        for line in sql.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("--") {
                continue;
            }
            if trimmed.to_uppercase().starts_with("CREATE TABLE") {
                inside = true;
                continue;
            }
            if inside && trimmed.starts_with(");") {
                inside = false;
                continue;
            }
            if !inside {
                continue;
            }
            let mut words = trimmed.split_whitespace();
            let (Some(name), Some(ty)) = (words.next(), words.next()) else {
                continue;
            };
            let named = !name.is_empty()
                && name
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c == '_' || c.is_ascii_digit());
            let typed = ty.chars().next().is_some_and(|c| c.is_ascii_uppercase());
            if named && typed && !matches!(name, "constraint" | "check" | "unique" | "primary") {
                found.insert(format!("{file}:{name}"));
            }
        }
    }
    found
}

#[test]
fn the_parser_actually_sees_the_fleet_schema() {
    let columns = fleet_columns();
    assert!(
        columns.len() >= 20,
        "the column parser found only {} columns across {} migrations — it has \
         stopped matching the files' shape, and every D14 assertion below is \
         now passing over an empty corpus: {columns:?}",
        columns.len(),
        FLEET_MIGRATIONS.len()
    );
    // One column per table, named, so a migration silently dropped from the
    // list above cannot be hidden by the other two clearing the count.
    for expected in [
        "migrations/077_bikes.sql:base_rate_thb_day",
        "migrations/078_bike_units.sql:unit_code",
        "migrations/080_bike_service_records.sql:bike_unit_id",
    ] {
        assert!(
            columns.contains(expected),
            "{expected} is missing from the parsed schema"
        );
    }
}

/// Column-name fragments that name a person, their money, or their access.
///
/// Each carries the register column it stands for. `purchase` alone is absent
/// on purpose: `km_since_purchase` is an odometer reading and legitimate, and a
/// gate that fires on a legitimate column gets deleted rather than obeyed —
/// so the cost fragments are spelled out in full instead.
const FORBIDDEN_FRAGMENTS: [(&str, &str); 12] = [
    ("renter", "«Кому сдан» — who the machine is out with"),
    ("phone", "the rental-history sheet's phone column"),
    ("telegram", "the customer's Telegram handle"),
    ("passport", "identity documents taken as a deposit"),
    ("debt", "«Долг» — what a customer still owes"),
    ("key_code", "«Key code» — the physical key for this machine"),
    ("keycode", "«Key code», spelled without the underscore"),
    ("purchase_price", "what TurboBaby paid for the unit"),
    ("purchase_cost", "what TurboBaby paid for the unit"),
    ("acquisition_cost", "what TurboBaby paid for the unit"),
    ("tax_date", "the register's TAX column"),
    ("customer_name", "a named individual"),
];

#[test]
fn no_fleet_column_names_a_person_their_money_or_their_keys() {
    for column in fleet_columns() {
        let name = column.rsplit(':').next().unwrap_or_default().to_string();
        for (fragment, source) in FORBIDDEN_FRAGMENTS {
            assert!(
                !name.contains(fragment),
                "{column} carries {fragment:?} — that is {source}, which D14 and \
                 issue #6 bar from this repository. The public catalog describes \
                 machines; the register describes people, and the boundary is \
                 the column list, not a promise in a comment."
            );
        }
    }
}

/// The committed seed, checked for values rather than names.
///
/// A leak that has already happened does not need a badly-named column — it
/// only needs a free-text `note` copied out of the operations sheet.
#[test]
fn the_seed_carries_no_phone_number_and_no_telegram_handle() {
    let seed = read("data/fleet_seed.json");
    assert!(
        seed.len() > 4000 && seed.contains("\"families\""),
        "data/fleet_seed.json is not the file this test was written against \
         ({} bytes); the scan below would pass over nothing",
        seed.len()
    );

    // A Thai number is +66 and nine more digits; the seed legitimately carries
    // the bare calling code `+66` as market data, so the run length is what
    // separates a dialling prefix from a person.
    let digits: Vec<char> = seed.chars().collect();
    for (i, w) in digits.windows(4).enumerate() {
        if w != ['+', '6', '6', '-'] && !(w[0] == '+' && w[1] == '6' && w[2] == '6') {
            continue;
        }
        let tail = digits[i + 3..]
            .iter()
            .take_while(|c| c.is_ascii_digit() || **c == ' ' || **c == '-')
            .filter(|c| c.is_ascii_digit())
            .count();
        assert!(
            tail < 7,
            "the seed contains +66 followed by {tail} digits — that is a phone \
             number, not the market's calling code"
        );
    }

    // `@` in this file may only appear inside an email-free context; the seed
    // has no legitimate handle at all today, so any run of handle-shaped text
    // is a copy from the rental-history sheet.
    let mut chars = seed.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        if c != '@' {
            continue;
        }
        let handle: String = seed[i + 1..]
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
            .collect();
        assert!(
            handle.len() < 4,
            "the seed contains @{handle} — a Telegram handle or an address is \
             personally identifying and must not enter this repository, in any \
             field, including a free-text note"
        );
    }
}

/// The counterpart to both scans: the register's *own* vocabulary must not
/// appear in the rows the file ships, columns or not.
///
/// The two tests above look at names and at shapes. This one looks at words —
/// the case where somebody pastes a sheet row into a `note` verbatim and the
/// resulting string is neither a column nor a phone number.
///
/// Scoped to `families` on purpose. The file's `$comment` and `sources` blocks
/// *name* the prohibited columns in order to declare that they are excluded —
/// "its price columns are purchase cost and are deliberately not read" — and a
/// scan of the whole file would fire on the sentence that states the rule. A
/// gate that punishes its own documentation is a gate that gets deleted.
#[test]
fn no_seeded_family_quotes_the_register_s_private_columns() {
    let raw = read("data/fleet_seed.json");
    let doc: serde_json::Value =
        serde_json::from_str(&raw).expect("data/fleet_seed.json parses as JSON");
    let families = doc["families"]
        .as_array()
        .expect("data/fleet_seed.json has a families array");
    assert!(
        families.len() >= 14,
        "only {} families parsed — the scan below has nothing to look at",
        families.len()
    );

    let text = serde_json::to_string(families)
        .expect("the families array re-serializes")
        .to_lowercase();
    // «Кому сдан», «Долг» and «Key code» as the sheet spells them, plus the
    // English a translation of it would reach for.
    for word in [
        "кому сдан",
        "долг",
        "key code",
        "renter",
        "purchase price",
        "purchase cost",
    ] {
        assert!(
            !text.contains(word),
            "a seeded family quotes {word:?} from the internal register. The \
             fleet sheet is authority for counts, years, colours and status \
             only — its price columns are purchase cost and its people columns \
             are nobody's business but the shop's."
        );
    }
}
