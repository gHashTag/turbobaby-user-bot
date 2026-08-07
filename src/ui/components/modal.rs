// Modal Component with Pixel Art Style
use crate::trios::i18n::{t, T_MODAL_CLOSE};
use crate::ui::lang;
use dioxus::prelude::*;

#[derive(Props, PartialEq, Clone)]
pub struct ModalProps {
    #[props(default = false)]
    open: bool,
    #[props(default)]
    title: Option<String>,
    #[props(default)]
    on_close: EventHandler<()>,
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
        return rsx! {};
    }

    let lang = lang::current_lang();
    let size_class = format!("modal-{}", props.size.as_str());
    // WCAG 4.1.2: announce the overlay as a modal dialog with an accessible name.
    let aria_label = props
        .title
        .clone()
        .unwrap_or_else(|| t(lang, T_MODAL_CLOSE).to_string());
    let close_label = t(lang, T_MODAL_CLOSE).to_string();

    rsx! {
        div {
            class: "modal-overlay",
            onclick: move |_| {
                // Close on overlay click (content has stop_propagation)
                props.on_close.call(());
            },
            div {
                class: "modal {size_class}",
                role: "dialog",
                "aria-modal": "true",
                "aria-label": "{aria_label}",
                tabindex: "-1",
                // WCAG 2.4.3: move focus into the dialog on open.
                onmounted: move |e: Event<MountedData>| {
                    spawn(async move {
                        let _ = e.set_focus(true).await;
                    });
                },
                // WCAG 2.1.2: Escape closes the dialog (keyboard parity with ✕).
                onkeydown: move |e: Event<KeyboardData>| {
                    if e.key() == Key::Escape {
                        props.on_close.call(());
                    }
                },
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
                                "aria-label": "{close_label}",
                                onclick: move |_| props.on_close.call(()),
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
