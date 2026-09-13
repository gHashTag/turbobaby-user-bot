//! Bottom tab bar.
//!
//! The customer bar contains only the five live TurboBaby surfaces. Legacy
//! cannabis routes remain addressable for compatibility while the migration
//! completes, but are deliberately not advertised from the bike catalog.

use crate::trios::i18n::{t, T_NAV_CART, T_NAV_FLEET, T_NAV_ORDERS, T_NAV_PROFILE, T_NAV_RIDE};
use crate::ui::prefetch;
use crate::ui::routes::Route;
use dioxus::prelude::*;

#[component]
pub fn BottomNav(#[props(default)] cart_count: u32) -> Element {
    let route = use_route::<Route>();
    let lang = crate::ui::lang::current_lang();

    let is_fleet = matches!(route, Route::Home {} | Route::Menu {});
    let is_ride = matches!(route, Route::Ride {} | Route::Skate {});
    let is_orders = matches!(route, Route::Orders {} | Route::OrderDetail { .. });
    let is_cart = matches!(route, Route::Cart {});
    let is_profile = matches!(route, Route::Profile {});

    rsx! {
        nav { class: "bottom-nav",
            Link { to: Route::Home {},
                div { class: if is_fleet { "nav-item active" } else { "nav-item" },
                    onmouseenter: move |_| { prefetch::prefetch_route(&Route::Home {}); },
                    span { class: "nav-icon", "🏍️" }
                    span { class: "nav-label", "{t(lang, T_NAV_FLEET)}" }
                }
            }
            Link { to: Route::Ride {},
                div { class: if is_ride { "nav-item active" } else { "nav-item" },
                    onmouseenter: move |_| { prefetch::prefetch_route(&Route::Ride {}); },
                    span { class: "nav-icon", "🏁" }
                    span { class: "nav-label", "{t(lang, T_NAV_RIDE)}" }
                }
            }
            Link { to: Route::Orders {},
                div { class: if is_orders { "nav-item active" } else { "nav-item" },
                    onmouseenter: move |_| { prefetch::prefetch_route(&Route::Orders {}); },
                    span { class: "nav-icon", "📋" }
                    span { class: "nav-label", "{t(lang, T_NAV_ORDERS)}" }
                }
            }
            Link { to: Route::Cart {},
                div { class: if is_cart { "nav-item active" } else { "nav-item" }, style: "position: relative;",
                    onmouseenter: move |_| { prefetch::prefetch_route(&Route::Cart {}); },
                    span { class: "nav-icon", "🛒" }
                    if cart_count > 0 {
                        span { class: "cart-badge",
                            if cart_count > 9 { "9+" } else { "{cart_count}" }
                        }
                    }
                    span { class: "nav-label", "{t(lang, T_NAV_CART)}" }
                }
            }
            Link { to: Route::Profile {},
                div { class: if is_profile { "nav-item active" } else { "nav-item" },
                    onmouseenter: move |_| { prefetch::prefetch_route(&Route::Profile {}); },
                    span { class: "nav-icon", "👤" }
                    span { class: "nav-label", "{t(lang, T_NAV_PROFILE)}" }
                }
            }
        }
    }
}
