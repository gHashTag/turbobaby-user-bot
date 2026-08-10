//! E-commerce core for Trios ecosystem

use crate::trios::core::{Error, Result, Timestamp};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Cart item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CartItem {
    pub strain_id: String,
    pub quantity: u32,
    pub is_set: bool,
    pub set_id: Option<String>,
    pub accessory_id: Option<String>,
    pub is_accessory: bool,
    pub tea_id: Option<String>,
    pub is_tea: bool,
    pub tea_set_id: Option<String>,
    pub is_tea_set: bool,
}

impl CartItem {
    pub fn new_strain(strain_id: String, quantity: u32) -> Self {
        Self {
            strain_id,
            quantity,
            is_set: false,
            set_id: None,
            accessory_id: None,
            is_accessory: false,
            tea_id: None,
            is_tea: false,
            tea_set_id: None,
            is_tea_set: false,
        }
    }

    pub fn new_set(set_id: String, quantity: u32) -> Self {
        Self {
            strain_id: String::new(),
            quantity,
            is_set: true,
            set_id: Some(set_id),
            accessory_id: None,
            is_accessory: false,
            tea_id: None,
            is_tea: false,
            tea_set_id: None,
            is_tea_set: false,
        }
    }

    pub fn new_accessory(accessory_id: String, quantity: u32) -> Self {
        Self {
            strain_id: String::new(),
            quantity,
            is_set: false,
            set_id: None,
            accessory_id: Some(accessory_id),
            is_accessory: true,
            tea_id: None,
            is_tea: false,
            tea_set_id: None,
            is_tea_set: false,
        }
    }

    pub fn new_tea(tea_id: String, quantity: u32) -> Self {
        Self {
            strain_id: String::new(),
            quantity,
            is_set: false,
            set_id: None,
            accessory_id: None,
            is_accessory: false,
            tea_id: Some(tea_id),
            is_tea: true,
            tea_set_id: None,
            is_tea_set: false,
        }
    }

    pub fn new_tea_set(tea_set_id: String, quantity: u32) -> Self {
        Self {
            strain_id: String::new(),
            quantity,
            is_set: false,
            set_id: None,
            accessory_id: None,
            is_accessory: false,
            tea_id: None,
            is_tea: false,
            tea_set_id: Some(tea_set_id),
            is_tea_set: true,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.quantity == 0
    }
}

/// Shopping cart
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Cart {
    pub items: Vec<CartItem>,
    pub total_price: i64,
    pub item_count: u32,
}

impl Cart {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_item(&mut self, item: CartItem) {
        self.items.push(item);
        self.recalculate();
    }

    pub fn remove_item(&mut self, index: usize) -> Result<CartItem> {
        if index >= self.items.len() {
            return Err(Error::NotFound("Item not found".to_string()));
        }
        let item = self.items.remove(index);
        self.recalculate();
        Ok(item)
    }

    pub fn update_quantity(&mut self, index: usize, quantity: u32) -> Result<()> {
        if index >= self.items.len() {
            return Err(Error::NotFound("Item not found".to_string()));
        }
        if quantity == 0 {
            self.items.remove(index);
        } else {
            self.items[index].quantity = quantity;
        }
        self.recalculate();
        Ok(())
    }

    pub fn clear(&mut self) {
        self.items.clear();
        self.recalculate();
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    fn recalculate(&mut self) {
        self.item_count = self
            .items
            .iter()
            .fold(0u32, |acc, i| acc.saturating_add(i.quantity));
        // Note: total_price calculation requires product pricing
        // This is a placeholder - actual calculation needs product data
    }

    pub fn get_total_items_count(&self) -> u32 {
        self.items
            .iter()
            .fold(0u32, |acc, i| acc.saturating_add(i.quantity))
    }
}

/// Order status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OrderStatus {
    Pending,
    Confirmed,
    Ready,
    Completed,
    Cancelled,
    Rejected,
}

/// Pickup type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PickupType {
    Pickup,
    Delivery,
}

