use dioxus::prelude::*;

#[derive(Clone, Copy, PartialEq, Default)]
pub enum LoadingSize {
    #[default]
    Medium,
    Small,
    Large,
}

impl LoadingSize {
    pub fn as_str(&self) -> &'static str {
        match self {
            LoadingSize::Small => "small",
            LoadingSize::Medium => "medium",
            LoadingSize::Large => "large",
        }
    }
}

#[derive(Props, PartialEq, Clone)]
pub struct LoadingOverlayProps {
    #[props(default)]
    pub size: LoadingSize,
    #[props(default = false)]
    pub is_visible: bool,
    #[props(default)]
    pub text: String,
}

#[component]
pub fn LoadingOverlay(props: LoadingOverlayProps) -> Element {
    let size_class = format!("loading-{}", props.size.as_str());

    rsx! {
        div {
            class: format!("loading-overlay {}", if props.is_visible { "visible" } else { "" }),
            div { class: format!("loading-spinner {}", size_class),
                if !props.text.is_empty() {
                    p { "{props.text}" }
                }
            }
        }
    }
}

#[derive(Props, PartialEq, Clone)]
pub struct LoadingProps {
    #[props(default = false)]
    pub is_loading: bool,
    #[props(default)]
    pub size: LoadingSize,
    #[props(default)]
    pub text: String,
}

#[component]
pub fn Loading(props: LoadingProps) -> Element {
    rsx! {
        LoadingOverlay {
            is_visible: props.is_loading,
            size: props.size,
            text: props.text,
        }
    }
}
