// Progress Bar Component with gradient fill
use dioxus::prelude::*;

/// Color theme for the progress bar fill
#[derive(PartialEq, Clone, Default)]
pub enum ProgressColor {
    #[default]
    Green,
    Purple,
    Gold,
    Neon,
}

impl ProgressColor {
    pub fn css_class(&self) -> &'static str {
        match self {
            Self::Green  => "progress-green",
            Self::Purple => "progress-purple",
            Self::Gold   => "progress-gold",
            Self::Neon   => "progress-neon",
        }
    }
}

/// Track size
#[derive(PartialEq, Clone, Default)]
pub enum ProgressSize {
    Small,
    #[default]
    Default,
    Large,
    XLarge,
}

impl ProgressSize {
    pub fn css_class(&self) -> &'static str {
        match self {
            Self::Small   => "progress-sm",
            Self::Default => "",
            Self::Large   => "progress-lg",
            Self::XLarge  => "progress-xl",
        }
    }
}

#[derive(Props, PartialEq, Clone)]
pub struct ProgressBarProps {
    /// Current value (0..=max)
    pub value: f32,
    /// Maximum value (default 100.0)
    #[props(default = 100.0)]
    pub max: f32,
    #[props(default)]
    pub color: ProgressColor,
    #[props(default)]
    pub size: ProgressSize,
    /// Optional left label
    #[props(default)]
    pub label: Option<String>,
    /// Show percentage text on the right
    #[props(default = false)]
    pub show_percent: bool,
    #[props(default)]
    pub class: Option<String>,
}

#[component]
pub fn ProgressBar(props: ProgressBarProps) -> Element {
    let max = if props.max <= 0.0 { 100.0 } else { props.max };
    let value = props.value.clamp(0.0, max);
    let pct = (value / max * 100.0) as u32;
    let fill_width = format!("{}%", pct);

    let size_class = props.size.css_class();
    let color_class = props.color.css_class();
    let custom_class = props.class.as_deref().unwrap_or_default();

    let has_label = props.label.is_some() || props.show_percent;

    rsx! {
        div {
            class: "progress-bar-wrapper {custom_class}",
            role: "progressbar",
            "aria-valuenow": "{pct}",
            "aria-valuemin": "0",
            "aria-valuemax": "100",

            if has_label {
                div { class: "progress-bar-label-row",
                    if let Some(ref lbl) = props.label {
                        span { "{lbl}" }
                    }
                    if props.show_percent {
                        span { "{pct}%" }
                    }
                }
            }

            div {
                class: "progress-bar-track {size_class}",
                div {
                    class: "progress-bar-fill {color_class}",
                    style: "width: {fill_width};",
                }
            }
        }
    }
}
