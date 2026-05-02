// src/web.rs - WASM binary entry point (fallback, Trunk uses lib.rs)
fn main() {
    // Trunk now builds the library target (src/lib.rs) with #[wasm_bindgen(start)]
    // This file is kept as a fallback but shouldn't be used by Trunk
    println!("woody-weed-bot binary — use Trunk with library target for WASM");
}
