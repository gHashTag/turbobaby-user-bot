/// Type-safe asset path constants for all migrated assets.
///
/// Strain images — served from /assets/<filename>
pub mod strains {
    pub const BANANA_FRITTER: &str = "/assets/Banana-fritter.webp";
    pub const BLACK_MAMBA: &str = "/assets/Black-Mamba.webp";
    pub const COLT_45: &str = "/assets/Colt-45.webp";
    pub const DIPZ: &str = "/assets/Dipz.webp";
    pub const JOKER_CANDY: &str = "/assets/Joker-candy.jpeg";
    pub const LA_ULTRA: &str = "/assets/LA-Ultra.webp";
    pub const MAC_1: &str = "/assets/Mac-1.jpeg";
    pub const MIAMI_VICE: &str = "/assets/Miami-Vice.webp";
    pub const MILK_MONKEY: &str = "/assets/Milk-Monkey.webp";
    pub const SUPER_BOOF: &str = "/assets/Super-Boof.webp";
    pub const SUPER_LEMON_HAZE: &str = "/assets/Super-Lemon-Haze.webp";
    pub const SUPER_RUNTZ: &str = "/assets/Super-Runtz.webp";
}

/// Game sprites — served from /assets/game/<number>.png
pub mod game {
    pub const SPRITE_1: &str = "/assets/game/1.png";
    pub const SPRITE_2: &str = "/assets/game/2.png";
    pub const SPRITE_3: &str = "/assets/game/3.png";
    pub const SPRITE_4: &str = "/assets/game/4.png";
    pub const SPRITE_5: &str = "/assets/game/5.png";
    pub const SPRITE_6: &str = "/assets/game/6.png";
    pub const SPRITE_7: &str = "/assets/game/7.png";
    pub const SPRITE_8: &str = "/assets/game/8.png";
    pub const SPRITE_9: &str = "/assets/game/9.png";
    pub const SPRITE_10: &str = "/assets/game/10.png";
    pub const SPRITE_11: &str = "/assets/game/11.png";
    pub const SPRITE_12: &str = "/assets/game/12.png";
    pub const SPRITE_13: &str = "/assets/game/13.png";
    pub const SPRITE_14: &str = "/assets/game/14.png";
}

/// Member card images — served from /assets/member-cards/.
///
/// `*-member-full.webp` are the full 1536×1024 RGBA cards used as the
/// background of the profile screen. Cycle #15 replaced the 3 MB lossless
/// PNG originals with lossy q=80 webp, saving ~8 MB per profile load.
/// `*-member.webp` are small bud thumbnails (~70 KB) — distinct images,
/// not lower-res versions of the full cards.
pub mod member_cards {
    pub const BRASS: &str = "/assets/member-cards/brass-member-full.webp";
    pub const SILVER: &str = "/assets/member-cards/silver-member-full.webp";
    pub const GOLD: &str = "/assets/member-cards/gold-member-full.webp";
    pub const BRONZE_WEBP: &str = "/assets/member-cards/bronze-member.webp";
    pub const SILVER_WEBP: &str = "/assets/member-cards/silver-member.webp";
    pub const GOLD_WEBP: &str = "/assets/member-cards/gold-member.webp";
}

/// Logo and favicon
pub mod logo {
    pub const MAIN: &str = "/assets/logo.jpg";
    pub const FAVICON: &str = "/assets/favicon.svg";
}

/// UI icons — served from /assets/icons/
pub mod icons {
    pub const PLANT: &str = "/assets/icons/plant.svg";
    pub const CART: &str = "/assets/icons/shopping-cart.svg";
    pub const USER: &str = "/assets/icons/user.svg";
}

/// Product pack images — served from /assets/packs/
pub mod packs {
    pub const INDICA: &str = "/assets/packs/indica_pack.webp";
    pub const SATIVA: &str = "/assets/packs/sativa_pack.webp";
    pub const STARTER: &str = "/assets/packs/starter_pack.webp";
}

/// Miscellaneous assets
pub const AVA: &str = "/assets/ava.webp";

/// Returns the asset path for a strain by its display name.
/// Returns `None` if the strain name is not recognised.
pub fn strain_image_url(name: &str) -> Option<&'static str> {
    match name {
        "Banana Fritter" => Some(strains::BANANA_FRITTER),
        "Black Mamba" => Some(strains::BLACK_MAMBA),
        "Colt 45" => Some(strains::COLT_45),
        "Dipz" => Some(strains::DIPZ),
        "Joker Candy" => Some(strains::JOKER_CANDY),
        "LA Ultra" => Some(strains::LA_ULTRA),
        "Mac 1" => Some(strains::MAC_1),
        "Miami Vice" => Some(strains::MIAMI_VICE),
        "Milk Monkey" => Some(strains::MILK_MONKEY),
        "Super Boof" => Some(strains::SUPER_BOOF),
        "Super Lemon Haze" => Some(strains::SUPER_LEMON_HAZE),
        "Super Runtz" => Some(strains::SUPER_RUNTZ),
        _ => None,
    }
}
