// Global State Management with Dioxus Signals
use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

/// Shopping cart item
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CartItem {
    pub id: String,
    pub name: String,
    pub price: f64,
    pub quantity: u32,
    pub image_url: Option<String>,
}

/// Shopping cart
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Cart {
    pub items: Vec<CartItem>,
    pub total: f64,
}

impl Cart {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            total: 0.0,
        }
    }

    pub fn add_item(&mut self, item: CartItem) {
        if let Some(existing) = self.items.iter_mut().find(|i| i.id == item.id) {
            existing.quantity += item.quantity;
        } else {
            self.items.push(item);
        }
        self.recalculate_total();
    }

    pub fn remove_item(&mut self, id: &str) {
        self.items.retain(|i| i.id != id);
        self.recalculate_total();
    }

    pub fn clear(&mut self) {
        self.items.clear();
        self.total = 0.0;
    }

    pub fn recalculate_total(&mut self) {
        self.total = self.items.iter().map(|i| i.price * i.quantity as f64).sum();
    }
}

impl Default for Cart {
    fn default() -> Self {
        Self::new()
    }
}

/// Application language
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Language {
    Russian,
    English,
    Thai,
    Chinese,
    Hebrew,
    German,
    French,
    Spanish,
}

impl Language {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Russian => "ru",
            Self::English => "en",
            Self::Thai => "th",
            Self::Chinese => "zh",
            Self::Hebrew => "he",
            Self::German => "de",
            Self::French => "fr",
            Self::Spanish => "es",
        }
    }

    pub fn flag(&self) -> &'static str {
        match self {
            Self::Russian => "🇷🇺",
            Self::English => "🇬🇧",
            Self::Thai => "🇹🇭",
            Self::Chinese => "🇨🇳",
            Self::Hebrew => "🇮🇱",
            Self::German => "🇩🇪",
            Self::French => "🇫🇷",
            Self::Spanish => "🇪🇸",
        }
    }
}

impl Default for Language {
    fn default() -> Self {
        Self::English
    }
}

/// Hook to access cart state
pub fn use_cart_state() -> Signal<Cart> {
    use_context::<Signal<Cart>>()
}

/// Provider component for cart state
#[component]
pub fn CartStateProvider(children: Element) -> Element {
    let cart = use_memo(|| Cart::new());
    provide_context(cart);
    rsx! {
        { children }
    }
}

/// Current route signal (shared across app)
#[component]
pub fn RouteProvider(children: Element) -> Element {
    let route = use_memo(|| "/".to_string());
    provide_context(route);
    rsx! {
        { children }
    }
}

/// Set the current route (navigate)
pub fn set_route(route: &str) {
    if let Some(mut signal) = try_use_context::<Signal<String>>() {
        *signal.write() = route.to_string();
    }
}

/// Hook to get current route
pub fn use_current_route() -> Signal<String> {
    use_context::<Signal<String>>()
}

/// Provider component for language state
#[component]
pub fn LanguageProvider(children: Element) -> Element {
    let lang = use_signal(Language::default);
    provide_context(lang);
    rsx! {
        { children }
    }
}

/// Hook to get language
pub fn use_language() -> Signal<Language> {
    use_context::<Signal<Language>>()
}