/// Order
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Order {
    pub id: String,
    pub items: Vec<CartItem>,
    pub total_price: i64,
    pub bonus_used: Option<i64>,
    pub cashback_earned: Option<i64>,
    pub customer_name: String,
    pub customer_phone: String,
    pub customer_telegram: Option<String>,
    pub customer_telegram_id: Option<i64>,
    pub pickup_type: PickupType,
    pub pickup_time: Option<String>,
    pub delivery_address: Option<String>,
    pub status: OrderStatus,
    pub created_at: Timestamp,
}

impl Order {
    pub fn new(
        cart: Cart,
        customer_name: String,
        customer_phone: String,
        pickup_type: PickupType,
    ) -> Self {
        let now = chrono::Utc::now().timestamp_millis();
        Self {
            id: format!("order_{}", now),
            items: cart.items,
            total_price: cart.total_price,
            bonus_used: None,
            cashback_earned: None,
            customer_name,
            customer_phone,
            customer_telegram: None,
            customer_telegram_id: None,
            pickup_type,
            pickup_time: None,
            delivery_address: None,
            status: OrderStatus::Pending,
            created_at: now,
        }
    }

    pub fn with_telegram(mut self, telegram: String, telegram_id: i64) -> Self {
        self.customer_telegram = Some(telegram);
        self.customer_telegram_id = Some(telegram_id);
        self
    }

    pub fn with_delivery(mut self, address: String, pickup_time: String) -> Self {
        self.delivery_address = Some(address);
        self.pickup_time = Some(pickup_time);
        self
    }

    pub fn is_pending(&self) -> bool {
        self.status == OrderStatus::Pending
    }

    pub fn is_completed(&self) -> bool {
        self.status == OrderStatus::Completed
    }

    pub fn can_cancel(&self) -> bool {
        matches!(self.status, OrderStatus::Pending)
    }
}

/// Product pricing data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductPrice {
    pub id: String,
    pub price: i64,
    pub discount_percent: Option<u32>,
}

/// Calculate cart total from product prices
pub fn calculate_cart_total(
    items: &[CartItem],
    prices: &HashMap<String, ProductPrice>,
    happy_hour_discount: Option<u32>,
) -> i64 {
    let mut total = 0i64;

    for item in items {
        let price = if let Some(set_id) = &item.set_id {
            prices.get(set_id).map(|p| p.price).unwrap_or(0)
        } else if item.is_accessory {
            item.accessory_id
                .as_ref()
                .and_then(|id| prices.get(id))
                .map(|p| p.price)
                .unwrap_or(0)
        } else if item.is_tea {
            item.tea_id
                .as_ref()
                .and_then(|id| prices.get(id))
                .map(|p| p.price)
                .unwrap_or(0)
        } else if item.is_tea_set {
            item.tea_set_id
                .as_ref()
                .and_then(|id| prices.get(id))
                .map(|p| p.price)
                .unwrap_or(0)
        } else {
            prices.get(&item.strain_id).map(|p| p.price).unwrap_or(0)
        };

        let line_total = price.saturating_mul(item.quantity as i64);
        total = total.saturating_add(line_total);
    }

    // Apply happy hour discount if active (cap at 100% to avoid negative totals)
    if let Some(discount) = happy_hour_discount {
        let d = discount.min(100);
        if d > 0 {
            total = total.saturating_sub(total.saturating_mul(d as i64) / 100);
        }
    }

    total
}

/// Default country calling code. The shop is on Koh Phangan, so a bare local
/// number typed by a customer is Thai unless it says otherwise.
const DEFAULT_COUNTRY_CODE: &str = "66";

/// How the customer receives the order.
///
/// Pickup is a real, separate flow: the customer chooses a shop and collects
/// in person, so there is no address to give. Modelling it explicitly is what
/// lets [`validate_checkout_for`] stop demanding one.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Fulfillment {
    /// Courier delivery to `address` — the address is mandatory.
    #[default]
    Delivery,
    /// Customer collects at the shop — no address needed.
    Pickup,
}

impl Fulfillment {
    /// Wire value sent to the server and stored on the order.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Delivery => "delivery",
            Self::Pickup => "pickup",
        }
    }

    /// Whether a delivery address must be supplied for this mode.
    pub fn requires_address(self) -> bool {
        matches!(self, Self::Delivery)
    }
}

