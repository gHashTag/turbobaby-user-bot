use crate::ui::api::types::Strain;
use crate::ui::components::StrainCard;
use crate::ui::state::{CartItem, CartItemType};
use dioxus::prelude::*;

#[derive(Props, PartialEq, Clone)]
pub struct StrainGridProps {
    strains: Vec<Strain>,
}

#[component]
pub fn StrainGrid(props: StrainGridProps) -> Element {
    let cart = use_context::<Signal<crate::ui::state::Cart>>();

    rsx! {
        div { class: "strain-grid",
            for item in &props.strains {
                StrainCard {
                    strain: item.clone(),
                    on_add_to_cart: {
                        let mut cart = cart;
                        move |s: Strain| {
                            let cart_item = CartItem {
                                id: s.id.clone(),
                                name: s.name.clone(),
                                price: s.price,
                                quantity: 1,
                                image_url: Some(s.image_url.clone()),
                                item_type: CartItemType::Strain,
                            };
                            cart.write().add_item(cart_item);
                            crate::ui::telegram::TelegramApp::init().haptic_notification(crate::ui::telegram::HapticNotification::Success);
                        }
                    },
                }
            }
        }
    }
}
