//! Two-column grid of bike cards.
//!
//! Layout only. Unlike the grid it replaces, this component does not
//! write to the cart itself: a rental line carries dates and a family key
//! (D8), and the date pickers live on the screen, so the screen owns the cart
//! write and the grid only forwards the press.
use crate::ui::components::bike_card::{BikeCard, BikeCardData};
use dioxus::prelude::*;

#[derive(Props, PartialEq, Clone)]
pub struct BikeGridProps {
    bikes: Vec<BikeCardData>,
    /// Family key of the highlighted pick, if the catalog has one.
    #[props(default)]
    featured_id: Option<String>,
    #[props(default)]
    on_add_to_cart: EventHandler<BikeCardData>,
}

#[component]
pub fn BikeGrid(props: BikeGridProps) -> Element {
    let featured_id = props.featured_id.clone();
    let on_add_to_cart = props.on_add_to_cart;

    rsx! {
        div { class: "bike-grid",
            for item in &props.bikes {
                BikeCard {
                    key: "{item.id}",
                    bike: item.clone(),
                    featured: featured_id.as_deref() == Some(item.id.as_str()),
                    on_add_to_cart: move |b: BikeCardData| on_add_to_cart.call(b),
                }
            }
        }
    }
}
