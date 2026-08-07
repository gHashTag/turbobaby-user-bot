use crate::trios::i18n::{
    t, T_NAV_ACCESSORIES, T_NAV_CART, T_NAV_EVENTS, T_NAV_GAME, T_NAV_GARDEN, T_NAV_HOME,
    T_NAV_MENU, T_NAV_MORE, T_NAV_PROFILE, T_NAV_SETS, T_NAV_TEA,
};
use crate::ui::components::AccessoriesIcon;
use crate::ui::prefetch;
use crate::ui::routes::Route;
use dioxus::prelude::*;

#[component]
pub fn BottomNav(#[props(default)] cart_count: u32) -> Element {
    let route = use_route::<Route>();
    let lang = crate::ui::lang::current_lang();
    let mut show_more = use_signal(|| false);

    let is_home = matches!(route, Route::Home {});
    let is_menu = matches!(route, Route::Menu {});
    let is_cart = matches!(route, Route::Cart {});
    let is_profile = matches!(route, Route::Profile {});
    let is_sets = matches!(route, Route::Sets {});
    let is_accessories = matches!(route, Route::Accessories {});
    let is_tea = matches!(route, Route::Tea {});
    let is_garden = matches!(route, Route::Garden {});
    let is_events = matches!(route, Route::Events {});
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
            button {
                class: "nav-item",
                style: "background:transparent;border:none;padding:0;display:flex;flex-direction:column;align-items:center;cursor:pointer;color:inherit;",
                onclick: move |_| show_more.set(true),
                span { class: "nav-icon", "⋮" }
                span { class: "nav-label", "{t(lang, T_NAV_MORE)}" }
            }
        }

        if show_more() {
            div {
                style: "position:fixed;inset:0;background:rgba(0,0,0,0.75);z-index:1100;display:flex;align-items:flex-end;justify-content:center;",
                onclick: move |_| show_more.set(false),
                div {
                    style: "width:100%;max-width:480px;background:#16213e;border-top:4px solid #39ff14;border-radius:20px 20px 0 0;padding:16px 16px 24px;box-shadow:0 -4px 20px rgba(0,0,0,0.6);",
                    onclick: move |e: Event<MouseData>| e.stop_propagation(),
                    div { style: "display:flex;justify-content:space-between;align-items:center;margin-bottom:12px;",
                        h3 { style: "font-size:16px;font-weight:800;color:#39ff14;margin:0;text-shadow:2px 2px 0 #000;", "{t(lang, T_NAV_MORE)}" }
                        button {
                            style: "width:44px;height:44px;background:transparent;border:none;color:#ff4757;font-size:24px;cursor:pointer;display:flex;align-items:center;justify-content:center;",
                            "aria-label": "{t(lang, T_NAV_MORE)}",
                            onclick: move |_| show_more.set(false),
                            "✕"
                        }
                    }
                    div { style: "display:grid;grid-template-columns:repeat(3, 1fr);gap:12px;",
                        Link { to: Route::Sets {},
                            div {
                                class: if is_sets { "nav-item active" } else { "nav-item" },
                                style: "padding:12px 4px;",
                                onmouseenter: move |_| { prefetch::prefetch_route(&Route::Sets {}); },
                                onclick: move |_| show_more.set(false),
                                span { class: "nav-icon", "🎁" }
                                span { class: "nav-label", "{t(lang, T_NAV_SETS)}" }
                            }
                        }
                        Link { to: Route::Accessories {},
                            div {
                                class: if is_accessories { "nav-item active" } else { "nav-item" },
                                style: "padding:12px 4px;",
                                onmouseenter: move |_| { prefetch::prefetch_route(&Route::Accessories {}); },
                                onclick: move |_| show_more.set(false),
                                span { class: "nav-icon", AccessoriesIcon {} }
                                span { class: "nav-label", "{t(lang, T_NAV_ACCESSORIES)}" }
                            }
                        }
                        Link { to: Route::Tea {},
                            div {
                                class: if is_tea { "nav-item active" } else { "nav-item" },
                                style: "padding:12px 4px;",
                                onmouseenter: move |_| { prefetch::prefetch_route(&Route::Tea {}); },
                                onclick: move |_| show_more.set(false),
                                span { class: "nav-icon", "🥤" }
                                span { class: "nav-label", "{t(lang, T_NAV_TEA)}" }
                            }
                        }
                        Link { to: Route::Garden {},
                            div {
                                class: if is_garden { "nav-item active" } else { "nav-item" },
                                style: "padding:12px 4px;",
                                onmouseenter: move |_| { prefetch::prefetch_route(&Route::Garden {}); },
                                onclick: move |_| show_more.set(false),
                                span { class: "nav-icon", "🌱" }
                                span { class: "nav-label", "{t(lang, T_NAV_GARDEN)}" }
                            }
                        }
                        Link { to: Route::Events {},
                            div {
                                class: if is_events { "nav-item active" } else { "nav-item" },
                                style: "padding:12px 4px;",
                                onmouseenter: move |_| { prefetch::prefetch_route(&Route::Events {}); },
                                onclick: move |_| show_more.set(false),
                                span { class: "nav-icon", "📅" }
                                span { class: "nav-label", "{t(lang, T_NAV_EVENTS)}" }
                            }
                        }
                        Link { to: Route::Game {},
                            div {
                                class: if is_game { "nav-item active" } else { "nav-item" },
                                style: "padding:12px 4px;",
                                onmouseenter: move |_| { prefetch::prefetch_route(&Route::Game {}); },
                                onclick: move |_| show_more.set(false),
                                span { class: "nav-icon", "🎮" }
                                span { class: "nav-label", "{t(lang, T_NAV_GAME)}" }
                            }
                        }
                    }
                }
            }
        }
    }
}
