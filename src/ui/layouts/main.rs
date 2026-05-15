use dioxus::prelude::*;
use crate::ui::components::bottom_nav::BottomNav;

#[component]
pub fn MainLayout(props: MainLayoutProps) -> Element {
    let cart_count = props.cart_count.unwrap_or(0) as u32;
    rsx! {
        div { class: "app-container",
            main { class: "main-content",
                { props.children }
            }
            BottomNav { cart_count }
        }
    }
}

#[derive(Props, PartialEq, Clone)]
pub struct MainLayoutProps {
    #[props(default)]
    cart_count: Option<i32>,
    children: Element,
}