/// Normalise a customer-typed phone number to E.164 (`+<digits>`).
///
/// Customers type what their phone shows them: `081 234 5678` in Thailand,
/// `8 999 123-45-67` in Russia, `0066…` from a landline. Requiring a literal
/// leading `+` rejected every one of those while the on-screen error only said
/// "min 5 digits", so the order button stayed dead with no way to find out why.
/// Accept the common shapes and convert; reject only what has no chance of
/// being a phone number.
///
/// Returns `None` when the input cannot be read as a phone number.
pub fn normalize_phone(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.len() > 50 {
        return None;
    }
    // Anything that is not a digit or a leading '+' is formatting noise:
    // spaces, dashes, dots, parentheses, non-breaking spaces.
    let had_plus = trimmed.starts_with('+');
    let digits: String = trimmed.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.len() < 5 || digits.len() > 15 {
        return None;
    }

    // Already international: `+66812345678`.
    if had_plus {
        return Some(format!("+{digits}"));
    }
    // IDD prefix: `0066812345678` → `+66812345678`.
    if let Some(rest) = digits.strip_prefix("00") {
        if rest.len() >= 5 {
            return Some(format!("+{rest}"));
        }
        return None;
    }
    // Russian national format: `8XXXXXXXXXX` (11 digits) → `+7XXXXXXXXXX`.
    if digits.len() == 11 && digits.starts_with('8') {
        return Some(format!("+7{}", &digits[1..]));
    }
    // National trunk prefix: `081…` → `+6681…`. Thai mobiles are 10 digits
    // with the trunk 0, landlines 9.
    if let Some(rest) = digits.strip_prefix('0') {
        if rest.len() >= 5 {
            return Some(format!("+{DEFAULT_COUNTRY_CODE}{rest}"));
        }
        return None;
    }
    // Bare international without the plus: `66812345678`.
    Some(format!("+{digits}"))
}

/// A single reason an order cannot be placed right now.
///
/// The checkout screen used to express this as a bare `disabled` attribute: if
/// any one of six conditions failed the button greyed out and said nothing, so
/// a customer with — say — an unticked age box had no way to find out why
/// nothing happened. Naming each reason lets the UI list them all.
///
/// Lives in `trios` rather than in the screen so it compiles (and is tested)
/// on the host target, not only under wasm.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CheckoutBlocker {
    NoTelegram,
    EmptyCart,
    Name,
    Phone,
    Address,
    Age,
}

impl CheckoutBlocker {
    /// Translation key describing how to clear this blocker.
    pub fn message_key(self) -> crate::trios::i18n::Key {
        use crate::trios::i18n::{
            T_CHECKOUT_ERR_ADDRESS, T_CHECKOUT_ERR_AGE, T_CHECKOUT_ERR_ITEMS, T_CHECKOUT_ERR_NAME,
            T_CHECKOUT_ERR_NO_TELEGRAM, T_CHECKOUT_ERR_PHONE_INVALID,
        };
        match self {
            Self::NoTelegram => T_CHECKOUT_ERR_NO_TELEGRAM,
            Self::EmptyCart => T_CHECKOUT_ERR_ITEMS,
            Self::Name => T_CHECKOUT_ERR_NAME,
            Self::Phone => T_CHECKOUT_ERR_PHONE_INVALID,
            Self::Address => T_CHECKOUT_ERR_ADDRESS,
            Self::Age => T_CHECKOUT_ERR_AGE,
        }
    }
}

