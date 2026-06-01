// Button Component — Pixel Art + Neon variants
use dioxus::prelude::*;

#[derive(PartialEq, Clone, Copy, Default)]
pub enum ButtonVariant {
    #[default]
    Primary,
    Secondary,
    Outline,
    Ghost,
    Danger,
    Pixel,
    // New neon variants (feat/design-system)
    NeonGreen,
    NeonPurple,
    NeonGold,
    NeonRed,
}

impl ButtonVariant {
    /// Returns the CSS class(es) for this variant
    pub fn css_class(&self) -> &'static str {
        match self {
            Self::Primary => "btn btn-primary",
            Self::Secondary => "btn btn-secondary",
            Self::Outline => "btn btn-outline",
            Self::Ghost => "btn btn-ghost",
            Self::Danger => "btn btn-danger",
            Self::Pixel => "btn btn-pixel",
            Self::NeonGreen => "btn btn-neon btn-neon-green",
            Self::NeonPurple => "btn btn-neon btn-neon-purple",
            Self::NeonGold => "btn btn-neon btn-neon-gold",
            Self::NeonRed => "btn btn-neon btn-neon-red",
        }
    }

    // Kept for backward compat
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Primary => "primary",
            Self::Secondary => "secondary",
            Self::Outline => "outline",
            Self::Ghost => "ghost",
            Self::Danger => "danger",
            Self::Pixel => "pixel",
            Self::NeonGreen => "neon-green",
            Self::NeonPurple => "neon-purple",
            Self::NeonGold => "neon-gold",
            Self::NeonRed => "neon-red",
        }
    }
}

#[derive(PartialEq, Clone, Copy, Default)]
pub enum ButtonSize {
    #[default]
    Medium,
    Small,
    Large,
    Full,
}

impl ButtonSize {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Small => "sm",
            Self::Medium => "md",
            Self::Large => "lg",
            Self::Full => "full",
        }
    }
}

#[derive(Props, PartialEq, Clone)]
pub struct ButtonProps {
    #[props(default)]
    pub variant: ButtonVariant,
    #[props(default)]
    pub size: ButtonSize,
    #[props(default = false)]
    pub disabled: bool,
    #[props(default = false)]
    pub loading: bool,
    #[props(default)]
    pub onclick: EventHandler<MouseEvent>,
    pub children: Element,
    #[props(default)]
    pub label: Option<String>,
    #[props(default)]
    pub icon: Option<String>,
    #[props(default)]
    pub class: Option<String>,
}

#[component]
pub fn Button(props: ButtonProps) -> Element {
    let variant_class = props.variant.css_class();
    let size_class = format!("btn-{}", props.size.as_str());
    let custom_class = props.class.as_deref().unwrap_or_default();
    let disabled_class = if props.disabled || props.loading {
        "btn-disabled"
    } else {
        ""
    };

    rsx! {
        button {
            class: "{variant_class} {size_class} {custom_class} {disabled_class}",
            disabled: props.disabled || props.loading,
            onclick: move |e| props.onclick.call(e),

            if props.loading {
                span { class: "btn-spinner", "⏳" }
            } else if let Some(icon) = props.icon.as_deref() {
                span { class: "btn-icon", "{icon}" }
                if let Some(label) = props.label.as_deref() {
                    span { class: "btn-label", "{label}" }
                }
                { props.children }
            } else {
                if let Some(label) = props.label.as_deref() {
                    span { class: "btn-label", "{label}" }
                }
                { props.children }
            }
        }
    }
}
