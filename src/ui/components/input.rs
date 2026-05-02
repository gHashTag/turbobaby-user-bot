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
    rsx! {
        input {
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
        }
    }
}
