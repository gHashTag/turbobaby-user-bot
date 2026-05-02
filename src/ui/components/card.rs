// Card Component with Pixel Art Style
use dioxus::prelude::*;

#[derive(Props, PartialEq, Clone)]
pub struct CardProps {
    #[props(default)]
    variant: CardVariant,
    #[props(default = false)]
    clickable: bool,
    #[props(default = false)]
    glow: bool,
    #[props(default)]
    onclick: Option<EventHandler<MouseEvent>>,
    children: Element,
    #[props(default)]
    class: Option<String>,
    #[props(default)]
    image_url: Option<String>,
}

#[derive(PartialEq, Clone, Copy, Default)]
pub enum CardVariant {
    #[default]
    Default,
    Product,
    Plant,
    Quest,
    Member,
}

impl CardVariant {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Product => "product",
            Self::Plant => "plant",
            Self::Quest => "quest",
            Self::Member => "member",
        }
    }
}

#[component]
pub fn Card(props: CardProps) -> Element {
    let base_class = "card";
    let variant_class = format!("card-{}", props.variant.as_str());
    let click_class = if props.clickable { "card-clickable" } else { "" };
    let glow_class = if props.glow { "card-glow" } else { "" };
    let custom_class = props.class.unwrap_or_default();
    let click_handler = props.onclick;

    rsx! {
        div {
            class: "{base_class} {variant_class} {click_class} {glow_class} {custom_class}",
            onclick: move |e| {
                if props.clickable {
                    if let Some(handler) = &click_handler {
                        handler.call(e);
                    }
                }
            },
            if let Some(url) = &props.image_url {
                div { class: "card-image",
                    img {
                        src: "{url}",
                        alt: "Product image",
                        loading: "lazy"
                    }
                }
            }
            { props.children }
        }
    }
}
