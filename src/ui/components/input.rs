use dioxus::prelude::*;

#[derive(Props, PartialEq, Clone)]
pub struct InputProps {
    #[props(default)]
    pub value: String,
    #[props(default)]
    pub placeholder: String,
    #[props(default)]
    pub input_type: InputType,
    #[props(default)]
    pub on_input: EventHandler<String>,
    /// Optional visible label rendered as a `<label>` above the field.
    #[props(default)]
    pub label: Option<String>,
    /// Optional accessible name when no visible label is used.
    #[props(default)]
    pub aria_label: Option<String>,
    /// Marks the field as invalid for screen readers.
    #[props(default = false)]
    pub aria_invalid: bool,
    /// Optional id of an element describing the error/hint.
    #[props(default)]
    pub aria_described_by: Option<String>,
    /// Optional input id to pair with a `<label>`.
    #[props(default)]
    pub id: Option<String>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum InputType {
    #[default]
    Text,
    Email,
    Password,
    Number,
}

#[component]
pub fn Input(props: InputProps) -> Element {
    let input_id = props.id.clone().unwrap_or_else(|| {
        props
            .label
            .clone()
            .unwrap_or_default()
            .replace(' ', "_")
            .to_lowercase()
    });
    let label_node = props.label.clone().map(|label_text| {
        rsx! {
            label {
                r#for: "{input_id}",
                style: "display:block;font-size:12px;color:#8b8b9e;margin-bottom:4px;",
                "{label_text}"
            }
        }
    });
    rsx! {
        div { style: "width:100%;",
            { label_node }
            input {
                id: "{input_id}",
                r#type: match props.input_type {
                    InputType::Password => "password",
                    InputType::Email => "email",
                    InputType::Number => "number",
                    _ => "text",
                },
                r#placeholder: props.placeholder,
                r#value: props.value,
                r#oninput: move |e| {
                    props.on_input.call(e.value());
                },
                class: "pixel-input",
                "aria-label": props.aria_label.clone().unwrap_or_default(),
                "aria-invalid": if props.aria_invalid { "true" } else { "false" },
                "aria-describedby": props.aria_described_by.clone().unwrap_or_default(),
            }
        }
    }
}
