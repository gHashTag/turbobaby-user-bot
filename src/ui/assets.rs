//! Type-safe asset paths.
//!
//! This module held 39 constants and a lookup function on 2026-09-16, and
//! exactly one of them — [`logo::MAIN`] — was referenced by any code in the
//! repository. The rest were corpses of the cannabis shop this app was copied
//! from: twelve strain photographs, three pack renders, and the
//! `strain_image_url` name lookup that mapped «Super Lemon Haze» onto one of
//! them. Alongside them sat fourteen `game/N.png` sprite constants that nothing
//! referenced either — not Rust, not `assets/game/*.js`, not `index.html`.
//!
//! None of it produced a warning. A `pub const` inside a `pub mod` of a `pub
//! mod` is reachable from outside the crate by definition, so `dead_code` has
//! nothing to say about it, and `src/ui` is additionally invisible to `cargo
//! test` (`src/lib.rs` gates it on `target_arch = "wasm32"`). The only
//! instrument that could ever have noticed is a source walk, which is why
//! `tests/customer_surface_wiring.rs` now carries one.
//!
//! The image *files* those constants pointed at stayed on disk until the
//! owner ruled on them: deleting shipped artwork was the owner's call, not a
//! consequence of tidying up the code that stopped pointing at it. The ruling
//! came on 2026-09-25 (#12, nothing cannabis-related anywhere), and the files
//! left `assets/` with the rest of the old shop's media; git history keeps
//! them. `tests/no_cannabis_client_wiring.rs` now lists the media `assets/`
//! may serve.

/// Logo and favicon.
///
/// `FAVICON` stood here as `/assets/favicon.svg`. The file is live — it is
/// `index.html` that names it — but no Rust ever read the constant.
pub mod logo {
    pub const MAIN: &str = "/assets/logo.jpg";
}

// `member_cards` stood here: six paths into `/assets/member-cards/`, kept
// UNREFERENCED since the loyalty ladder became wheels (`Tier::full_card_image`
// and `Tier::wheel_image` in `ui/screens/profile_screen.rs` draw a
// tier-coloured wheel inline) and "until the owner signs off on the vector
// art". The artwork behind all six was a cannabis bud. The owner's ruling of
// 2026-09-25 (#12) removed the files, and the constants went with them.
