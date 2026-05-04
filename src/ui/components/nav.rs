// Bottom Navigation Component with Pixel Art Style
use dioxus::prelude::*;
use crate::ui::assets;

#[derive(Props, PartialEq, Clone)]
pub struct NavProps {
    #[props(default)]
    current_route: String,
    #[props(default = 0)]
    cart_count: u32,
}

#[derive(Clone, PartialEq)]
pub struct NavItem {
    pub id: &'static str,
    pub label: &'static str,
    pub icon: &'static str,
    pub route: &'static str,
}

const NAV_ITEMS: [NavItem; 6] = [
    NavItem {
        id: "home",
        label: "Home",
        icon: "🏠",
        route: "/",
    },
    NavItem {
        id: "menu",
        label: "Menu",
        icon: "🌿",
        route: "/menu",
    },
    NavItem {
        id: "cart",
        label: "Cart",
        icon: "🛒",
        route: "/cart",
    },
    NavItem {
        id: "garden",
        label: "Garden",
        icon: "🌱",
        route: "/garden",
    },
    NavItem {
        id: "referrals",
        label: "Invite",
        icon: "🎁",
        route: "/referrals",
    },
    NavItem {
        id: "profile",
        label: "Profile",
        icon: "👤",
        route: "/profile",
    },
];

#[component]
pub fn Nav(props: NavProps) -> Element {
    // Pre-calculate active states outside rsx
    let route = props.current_route.clone();
    let cart_count = props.cart_count;

    rsx! {
        nav {
            class: "bottom-nav",
            // Home
            {
                let item = &NAV_ITEMS[0];
                let is_active = route == item.route;
                let active_class = if is_active { "nav-item-active" } else { "" };

                rsx! {
                    a {
                        href: "{item.route}",
                        class: "nav-item {active_class}",
                        onclick: move |e| {
                            e.prevent_default();
                            crate::ui::state::set_route(item.route);
                        },
                        span { class: "nav-icon", "{item.icon}" }
                        span { class: "nav-label", "{item.label}" }
                    }
                }
            }
            // Menu
            {
                let item = &NAV_ITEMS[1];
                let is_active = route == item.route;
                let active_class = if is_active { "nav-item-active" } else { "" };

                rsx! {
                    a {
                        href: "{item.route}",
                        class: "nav-item {active_class}",
                        onclick: move |e| {
                            e.prevent_default();
                            crate::ui::state::set_route(item.route);
                        },
                        span { class: "nav-icon",
                            img {
                                src: "{assets::icons::PLANT}",
                                alt: "Menu",
                                style: "width: 20px; height: 20px;",
                            }
                        }
                        span { class: "nav-label", "{item.label}" }
                    }
                }
            }
            // Cart
            {
                let item = &NAV_ITEMS[2];
                let is_active = route == item.route || route.starts_with("/cart");
                let active_class = if is_active { "nav-item-active" } else { "" };
                let has_badge = cart_count > 0;

                rsx! {
                    a {
                        href: "{item.route}",
                        class: "nav-item {active_class}",
                        onclick: move |e| {
                            e.prevent_default();
                            crate::ui::state::set_route(item.route);
                        },
                        span { class: "nav-icon", "{item.icon}" }
                        if has_badge {
                            span { class: "nav-badge", "{cart_count}" }
                        }
                        span { class: "nav-label", "{item.label}" }
                    }
                }
            }
            // Garden
            {
                let item = &NAV_ITEMS[3];
                let is_active = route == item.route;
                let active_class = if is_active { "nav-item-active" } else { "" };

                rsx! {
                    a {
                        href: "{item.route}",
                        class: "nav-item {active_class}",
                        onclick: move |e| {
                            e.prevent_default();
                            crate::ui::state::set_route(item.route);
                        },
                        span { class: "nav-icon", "{item.icon}" }
                        span { class: "nav-label", "{item.label}" }
                    }
                }
            }
            // Referrals
            {
                let item = &NAV_ITEMS[4];
                let is_active = route == item.route;
                let active_class = if is_active { "nav-item-active" } else { "" };

                rsx! {
                    a {
                        href: "{item.route}",
                        class: "nav-item {active_class}",
                        onclick: move |e| {
                            e.prevent_default();
                            crate::ui::state::set_route(item.route);
                        },
                        span { class: "nav-icon", "{item.icon}" }
                        span { class: "nav-label", "{item.label}" }
                    }
                }
            }
            // Profile
            {
                let item = &NAV_ITEMS[5];
                let is_active = route == item.route;
                let active_class = if is_active { "nav-item-active" } else { "" };

                rsx! {
                    a {
                        href: "{item.route}",
                        class: "nav-item {active_class}",
                        onclick: move |e| {
                            e.prevent_default();
                            crate::ui::state::set_route(item.route);
                        },
                        span { class: "nav-icon",
                            img {
                                src: "{assets::icons::USER}",
                                alt: "Profile",
                                style: "width: 20px; height: 20px;",
                            }
                        }
                        span { class: "nav-label", "{item.label}" }
                    }
                }
            }
        }
    }
}
