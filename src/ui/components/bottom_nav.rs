//! Bottom tab bar.
//!
//! All ten sections are addressable directly from the bar. Loop #6 had
//! collapsed it to four tabs plus a "More" sheet because ten content-sized
//! slots overflowed the viewport and the trailing tabs were clipped; the fix
//! is layout, not fewer tabs — `.bottom-nav > *` gets `flex: 1 1 0` and
//! `min-width: 0` in `styles/main.css`, so every slot is exactly one tenth of
//! the row on any width, and labels ellipsis rather than push the row wide.

use crate::trios::i18n::{
    t, T_NAV_ACCESSORIES, T_NAV_CART, T_NAV_EVENTS, T_NAV_GAME, T_NAV_GARDEN, T_NAV_HOME,
    T_NAV_MENU, T_NAV_PROFILE, T_NAV_SETS, T_NAV_TEA,
};
use crate::ui::components::AccessoriesIcon;
use crate::ui::prefetch;
use crate::ui::routes::Route;
use dioxus::prelude::*;

#[component]
pub fn BottomNav(#[props(default)] cart_count: u32) -> Element {
    let route = use_route::<Route>();
    let lang = crate::ui::lang::current_lang();

    let is_home = matches!(route, Route::Home {});
    let is_menu = matches!(route, Route::Menu {});
    let is_sets = matches!(route, Route::Sets {});
    let is_accessories = matches!(route, Route::Accessories {});
    let is_tea = matches!(route, Route::Tea {});
    let is_garden = matches!(route, Route::Garden {});
    let is_events = matches!(route, Route::Events {});
    let is_cart = matches!(route, Route::Cart {});
    let is_profile = matches!(route, Route::Profile {});
    let is_game = matches!(route, Route::Game {});

    rsx! {
        nav { class: "bottom-nav",
            Link { to: Route::Home {},
                div { class: if is_home { "nav-item active" } else { "nav-item" },
                    onmouseenter: move |_| { prefetch::prefetch_route(&Route::Home {}); },
                    span { class: "nav-icon", "🪵" }
                    span { class: "nav-label", "{t(lang, T_NAV_HOME)}" }
                }
            }
            Link { to: Route::Menu {},
                div { class: if is_menu { "nav-item active" } else { "nav-item" },
                    onmouseenter: move |_| { prefetch::prefetch_route(&Route::Menu {}); },
                    span { class: "nav-icon", "🌿" }
                    span { class: "nav-label", "{t(lang, T_NAV_MENU)}" }
                }
            }
            Link { to: Route::Sets {},
                div { class: if is_sets { "nav-item active" } else { "nav-item" },
                    onmouseenter: move |_| { prefetch::prefetch_route(&Route::Sets {}); },
                    span { class: "nav-icon", "🎁" }
                    span { class: "nav-label", "{t(lang, T_NAV_SETS)}" }
                }
            }
            Link { to: Route::Accessories {},
                div { class: if is_accessories { "nav-item active" } else { "nav-item" },
                    onmouseenter: move |_| { prefetch::prefetch_route(&Route::Accessories {}); },
                    span { class: "nav-icon", AccessoriesIcon {} }
                    span { class: "nav-label", "{t(lang, T_NAV_ACCESSORIES)}" }
                }
            }
            Link { to: Route::Tea {},
                div { class: if is_tea { "nav-item active" } else { "nav-item" },
                    onmouseenter: move |_| { prefetch::prefetch_route(&Route::Tea {}); },
                    span { class: "nav-icon", "🥤" }
                    span { class: "nav-label", "{t(lang, T_NAV_TEA)}" }
                }
            }
            Link { to: Route::Garden {},
                div { class: if is_garden { "nav-item active" } else { "nav-item" },
                    onmouseenter: move |_| { prefetch::prefetch_route(&Route::Garden {}); },
                    span { class: "nav-icon", "🌱" }
                    span { class: "nav-label", "{t(lang, T_NAV_GARDEN)}" }
                }
            }
            Link { to: Route::Events {},
                div { class: if is_events { "nav-item active" } else { "nav-item" },
                    onmouseenter: move |_| { prefetch::prefetch_route(&Route::Events {}); },
                    span { class: "nav-icon", "📅" }
                    span { class: "nav-label", "{t(lang, T_NAV_EVENTS)}" }
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
            Link { to: Route::Game {},
                div { class: if is_game { "nav-item active" } else { "nav-item" },
                    onmouseenter: move |_| { prefetch::prefetch_route(&Route::Game {}); },
                    span { class: "nav-icon", "🎮" }
                    span { class: "nav-label", "{t(lang, T_NAV_GAME)}" }
                }
            }
        }
    }
}
