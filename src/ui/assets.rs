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
//! The image *files* those constants pointed at are still on disk. Deleting
//! shipped artwork is the owner's call, not a consequence of tidying up the
//! code that stopped pointing at it.

/// Logo and favicon.
///
/// `FAVICON` stood here as `/assets/favicon.svg`. The file is live — it is
/// `index.html` that names it — but no Rust ever read the constant.
pub mod logo {
    pub const MAIN: &str = "/assets/logo.jpg";
}

/// Member card images — served from /assets/member-cards/.
///
/// UNREFERENCED since the loyalty ladder became wheels: the artwork behind
/// both sets is a cannabis bud (`*-member-full.webp` was the 1536×1024 hero
/// card on the profile screen, `*-member.webp` the ~70 KB tier thumbnail),
/// which is the wrong identity for a bike shop. `Tier::full_card_image` and
/// `Tier::wheel_image` in `ui/screens/profile_screen.rs` now draw a
/// tier-coloured wheel inline instead. The files and the constants are kept
/// until the owner signs off on the vector art.
pub mod member_cards {
    pub const BRASS: &str = "/assets/member-cards/brass-member-full.webp";
    pub const SILVER: &str = "/assets/member-cards/silver-member-full.webp";
    pub const GOLD: &str = "/assets/member-cards/gold-member-full.webp";
    pub const BRONZE_WEBP: &str = "/assets/member-cards/bronze-member.webp";
    pub const SILVER_WEBP: &str = "/assets/member-cards/silver-member.webp";
    pub const GOLD_WEBP: &str = "/assets/member-cards/gold-member.webp";
}
