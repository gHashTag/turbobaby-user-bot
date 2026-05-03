// Toast Notification Component
use dioxus::prelude::*;

/// Toast kind / severity
#[derive(PartialEq, Clone, Copy, Default)]
pub enum ToastKind {
    #[default]
    Info,
    Success,
    Warning,
    Error,
}

impl ToastKind {
    pub fn css_class(&self) -> &'static str {
        match self {
            Self::Success => "toast toast-success",
            Self::Error   => "toast toast-error",
            Self::Warning => "toast toast-warning",
            Self::Info    => "toast toast-info",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            Self::Success => "✅",
            Self::Error   => "❌",
            Self::Warning => "⚠️",
            Self::Info    => "ℹ️",
        }
    }

    pub fn title(&self) -> &'static str {
        match self {
            Self::Success => "Успех",
            Self::Error   => "Ошибка",
            Self::Warning => "Внимание",
            Self::Info    => "Информация",
        }
    }
}

#[derive(Props, PartialEq, Clone)]
pub struct ToastProps {
    #[props(default)]
    pub kind: ToastKind,
    pub message: String,
    /// Optional custom title; defaults to kind label
    #[props(default)]
    pub title: Option<String>,
    /// Called when close button is clicked
    #[props(default)]
    pub on_close: EventHandler<MouseEvent>,
    #[props(default)]
    pub class: Option<String>,
}

#[component]
pub fn Toast(props: ToastProps) -> Element {
    let toast_class = props.kind.css_class();
    let custom_class = props.class.as_deref().unwrap_or_default();
    let icon = props.kind.icon();
    let title = props
        .title
        .clone()
        .unwrap_or_else(|| props.kind.title().to_string());

    rsx! {
        div {
            class: "{toast_class} {custom_class}",
            role: "alert",
            "aria-live": "polite",

            span { class: "toast-icon", "{icon}" }

            div { class: "toast-body",
                span { class: "toast-title", "{title}" }
                span { class: "toast-message", "{props.message}" }
            }

            button {
                class: "toast-close",
                "aria-label": "Закрыть",
                onclick: move |e| props.on_close.call(e),
                "×"
            }
        }
    }
}

/// Container that holds multiple toasts in the top-right corner
#[derive(Props, PartialEq, Clone)]
pub struct ToastContainerProps {
    children: Element,
}

#[component]
pub fn ToastContainer(props: ToastContainerProps) -> Element {
    rsx! {
        div {
            class: "toast-container",
            { props.children }
        }
    }
}
