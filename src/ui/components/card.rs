// Card Component — Pixel Art + Glassmorphism variants
use dioxus::prelude::*;

#[derive(PartialEq, Clone, Copy, Default)]
pub enum CardVariant {
    #[default]
    Default,
    Product,
    Plant,
    Quest,
    Member,
    // New variants (feat/design-system)
    Glass,
    GlassGreen,
    GlassPurple,
    GlassGold,
}

impl CardVariant {
    pub fn css_class(&self) -> &'static str {
        match self {
            Self::Default => "card card-default",
            Self::Product => "card card-product",
            Self::Plant => "card card-plant",
            Self::Quest => "card card-quest",
            Self::Member => "card card-member",
            Self::Glass => "card card-glass",
            Self::GlassGreen => "card card-glass card-glass-green",
            Self::GlassPurple => "card card-glass card-glass-purple",
            Self::GlassGold => "card card-glass card-glass-gold",
        }
    }

    // Kept for backward compat
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Product => "product",
            Self::Plant => "plant",
            Self::Quest => "quest",
            Self::Member => "member",
            Self::Glass => "glass",
            Self::GlassGreen => "glass-green",
            Self::GlassPurple => "glass-purple",
            Self::GlassGold => "glass-gold",
        }
    }
}

#[derive(Props, PartialEq, Clone)]
pub struct CardProps {
    #[props(default)]
    pub variant: CardVariant,
    #[props(default = false)]
    pub clickable: bool,
    #[props(default = false)]
    pub glow: bool,
    #[props(default)]
    pub onclick: Option<EventHandler<MouseEvent>>,
    pub children: Element,
    #[props(default)]
    pub class: Option<String>,
    #[props(default)]
    pub image_url: Option<String>,
}

#[component]
pub fn Card(props: CardProps) -> Element {
    let variant_class = props.variant.css_class();
    let click_class = if props.clickable { "clickable" } else { "" };
    let glow_class = if props.glow { "glow" } else { "" };
    let custom_class = props.class.as_deref().unwrap_or_default();
    let click_handler = props.onclick;

    rsx! {
        div {
            class: "{variant_class} {click_class} {glow_class} {custom_class}",
            onclick: move |e| {
                if props.clickable {
                    if let Some(handler) = &click_handler {
                        handler.call(e);
                    }
                }
            },
            if let Some(url) = &props.image_url {
                if !url.is_empty() && (url.starts_with("http://") || url.starts_with("https://") || (url.starts_with("/") && !url.starts_with("//")))
                {
                    div { class: "card-image",
                        img {
                            src: "{url}?v=2",
                            alt: "Product image",
                            loading: "lazy"
                        }
                    }
                }
            }
            { props.children }
        }
    }
}
