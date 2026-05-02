// Main Layout Component
use dioxus::prelude::*;
use crate::ui::components::Nav;

#[derive(PartialEq, Clone, Copy, Default)]
pub enum LayoutNavRoute {
    #[default]
    Home,
    Menu,
    Sets,
    Sommelier,
    Accessories,
    Tea,
    Cart,
    Orders,
    Profile,
    Garden,
    Quest,
}

impl LayoutNavRoute {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Home => "/",
            Self::Menu => "/menu",
            Self::Sets => "/sets",
            Self::Sommelier => "/sommelier",
            Self::Accessories => "/accessories",
            Self::Tea => "/tea",
            Self::Cart => "/cart",
            Self::Orders => "/orders",
            Self::Profile => "/profile",
            Self::Garden => "/garden",
            Self::Quest => "/quest",
        }
    }
}

#[derive(Props, PartialEq, Clone)]
pub struct MainLayoutProps {
    #[props(default)]
    current_route: LayoutNavRoute,
    #[props(default)]
    cart_count: Option<i32>,
    children: Element,
}

#[component]
pub fn MainLayout(props: MainLayoutProps) -> Element {
    let route_str = props.current_route.as_str().to_string();
    let cart_count = props.cart_count.unwrap_or(0) as u32;

    rsx! {
        div { class: "app-container",
            main { class: "main-content",
                { props.children }
            }
            Nav {
                current_route: route_str,
                cart_count: cart_count,
            }
        }
    }
}
