// Empty State Component
use dioxus::prelude::*;

#[derive(Props, PartialEq, Clone)]
pub struct EmptyStateProps {
    /// Emoji or icon string shown large at the top
    pub icon: String,
    pub title: String,
    pub description: String,
    /// Optional call-to-action element (e.g. a Button)
    #[props(default)]
    pub action: Option<Element>,
    #[props(default)]
    pub class: Option<String>,
    /// Wrap in a glass card
    #[props(default = false)]
    pub glass: bool,
}

#[component]
pub fn EmptyState(props: EmptyStateProps) -> Element {
    let glass_class = if props.glass { "card-glass" } else { "" };
    let custom_class = props.class.as_deref().unwrap_or_default();

    rsx! {
        div {
            class: "empty-state {glass_class} {custom_class}",

            div { class: "empty-state-icon", "{props.icon}" }

            h3 { class: "empty-state-title", "{props.title}" }

            p { class: "empty-state-description", "{props.description}" }

            if let Some(action) = props.action {
                div { class: "empty-state-action",
                    { action }
                }
            }
        }
    }
}
