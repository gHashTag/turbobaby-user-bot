// Button Component with Pixel Art Style
use dioxus::prelude::*;

#[derive(Props, PartialEq, Clone)]
pub struct ButtonProps {
    #[props(default)]
    variant: ButtonVariant,
    #[props(default)]
    size: ButtonSize,
    #[props(default = false)]
    disabled: bool,
    #[props(default = false)]
    loading: bool,
    #[props(default)]
    onclick: EventHandler<MouseEvent>,
    children: Element,
    #[props(default)]
    label: Option<String>,
    #[props(default)]
    icon: Option<String>,
    #[props(default)]
    class: Option<String>,
}

#[derive(PartialEq, Clone, Copy, Default)]
pub enum ButtonVariant {
    #[default]
    Primary,
    Secondary,
    Outline,
    Ghost,
    Danger,
    Pixel,
}

impl ButtonVariant {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Primary => "primary",
            Self::Secondary => "secondary",
            Self::Outline => "outline",
            Self::Ghost => "ghost",
            Self::Danger => "danger",
            Self::Pixel => "pixel",
        }
    }
}

#[derive(PartialEq, Clone, Copy, Default)]
pub enum ButtonSize {
    #[default]
    Medium,
    Small,
    Large,
}

impl ButtonSize {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Small => "sm",
            Self::Medium => "md",
            Self::Large => "lg",
        }
    }
}

#[component]
pub fn Button(props: ButtonProps) -> Element {
    let base_class = "btn";
    let variant_class = format!("btn-{}", props.variant.as_str());
    let size_class = format!("btn-{}", props.size.as_str());
    let custom_class = props.class.as_deref().unwrap_or_default();
    let disabled_class = if props.disabled || props.loading { "btn-disabled" } else { "" };

    let has_icon = props.icon.is_some();
    let has_label = props.label.is_some();

    rsx! {
        button {
            class: "{base_class} {variant_class} {size_class} {custom_class} {disabled_class}",
            disabled: props.disabled || props.loading,
            onclick: move |e| props.onclick.call(e),
            if props.loading {
                span { class: "btn-spinner", "⏳" }
            } else if has_icon {
                span { class: "btn-icon", "{props.icon.as_deref().unwrap()}" }
                if has_label {
                    span { class: "btn-label", "{props.label.as_deref().unwrap()}" }
                }
                { props.children }
            } else {
                if has_label {
                    span { class: "btn-label", "{props.label.as_deref().unwrap()}" }
                }
                { props.children }
            }
        }
    }
}
