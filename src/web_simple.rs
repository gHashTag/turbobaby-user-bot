// Simple WASM entry point for testing
#![allow(unused_imports)]

fn main() {
    dioxus_web::launch::launch_app(crate::ui::test_simple::SimpleTest);
}