/// Everything standing between the customer and a placed order.
///
/// Pure and side-effect free, so the gate that silently decided no request
/// would be sent is fully testable. Returned in form order, so the list reads
/// top-to-bottom like the screen does.
pub fn checkout_blockers(
    has_telegram_id: bool,
    name: &str,
    phone: &str,
    address: &str,
    fulfillment: Fulfillment,
    item_count: usize,
    age_confirmed: bool,
) -> Vec<CheckoutBlocker> {
    let mut out = Vec::new();
    if !has_telegram_id {
        out.push(CheckoutBlocker::NoTelegram);
    }
    if item_count == 0 {
        out.push(CheckoutBlocker::EmptyCart);
    }
    if name.trim().is_empty() || name.len() > 200 {
        out.push(CheckoutBlocker::Name);
    }
    if normalize_phone(phone).is_none() || phone.len() > 50 {
        out.push(CheckoutBlocker::Phone);
    }
    if (fulfillment.requires_address() && address.trim().is_empty()) || address.len() > 500 {
        out.push(CheckoutBlocker::Address);
    }
    if !age_confirmed {
        out.push(CheckoutBlocker::Age);
    }
    out
}

/// Validate checkout data for a courier delivery.
///
/// Kept as the delivery-shaped wrapper so existing callers are unchanged;
/// see [`validate_checkout_for`] for the pickup variant.
pub fn validate_checkout(name: &str, phone: &str, address: &str, items: &[CartItem]) -> Result<()> {
    validate_checkout_for(name, phone, address, items, Fulfillment::Delivery)
}

