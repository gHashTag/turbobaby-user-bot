// src/web.rs - WASM binary entry point (fallback, Trunk uses lib.rs)

/// Fallback entry point. Trunk builds the library target (`src/lib.rs`) with
/// `#[wasm_bindgen(start)]`; this binary is only a development stub and never
/// runs in the browser where tracing is wired up.
#[allow(clippy::print_stdout)]
fn main() {
    println!("turbobaby-bot binary — use Trunk with library target for WASM");
}
