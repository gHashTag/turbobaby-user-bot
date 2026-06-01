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

/// Validate checkout data
pub fn validate_checkout(name: &str, phone: &str, items: &[CartItem]) -> Result<()> {
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
    if items.is_empty() {
        return Err(Error::Validation("Cart cannot be empty".to_string()));
    }
    // Simple phone validation - at least 5 digits
    let digits: String = phone.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.len() < 5 {
        return Err(Error::Validation("Invalid phone number".to_string()));
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

    #[test]
    fn test_validate_checkout_valid() {
        let items = vec![CartItem::new_strain("strain1".to_string(), 1)];
        assert!(validate_checkout("John", "12345", &items).is_ok());
    }

    #[test]
    fn test_validate_checkout_empty_name() {
        let items = vec![CartItem::new_strain("strain1".to_string(), 1)];
        assert!(validate_checkout("", "12345", &items).is_err());
    }

    #[test]
    fn test_validate_checkout_empty_phone() {
        let items = vec![CartItem::new_strain("strain1".to_string(), 1)];
        assert!(validate_checkout("John", "", &items).is_err());
    }

    #[test]
    fn test_validate_checkout_empty_cart() {
        assert!(validate_checkout("John", "12345", &[]).is_err());
    }

    #[test]
    fn test_validate_checkout_invalid_phone() {
        let items = vec![CartItem::new_strain("strain1".to_string(), 1)];
        assert!(validate_checkout("John", "123", &items).is_err());
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
