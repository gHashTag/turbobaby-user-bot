// Global State Management with Dioxus Signals
use crate::ui::api::types::ServerCartItem;
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
    /// The unit price, or `None` when nobody published one.
    ///
    /// `None` is not free and it is not zero: it is the line the shop cannot
    /// price, and every renderer shows it as a dash beside the sentence D11
    /// requires. `#[serde(default)]` is deliberate — a cart persisted to
    /// localStorage by a bundle that predates this field's `Option` carries a
    /// plain number, which deserialises straight into `Some`, and a cart that
    /// somehow carries no `price` key at all restores as an absence rather
    /// than failing the whole restore.
    #[serde(default)]
    pub price: Option<f64>,
    pub quantity: u32,
    pub image_url: Option<String>,
    pub item_type: CartItemType,
    /// A3: per-drink fulfillment — Some("dine_in") / Some("takeaway") for drinks,
    /// None for everything else. Chosen via a toggle on drink cart lines.
    #[serde(default)]
    pub fulfillment: Option<String>,
}

impl CartItem {
    /// Loop #11/15: convert a server-side cart line to the local UI model.
    /// Shared helper so app.rs and cart_screen.rs do not drift.
    pub fn from_server(item: ServerCartItem) -> Option<Self> {
        let item_type = match item.kind.as_str() {
            "strain" => CartItemType::Strain,
            "accessory" => CartItemType::Accessory,
            "tea" => CartItemType::Tea,
            "set" => CartItemType::Set,
            _ => return None,
        };
        let quantity = item.quantity.max(0) as u32;
        if quantity == 0 {
            return None;
        }
        Some(Self {
            id: item.catalog_id,
            name: item.name,
            // The two absences this function already handled were loud because
            // the LINE disappeared; the price was the silent one. It replaced
            // only a NON-finite value, and the value that actually arrives when
            // the server omits the field is finite — it is zero, which reads as
            // FREE (D9). The line now survives carrying its absence, so the
            // customer sees a line the shop cannot price instead of a free one.
            // `published_money` is the same filter the catalog renders through,
            // shared rather than spelled again (D15).
            price: crate::trios::pricing::published_money(item.unit_price),
            quantity,
            image_url: item.image_url,
            item_type,
            fulfillment: None,
        })
    }

    /// Is this line one the shop can put a number on?
    ///
    /// Exposed so screens branch on the fact instead of parsing a rendered
    /// string for a dash.
    pub fn is_priced(&self) -> bool {
        crate::trios::pricing::published_money(self.price).is_some()
    }
}

/// Shopping cart
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Cart {
    pub items: Vec<CartItem>,
    /// What the cart costs, or `None` when it holds a line nobody priced.
    ///
    /// One unpriced line makes the whole total unknown. Summing the rest is
    /// not a partial answer, it is a wrong one: the figure understates the
    /// cart and presents the understatement as a measured fact. The arithmetic
    /// and that rule live once, in `trios::pricing::cart_total`.
    #[serde(default)]
    pub total: Option<f64>,
}

impl Cart {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            total: Some(0.0),
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
        self.total = Some(0.0);
    }

    pub fn recalculate_total(&mut self) {
        self.total =
            crate::trios::pricing::cart_total(self.items.iter().map(|i| (i.price, i.quantity)));
    }

    /// Does the cart hold a line the shop cannot price?
    ///
    /// The same question as `self.total.is_none()`, named so a screen reads as
    /// what it is asking rather than as a check for a missing number.
    pub fn has_unpriced_line(&self) -> bool {
        self.total.is_none()
    }

    /// The sum of the lines that DO carry a price.
    ///
    /// This is a measured figure — what the priced part of the cart comes to —
    /// and it is never the cart's total: when `has_unpriced_line` is true the
    /// screens show a dash and the checkout refuses to submit, so this number
    /// reaches nothing but the bonus and star caps, which are only offered on a
    /// cart that can be quoted. It exists because those caps are arithmetic on
    /// a subtotal, and threading an `Option` through them would have bought a
    /// branch per cap and no extra honesty.
    pub fn priced_subtotal(&self) -> f64 {
        self.items
            .iter()
            .filter_map(|i| crate::trios::pricing::cart_line_total(i.price, i.quantity))
            .sum()
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
