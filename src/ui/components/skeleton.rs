// Skeleton Loader Component — shimmer placeholder
use dioxus::prelude::*;

/// Preset skeleton shapes
#[derive(PartialEq, Clone, Default)]
pub enum SkeletonShape {
    #[default]
    Rectangle,
    Text,
    TextSm,
    Title,
    Avatar,
    AvatarLg,
    Card,
    Button,
}

impl SkeletonShape {
    pub fn css_class(&self) -> &'static str {
        match self {
            Self::Rectangle => "skeleton-loader",
            Self::Text      => "skeleton-loader skeleton-text",
            Self::TextSm    => "skeleton-loader skeleton-text-sm",
            Self::Title     => "skeleton-loader skeleton-title",
            Self::Avatar    => "skeleton-loader skeleton-avatar",
            Self::AvatarLg  => "skeleton-loader skeleton-avatar-lg",
            Self::Card      => "skeleton-loader skeleton-card",
            Self::Button    => "skeleton-loader skeleton-btn",
        }
    }
}

#[derive(Props, PartialEq, Clone)]
pub struct SkeletonProps {
    /// Inline width (e.g. "120px", "60%"). Ignored for preset shapes that set width.
    #[props(default)]
    pub width: Option<String>,
    /// Inline height (e.g. "16px"). Ignored for preset shapes that set height.
    #[props(default)]
    pub height: Option<String>,
    #[props(default)]
    pub shape: SkeletonShape,
    #[props(default)]
    pub class: Option<String>,
}

#[component]
pub fn Skeleton(props: SkeletonProps) -> Element {
    let shape_class = props.shape.css_class();
    let custom_class = props.class.as_deref().unwrap_or_default();

    let style = {
        let mut s = String::new();
        if let Some(ref w) = props.width {
            s.push_str(&format!("width:{};", w));
        }
        if let Some(ref h) = props.height {
            s.push_str(&format!("height:{};", h));
        }
        s
    };

    rsx! {
        span {
            class: "{shape_class} {custom_class}",
            style: "{style}",
            "aria-hidden": "true"
        }
    }
}

/// Convenience: a column of skeleton rows with optional label
#[derive(Props, PartialEq, Clone)]
pub struct SkeletonRowProps {
    /// Number of text skeleton lines to render
    #[props(default = 3)]
    pub lines: u8,
    #[props(default)]
    pub class: Option<String>,
}

#[component]
pub fn SkeletonRow(props: SkeletonRowProps) -> Element {
    let custom_class = props.class.as_deref().unwrap_or_default();
    rsx! {
        div {
            class: "skeleton-row {custom_class}",
            for _i in 0..props.lines {
                Skeleton { shape: SkeletonShape::Text }
            }
        }
    }
}
