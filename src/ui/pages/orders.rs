use dioxus::prelude::*;
use crate::ui::api::{client::ApiClient, types::Order};
use crate::ui::components::Loading;
use crate::ui::telegram::{use_telegram_id, use_telegram_init_data};

#[component]
pub fn Orders() -> Element {
    let telegram_id = use_telegram_id().unwrap_or(123456i64);
    let init_data = use_telegram_init_data();
    let orders = use_resource(move || {
        let init = init_data.clone();
        async move {
            ApiClient::new(String::new(), init).get_user_orders(telegram_id).await
        }
    });

    rsx! {
        div { class: "page orders-page",
            div { class: "page-header",
                h1 { class: "page-title", "Orders" }
                p { class: "page-description", "Your order history" }
            }
            match &*orders.read() {
                Some(Ok(order_list)) => {
                    if order_list.is_empty() {
                        rsx! { EmptyOrdersView {} }
                    } else {
                        rsx! { OrdersListView { orders: order_list.clone() } }
                    }
                },
                Some(Err(_)) => rsx! { ErrorOrdersView {} },
                None => rsx! { Loading {} }
            }
        }
    }
}

#[component]
fn EmptyOrdersView() -> Element {
    rsx! {
        div { class: "empty-state",
            span { "No orders yet" }
            button {
                class: "btn btn-primary",
                onclick: move |_| crate::ui::state::set_route("/menu"),
                "Browse Menu"
            }
        }
    }
}

#[component]
fn ErrorOrdersView() -> Element {
    rsx! {
        div { class: "error-state",
            span { "Failed to load orders" }
            button {
                class: "btn btn-secondary",
                onclick: move |_| {},
                "Try Again"
            }
        }
    }
}

#[component]
fn OrdersListView(orders: Vec<Order>) -> Element {
    rsx! {
        div { class: "orders-list",
            {orders.into_iter().map(|order| {
                rsx!(OrderCard { key: "{order.id}", order: order.clone() })
            })}
        }
    }
}

#[component]
fn OrderCard(order: Order) -> Element {
    let status = format!("{:?}", order.status);
    let oid = if order.id.len() > 8 {
        format!("...{}", &order.id[order.id.len()-8..])
    } else {
        order.id.clone()
    };
    let date = order.created_at.split('T').next().unwrap_or(&order.created_at).to_string();
    let total = format!("{:.0}", order.total);

    rsx! {
        div { class: "order-card",
            div { class: "order-header",
                span { class: "order-id", "Order #{oid}" }
                span { class: "order-status", "{status}" }
            }
            div { class: "order-body",
                div { class: "order-items",
                    {order.items.iter().map(|item| {
                        let item_name = item.name.clone();
                        let item_qty = item.quantity;
                        rsx! {
                            div { class: "order-item",
                                span { class: "item-name", "{item_name}" }
                                span { class: "item-qty", "x{item_qty}" }
                            }
                        }
                    })}
                }
            }
            div { class: "order-footer",
                span { class: "order-date", "{date}" }
                span { class: "order-total", "{total} THB" }
            }
        }
    }
}
