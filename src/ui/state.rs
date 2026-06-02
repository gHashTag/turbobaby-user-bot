// Global State Management with Dioxus Signals
use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CartItemType {
    Strain,
    Accessory,
    Tea,
    Set,
}

/// Shopping cart item
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CartItem {
    pub id: String,
    pub name: String,
    pub price: f64,
    pub quantity: u32,
    pub image_url: Option<String>,
    pub item_type: CartItemType,
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
            existing.quantity = existing.quantity.saturating_add(item.quantity);
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
        self.total = self
            .items
            .iter()
            .map(|i| {
                let price = if i.price.is_finite() {
                    i.price.max(0.0)
                } else {
                    0.0
                };
                price * i.quantity as f64
            })
            .sum();
    }
}

impl Default for Cart {
    fn default() -> Self {
        Self::new()
    }
}

/// Hook to access cart state
pub fn use_cart_state() -> Signal<Cart> {
    use_context::<Signal<Cart>>()
}

/// Provider component for cart state
#[component]
pub fn CartStateProvider(children: Element) -> Element {
    let cart = use_memo(Cart::new);
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