/// Validate checkout data for a given fulfillment mode.
///
/// The address is only required when the order is delivered. A pickup order
/// with an empty address is valid — previously it was not, which made it
/// impossible to place an in-store order at all.
pub fn validate_checkout_for(
    name: &str,
    phone: &str,
    address: &str,
    items: &[CartItem],
    fulfillment: Fulfillment,
) -> Result<()> {
    if name.trim().is_empty() {
        return Err(Error::Validation("Name is required".to_string()));
    }
    if name.trim().len() > 200 {
        return Err(Error::Validation("Name is too long".to_string()));
    }
    if phone.trim().is_empty() {
        return Err(Error::Validation("Phone is required".to_string()));
    }
    if phone.trim().len() > 50 {
        return Err(Error::Validation("Phone is too long".to_string()));
    }
    if normalize_phone(phone).is_none() {
        return Err(Error::Validation("Invalid phone number".to_string()));
    }
    if fulfillment.requires_address() && address.trim().is_empty() {
        return Err(Error::Validation(
            "Delivery address is required".to_string(),
        ));
    }
    if address.trim().len() > 500 {
        return Err(Error::Validation(
            "Delivery address is too long".to_string(),
        ));
    }
    if items.is_empty() {
        return Err(Error::Validation("Cart cannot be empty".to_string()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cart_item_strain() {
        let item = CartItem::new_strain("strain1".to_string(), 2);
        assert_eq!(item.strain_id, "strain1");
        assert_eq!(item.quantity, 2);
        assert!(!item.is_set);
    }

    #[test]
    fn test_cart_item_set() {
        let item = CartItem::new_set("set1".to_string(), 1);
        assert_eq!(item.set_id, Some("set1".to_string()));
        assert!(item.is_set);
    }

    #[test]
    fn test_cart_new() {
        let cart = Cart::new();
        assert!(cart.is_empty());
        assert_eq!(cart.len(), 0);
    }

    #[test]
    fn test_cart_add_item() {
        let mut cart = Cart::new();
        cart.add_item(CartItem::new_strain("strain1".to_string(), 1));
        assert_eq!(cart.len(), 1);
        assert_eq!(cart.get_total_items_count(), 1);
    }

    #[test]
    fn test_cart_remove_item() {
        let mut cart = Cart::new();
        cart.add_item(CartItem::new_strain("strain1".to_string(), 1));
        cart.remove_item(0).unwrap();
        assert!(cart.is_empty());
    }

    #[test]
    fn test_cart_remove_item_invalid() {
        let mut cart = Cart::new();
        assert!(cart.remove_item(0).is_err());
    }

    #[test]
    fn test_cart_update_quantity() {
        let mut cart = Cart::new();
        cart.add_item(CartItem::new_strain("strain1".to_string(), 1));
        cart.update_quantity(0, 5).unwrap();
        assert_eq!(cart.get_total_items_count(), 5);
    }

    #[test]
    fn test_cart_update_quantity_zero_removes() {
        let mut cart = Cart::new();
        cart.add_item(CartItem::new_strain("strain1".to_string(), 1));
        cart.update_quantity(0, 0).unwrap();
        assert!(cart.is_empty());
    }

    #[test]
    fn test_cart_clear() {
        let mut cart = Cart::new();
        cart.add_item(CartItem::new_strain("strain1".to_string(), 1));
        cart.add_item(CartItem::new_set("set1".to_string(), 1));
        cart.clear();
        assert!(cart.is_empty());
    }

    #[test]
    fn test_order_new() {
        let cart = Cart::new();
        let order = Order::new(
            cart,
            "John Doe".to_string(),
            "+1234567890".to_string(),
            PickupType::Pickup,
        );
        assert!(order.is_pending());
        assert!(!order.is_completed());
        assert_eq!(order.customer_name, "John Doe");
    }

    #[test]
    fn test_order_with_telegram() {
        let cart = Cart::new();
        let order = Order::new(
            cart,
            "John Doe".to_string(),
            "+1234567890".to_string(),
            PickupType::Pickup,
        )
        .with_telegram("@johndoe".to_string(), 123456);
        assert_eq!(order.customer_telegram, Some("@johndoe".to_string()));
        assert_eq!(order.customer_telegram_id, Some(123456));
    }

    #[test]
    fn test_calculate_cart_total() {
        let items = vec![
            CartItem::new_strain("strain1".to_string(), 2),
            CartItem::new_set("set1".to_string(), 1),
        ];

        let mut prices = HashMap::new();
        prices.insert(
            "strain1".to_string(),
            ProductPrice {
                id: "strain1".to_string(),
                price: 500,
                discount_percent: None,
            },
        );
        prices.insert(
            "set1".to_string(),
            ProductPrice {
                id: "set1".to_string(),
                price: 1000,
                discount_percent: None,
            },
        );

        let total = calculate_cart_total(&items, &prices, None);
        assert_eq!(total, 2000); // 2*500 + 1*1000
    }

    #[test]
    fn test_calculate_cart_total_with_discount() {
        let items = vec![CartItem::new_strain("strain1".to_string(), 2)];

        let mut prices = HashMap::new();
        prices.insert(
            "strain1".to_string(),
            ProductPrice {
                id: "strain1".to_string(),
                price: 1000,
                discount_percent: None,
            },
        );

        let total = calculate_cart_total(&items, &prices, Some(20));
        assert_eq!(total, 1600); // 2*1000 - 20%
    }

    /// Financial invariant guard: a happy-hour discount must always keep the
    /// total within `[0, subtotal]`, be exactly 0 at 100%, and be monotonically
    /// non-increasing as the discount grows. The None/20% tests above only pin
    /// two points; this locks in the *shape* of the discount curve so a future
    /// edit to the formula (wrong operator, off-by-one, swapping to
    /// non-saturating arithmetic that could panic/overflow, or rounding the
    /// wrong way) is caught.
    ///
    /// Note on over-100%: today the bound is double-protected — `discount` is
    /// `u32` (can't be negative) and `saturating_sub` floors at 0 — so even
    /// without the explicit `.min(100)` the total stays ≥ 0. The clamp is
    /// defense-in-depth/clarity; this test asserts the observable behavior, not
    /// that the clamp is the sole guard.
    #[test]
    fn test_calculate_cart_total_discount_is_clamped_to_100_percent() {
        let items = vec![CartItem::new_strain("strain1".to_string(), 2)];
        let mut prices = HashMap::new();
        prices.insert(
            "strain1".to_string(),
            ProductPrice {
                id: "strain1".to_string(),
                price: 1000,
                discount_percent: None,
            },
        );
        let subtotal = 2000; // 2 * 1000

        // Exactly 100% → free, never negative.
        assert_eq!(
            calculate_cart_total(&items, &prices, Some(100)),
            0,
            "100% discount must zero the total exactly"
        );

        // Over 100% (misconfig) must still clamp to 0 — never go negative.
        for over in [101u32, 150, 200, 1000, u32::MAX] {
            let total = calculate_cart_total(&items, &prices, Some(over));
            assert_eq!(
                total, 0,
                "discount {over}% must clamp to a 0 total, never negative (got {total})"
            );
        }

        // Bounds hold across the whole legal range: 0 <= total <= subtotal,
        // and the result is monotonically non-increasing in the discount.
        let mut prev = subtotal;
        for d in 0u32..=100 {
            let total = calculate_cart_total(&items, &prices, Some(d));
            assert!(
                (0..=subtotal).contains(&total),
                "total {total} out of [0,{subtotal}] for discount {d}%"
            );
            assert!(
                total <= prev,
                "total must not increase as discount grows ({d}%: {total} > prev {prev})"
            );
            prev = total;
        }
    }

    #[test]
    fn test_validate_checkout_valid() {
        let items = vec![CartItem::new_strain("strain1".to_string(), 1)];
        assert!(validate_checkout("John", "+12345", "Koh Phangan", &items).is_ok());
    }

    #[test]
    fn test_validate_checkout_empty_name() {
        let items = vec![CartItem::new_strain("strain1".to_string(), 1)];
        assert!(validate_checkout("", "+12345", "Koh Phangan", &items).is_err());
    }

    #[test]
    fn test_validate_checkout_empty_phone() {
        let items = vec![CartItem::new_strain("strain1".to_string(), 1)];
        assert!(validate_checkout("John", "", "Koh Phangan", &items).is_err());
    }

    #[test]
    fn test_validate_checkout_empty_address() {
        let items = vec![CartItem::new_strain("strain1".to_string(), 1)];
        assert!(validate_checkout("John", "+12345", "", &items).is_err());
    }

    #[test]
    fn test_validate_checkout_empty_cart() {
        assert!(validate_checkout("John", "+12345", "Koh Phangan", &[]).is_err());
    }

    #[test]
    fn test_validate_checkout_invalid_phone() {
        let items = vec![CartItem::new_strain("strain1".to_string(), 1)];
        assert!(validate_checkout("John", "+123", "Koh Phangan", &items).is_err());
    }

    // ---- Phone normalisation -------------------------------------------
    //
    // These are the inputs that used to leave the "Place order" button dead:
    // every one of them is what a customer's own phone shows them.

    #[test]
    fn normalize_phone_accepts_thai_local_mobile() {
        // The single most common real input on Koh Phangan.
        assert_eq!(
            normalize_phone("0812345678").as_deref(),
            Some("+66812345678")
        );
    }

    #[test]
    fn normalize_phone_strips_formatting_noise() {
        for raw in ["081 234 5678", "081-234-5678", "(081) 234.5678"] {
            assert_eq!(
                normalize_phone(raw).as_deref(),
                Some("+66812345678"),
                "{raw} should normalise to the same number"
            );
        }
    }

    #[test]
    fn normalize_phone_keeps_international_input() {
        assert_eq!(
            normalize_phone("+66 81 234 5678").as_deref(),
            Some("+66812345678")
        );
    }

    #[test]
    fn normalize_phone_converts_idd_prefix() {
        assert_eq!(
            normalize_phone("0066812345678").as_deref(),
            Some("+66812345678")
        );
    }

    #[test]
    fn normalize_phone_converts_russian_national_format() {
        assert_eq!(
            normalize_phone("8 999 123-45-67").as_deref(),
            Some("+79991234567")
        );
    }

    #[test]
    fn normalize_phone_adds_plus_to_bare_international() {
        assert_eq!(
            normalize_phone("66812345678").as_deref(),
            Some("+66812345678")
        );
    }

    #[test]
    fn normalize_phone_rejects_junk() {
        for raw in ["", "   ", "abc", "12", "+1", "0", "00"] {
            assert!(
                normalize_phone(raw).is_none(),
                "{raw:?} should not be accepted as a phone number"
            );
        }
    }

    #[test]
    fn normalize_phone_rejects_absurdly_long_input() {
        // 16 digits exceeds E.164's maximum of 15.
        assert!(normalize_phone("1234567890123456").is_none());
    }

    #[test]
    fn normalize_phone_is_idempotent() {
        let once = normalize_phone("0812345678").expect("first pass normalises");
        let twice = normalize_phone(&once).expect("second pass normalises");
        assert_eq!(once, twice);
    }

    // ---- Checkout gate --------------------------------------------------

    #[test]
    fn validate_checkout_accepts_local_phone() {
        // Regression: this exact combination produced a permanently disabled
        // "Place order" button and no request ever reached the server.
        let items = vec![CartItem::new_strain("strain1".to_string(), 1)];
        assert!(validate_checkout("John", "0812345678", "Koh Phangan", &items).is_ok());
    }

    #[test]
    fn validate_checkout_pickup_does_not_require_address() {
        // Regression: an in-store (offline) order was impossible because the
        // delivery address was demanded unconditionally.
        let items = vec![CartItem::new_strain("strain1".to_string(), 1)];
        assert!(
            validate_checkout_for("John", "+66812345678", "", &items, Fulfillment::Pickup).is_ok()
        );
    }

    #[test]
    fn validate_checkout_delivery_still_requires_address() {
        let items = vec![CartItem::new_strain("strain1".to_string(), 1)];
        assert!(
            validate_checkout_for("John", "+66812345678", "", &items, Fulfillment::Delivery)
                .is_err()
        );
    }

    #[test]
    fn validate_checkout_pickup_still_requires_name_phone_and_items() {
        let items = vec![CartItem::new_strain("strain1".to_string(), 1)];
        assert!(
            validate_checkout_for("", "+66812345678", "", &items, Fulfillment::Pickup).is_err(),
            "name is required for pickup too"
        );
        assert!(
            validate_checkout_for("John", "", "", &items, Fulfillment::Pickup).is_err(),
            "phone is required for pickup too"
        );
        assert!(
            validate_checkout_for("John", "+66812345678", "", &[], Fulfillment::Pickup).is_err(),
            "an empty cart is never orderable"
        );
    }

    #[test]
    fn validate_checkout_rejects_over_long_address_even_for_pickup() {
        let items = vec![CartItem::new_strain("strain1".to_string(), 1)];
        let long = "x".repeat(501);
        assert!(
            validate_checkout_for("John", "+66812345678", &long, &items, Fulfillment::Pickup)
                .is_err()
        );
    }

    #[test]
    fn fulfillment_wire_values_are_stable() {
        // The server stores these verbatim; changing them silently would
        // reclassify historical orders.
        assert_eq!(Fulfillment::Delivery.as_str(), "delivery");
        assert_eq!(Fulfillment::Pickup.as_str(), "pickup");
        assert!(Fulfillment::Delivery.requires_address());
        assert!(!Fulfillment::Pickup.requires_address());
        assert_eq!(Fulfillment::default(), Fulfillment::Delivery);
    }

    // ---- The order gate ------------------------------------------------
    //
    // This is the logic that silently decided no POST /api/orders would ever
    // be sent. Each case below is a form a real customer can produce.

    #[test]
    fn local_phone_and_address_place_the_order() {
        // Regression for the reported bug: this exact form produced a
        // permanently disabled button and no request ever left the client.
        let blockers = checkout_blockers(
            true,
            "Дмитрий",
            "0812345678",
            "Baan Tai, Koh Phangan",
            Fulfillment::Delivery,
            1,
            true,
        );
        assert_eq!(blockers, Vec::new());
    }

    #[test]
    fn pickup_needs_no_address() {
        // Regression: ordering offline (collect in store) was impossible.
        let blockers = checkout_blockers(
            true,
            "Дмитрий",
            "+66812345678",
            "",
            Fulfillment::Pickup,
            1,
            true,
        );
        assert_eq!(blockers, Vec::new());
    }

    #[test]
    fn delivery_without_address_is_blocked() {
        let blockers = checkout_blockers(
            true,
            "Дмитрий",
            "+66812345678",
            "",
            Fulfillment::Delivery,
            1,
            true,
        );
        assert_eq!(blockers, vec![CheckoutBlocker::Address]);
    }

    #[test]
    fn unticked_age_box_is_reported() {
        // The one blocker that previously had no on-screen indicator at all.
        let blockers = checkout_blockers(
            true,
            "Дмитрий",
            "0812345678",
            "Baan Tai",
            Fulfillment::Delivery,
            1,
            false,
        );
        assert_eq!(blockers, vec![CheckoutBlocker::Age]);
    }

    #[test]
    fn empty_cart_is_reported() {
        let blockers = checkout_blockers(
            true,
            "Дмитрий",
            "0812345678",
            "Baan Tai",
            Fulfillment::Delivery,
            0,
            true,
        );
        assert_eq!(blockers, vec![CheckoutBlocker::EmptyCart]);
    }

    #[test]
    fn missing_telegram_id_is_reported() {
        let blockers = checkout_blockers(
            false,
            "Дмитрий",
            "0812345678",
            "Baan Tai",
            Fulfillment::Delivery,
            1,
            true,
        );
        assert_eq!(blockers, vec![CheckoutBlocker::NoTelegram]);
    }

    #[test]
    fn every_blocker_is_listed_not_just_the_first() {
        // An empty form must name all six problems at once; surfacing them
        // one at a time is what made the screen feel broken.
        let blockers = checkout_blockers(false, "", "", "", Fulfillment::Delivery, 0, false);
        assert_eq!(
            blockers,
            vec![
                CheckoutBlocker::NoTelegram,
                CheckoutBlocker::EmptyCart,
                CheckoutBlocker::Name,
                CheckoutBlocker::Phone,
                CheckoutBlocker::Address,
                CheckoutBlocker::Age,
            ]
        );
    }

    #[test]
    fn over_long_address_is_blocked_even_for_pickup() {
        let long = "x".repeat(501);
        let blockers = checkout_blockers(
            true,
            "Дмитрий",
            "0812345678",
            &long,
            Fulfillment::Pickup,
            1,
            true,
        );
        assert_eq!(blockers, vec![CheckoutBlocker::Address]);
    }

    #[test]
    fn gate_agrees_with_validate_checkout_for() {
        // Two gates guard the same submit path — the button state and the
        // click handler. If they disagree the button is clickable and the
        // click does nothing, which is exactly the failure being fixed.
        let items = vec![CartItem::new_strain("strain1".to_string(), 1)];
        for (name, phone, address, mode) in [
            ("Дмитрий", "0812345678", "Baan Tai", Fulfillment::Delivery),
            ("Дмитрий", "+66812345678", "", Fulfillment::Pickup),
            ("", "0812345678", "Baan Tai", Fulfillment::Delivery),
            ("Дмитрий", "junk", "Baan Tai", Fulfillment::Delivery),
            ("Дмитрий", "0812345678", "", Fulfillment::Delivery),
        ] {
            let gate_ok =
                checkout_blockers(true, name, phone, address, mode, items.len(), true).is_empty();
            let validate_ok = validate_checkout_for(name, phone, address, &items, mode).is_ok();
            assert_eq!(
                gate_ok, validate_ok,
                "gate and validator disagree for {name:?}/{phone:?}/{address:?}/{mode:?}"
            );
        }
    }

    #[test]
    fn blocker_messages_are_translated_in_both_languages() {
        // A blocker the customer cannot read is no better than a silent one.
        use crate::trios::core::Lang;
        use crate::trios::i18n::t;
        for b in [
            CheckoutBlocker::NoTelegram,
            CheckoutBlocker::EmptyCart,
            CheckoutBlocker::Name,
            CheckoutBlocker::Phone,
            CheckoutBlocker::Address,
            CheckoutBlocker::Age,
        ] {
            for lang in [Lang::Russian, Lang::English] {
                let msg = t(lang, b.message_key());
                assert!(
                    !msg.is_empty() && msg != b.message_key(),
                    "{b:?} has no {lang:?} translation"
                );
            }
        }
    }

    #[test]
    fn test_order_can_cancel() {
        let cart = Cart::new();
        let order = Order::new(
            cart,
            "John Doe".to_string(),
            "+1234567890".to_string(),
            PickupType::Pickup,
        );
        assert!(order.can_cancel());
    }
}
