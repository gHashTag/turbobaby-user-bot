// Chip / Filter Tag Component
use dioxus::prelude::*;

/// Color accent when selected
#[derive(PartialEq, Clone, Default)]
pub enum ChipColor {
    #[default]
    Green,
    Purple,
    Gold,
}

impl ChipColor {
    pub fn selected_class(&self) -> &'static str {
        match self {
            Self::Green  => "selected",
            Self::Purple => "selected chip-purple",
            Self::Gold   => "selected chip-gold",
        }
    }
}

#[derive(Props, PartialEq, Clone)]
pub struct ChipProps {
    pub label: String,
    #[props(default = false)]
    pub selected: bool,
    #[props(default)]
    pub on_click: EventHandler<MouseEvent>,
    /// Optional leading icon/emoji
    #[props(default)]
    pub icon: Option<String>,
    /// Show ×-close button (useful for removable chips)
    #[props(default = false)]
    pub removable: bool,
    #[props(default)]
    pub on_remove: EventHandler<MouseEvent>,
    #[props(default)]
    pub color: ChipColor,
    #[props(default)]
    pub class: Option<String>,
}

#[component]
pub fn Chip(props: ChipProps) -> Element {
    let selected_class = if props.selected {
        props.color.selected_class()
    } else {
        ""
    };
    let custom_class = props.class.as_deref().unwrap_or_default();

    rsx! {
        button {
            class: "chip {selected_class} {custom_class}",
            r#type: "button",
            "data-selected": if props.selected { "true" } else { "false" },
            onclick: move |e| props.on_click.call(e),

            if let Some(ref icon) = props.icon {
                span { class: "chip-icon", "{icon}" }
            }

            span { class: "chip-label", "{props.label}" }

            if props.removable {
                span {
                    class: "chip-close",
                    "aria-label": "Удалить",
                    onclick: move |e| {
                        e.stop_propagation();
                        props.on_remove.call(e);
                    },
                    "×"
                }
            }
        }
    }
}

/// Chip group — a horizontal scrollable row of chips
#[derive(Props, PartialEq, Clone)]
pub struct ChipGroupProps {
    children: Element,
    #[props(default)]
    pub class: Option<String>,
}

#[component]
pub fn ChipGroup(props: ChipGroupProps) -> Element {
    let custom_class = props.class.as_deref().unwrap_or_default();
    rsx! {
        div {
            class: "filter-tag-group {custom_class}",
            { props.children }
        }
    }
}
