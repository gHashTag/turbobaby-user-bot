use dioxus::prelude::*;

#[derive(Props, PartialEq, Clone)]
pub struct MobileLayoutProps {
    children: Element,
}

#[component]
pub fn MobileLayout(props: MobileLayoutProps) -> Element {
    rsx! {
        div { class: "mobile-layout",
            div { class: "mobile-content",
                { props.children }
            }
        }
    }
}
