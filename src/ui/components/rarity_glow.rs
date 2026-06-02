// RarityGlow — wrapper component that applies border-glow by rarity level
use dioxus::prelude::*;

/// Rarity tier
#[derive(PartialEq, Clone, Copy, Default)]
pub enum Rarity {
    #[default]
    Common,
    Rare,
    Epic,
    Legendary,
}

impl Rarity {
    /// Returns the CSS class that provides the glow effect
    pub fn css_class(&self) -> &'static str {
        match self {
            Self::Common => "rarity-glow rarity-glow-common",
            Self::Rare => "rarity-glow rarity-glow-rare",
            Self::Epic => "rarity-glow rarity-glow-epic",
            Self::Legendary => "rarity-glow rarity-glow-legendary",
        }
    }

    /// Matching badge variant string for display
    pub fn label(&self) -> &'static str {
        match self {
            Self::Common => "Common",
            Self::Rare => "Rare",
            Self::Epic => "Epic",
            Self::Legendary => "Legendary",
        }
    }

    /// Parse from a string (e.g. "rare") — infallible (unknown variants
    /// fall back to `Common`). Named distinctly from `FromStr::from_str`
    /// since that trait returns `Result<Self, E>` and we deliberately
    /// don't surface errors here.
    pub fn from_str_lossy(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "rare" => Self::Rare,
            "epic" => Self::Epic,
            "legendary" => Self::Legendary,
            _ => Self::Common,
        }
    }
}

#[derive(Props, PartialEq, Clone)]
pub struct RarityGlowProps {
    pub rarity: Rarity,
    pub children: Element,
    #[props(default)]
    pub class: Option<String>,
    /// HTML tag to render the wrapper as (default "div")
    #[props(default)]
    pub tag: Option<String>,
}

#[component]
pub fn RarityGlow(props: RarityGlowProps) -> Element {
    let rarity_class = props.rarity.css_class();
    let custom_class = props.class.as_deref().unwrap_or_default();

    // We always render a div wrapper — lightweight enough for all use-cases.
    rsx! {
        div {
            class: "{rarity_class} {custom_class}",
            { props.children }
        }
    }
}
