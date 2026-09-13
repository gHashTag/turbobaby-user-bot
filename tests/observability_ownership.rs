//! Ownership contract for the request-correlation middleware.
//!
//! The backend library and server binary both compile `src/api/mod.rs` as
//! separate crate trees. Runtime-only middleware must therefore belong to the
//! server root once, or the unused library copy makes strict clippy fail while
//! testing a different copy from the one production runs.

use std::fs;
use std::path::Path;

fn declaration_count(source: &str) -> usize {
    source
        .lines()
        .map(str::trim)
        .map(|line| line.split_once("//").map_or(line, |(code, _)| code).trim())
        .filter(|line| {
            matches!(
                *line,
                "mod observability;"
                    | "pub mod observability;"
                    | "pub(crate) mod observability;"
                    | "pub(super) mod observability;"
            )
        })
        .count()
}

#[test]
fn server_binary_is_the_single_observability_owner() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let main = fs::read_to_string(root.join("src/main.rs")).expect("read src/main.rs");
    let api_mod = fs::read_to_string(root.join("src/api/mod.rs")).expect("read src/api/mod.rs");

    let main_declarations = declaration_count(&main);
    let api_declarations = declaration_count(&api_mod);
    let total_declarations = main_declarations + api_declarations;

    assert!(
        total_declarations > 0,
        "no observability module declaration found; the ownership scan is empty"
    );
    assert_eq!(
        main_declarations, 1,
        "src/main.rs must be the sole owner of runtime-only observability middleware"
    );
    assert_eq!(
        api_declarations, 0,
        "src/api/mod.rs is compiled into both lib and bin, so it must not own observability"
    );
    assert!(
        main.contains("#[path = \"api/observability.rs\"]"),
        "the server declaration must resolve src/api/observability.rs explicitly"
    );
}
