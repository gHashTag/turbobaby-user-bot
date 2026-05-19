// Modal Component with Pixel Art Style
use dioxus::prelude::*;

#[derive(Props, PartialEq, Clone)]
pub struct ModalProps {
    #[props(default = false)]
    open: bool,
    #[props(default)]
    title: Option<String>,
    #[props(default)]
    on_close: EventHandler<MouseEvent>,
    children: Element,
    #[props(default)]
    size: ModalSize,
    #[props(default = false)]
    show_close: bool,
}

#[derive(PartialEq, Clone, Copy, Default)]
pub enum ModalSize {
    #[default]
    Medium,
    Small,
    Large,
    Full,
}

impl ModalSize {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Small => "sm",
            Self::Medium => "md",
            Self::Large => "lg",
            Self::Full => "full",
        }
    }
}

#[component]
pub fn Modal(props: ModalProps) -> Element {
    if !props.open {
        return rsx! {  };
    }

    let size_class = format!("modal-{}", props.size.as_str());

    rsx! {
        div {
            class: "modal-overlay",
            onclick: move |e| {
                // Note: In Dioxus 0.6, event target comparison works differently
                // For now, we'll just close on any click
                props.on_close.call(e);
            },
            div {
                class: "modal {size_class}",
                onclick: move |e| { e.stop_propagation(); },
                if props.show_close || props.title.is_some() {
                    div {
                        class: "modal-header",
                        if let Some(title) = &props.title {
                            h2 { class: "modal-title", "{title}" }
                        }
                        if props.show_close {
                            button {
                                class: "modal-close",
                                onclick: move |e| props.on_close.call(e),
                                "×"
                            }
                        }
                    }
                }
                div { class: "modal-body", { props.children } }
            }
        }
    }
}
