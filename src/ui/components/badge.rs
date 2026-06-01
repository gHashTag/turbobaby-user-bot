// Badge Component — rarity and status badges
use dioxus::prelude::*;

/// Rarity / status variants for badges
#[derive(PartialEq, Clone, Copy, Default)]
pub enum BadgeVariant {
    #[default]
    Neutral,
    Common,
    Rare,
    Epic,
    Legendary,
    Success,
    Warning,
    Info,
    Error,
}

impl BadgeVariant {
    pub fn css_class(&self) -> &'static str {
        match self {
            Self::Neutral => "badge badge-neutral",
            Self::Common => "badge badge-rarity-common",
            Self::Rare => "badge badge-rarity-rare",
            Self::Epic => "badge badge-rarity-epic",
            Self::Legendary => "badge badge-rarity-legendary",
            Self::Success => "badge badge-success",
            Self::Warning => "badge badge-warning",
            Self::Info => "badge badge-info",
            Self::Error => "badge badge-error",
        }
    }

    pub fn default_icon(&self) -> &'static str {
        match self {
            Self::Common => "⚪",
            Self::Rare => "🔵",
            Self::Epic => "🟣",
            Self::Legendary => "🌟",
            Self::Success => "✅",
            Self::Warning => "⚠️",
            Self::Info => "ℹ️",
            Self::Error => "❌",
            Self::Neutral => "",
        }
    }
}

#[derive(PartialEq, Clone, Copy, Default)]
pub enum BadgeSize {
    #[default]
    Default,
    Small,
    Large,
}

impl BadgeSize {
    pub fn css_class(&self) -> &'static str {
        match self {
            Self::Small => "badge-sm",
            Self::Default => "",
            Self::Large => "badge-lg",
        }
    }
}

#[derive(Props, PartialEq, Clone)]
pub struct BadgeProps {
    #[props(default)]
    pub variant: BadgeVariant,
    pub label: String,
    #[props(default)]
    pub icon: Option<String>,
    #[props(default)]
    pub size: BadgeSize,
    #[props(default)]
    pub class: Option<String>,
    /// Show default icon for variant when no icon is explicitly provided
    #[props(default = true)]
    pub show_icon: bool,
}

#[component]
pub fn Badge(props: BadgeProps) -> Element {
    let variant_class = props.variant.css_class();
    let size_class = props.size.css_class();
    let custom_class = props.class.as_deref().unwrap_or_default();

    let icon_str = if let Some(ref icon) = props.icon {
        icon.clone()
    } else if props.show_icon {
        props.variant.default_icon().to_string()
    } else {
        String::new()
    };

    rsx! {
        span {
            class: "{variant_class} {size_class} {custom_class}",
            if !icon_str.is_empty() {
                span { class: "badge-icon", "{icon_str}" }
            }
            span { class: "badge-label", "{props.label}" }
        }
    }
}
