use crate::trios::i18n::{t, T_BIKE_PRICE_ON_REQUEST};
use crate::ui::components::{Button, ButtonVariant};
use crate::ui::state::CartItem;
use dioxus::prelude::*;

#[derive(Props, PartialEq, Clone)]
pub struct CartItemProps {
    item: CartItem,
}

#[component]
pub fn CartItemComponent(props: CartItemProps) -> Element {
    let item = props.item.clone();
    let lang = crate::ui::lang::current_lang();

    // Owner, 2026-09-25, answer 3: a device cart line of the old catalogue
    // shows the neutral name and no stored picture. Nothing mounts this
    // component today; it reads the line the way the cart screen does, so
    // mounting it cannot bring the stored name or picture back.
    let kind = crate::ui::api::http::cart_item_type_to_kind(&item.item_type);
    let shown_name = crate::trios::legacy_view::shown_cart_line_name(lang, kind, &item.name);
    let shown_image = crate::trios::legacy_view::cart_line_image(kind, &item.image_url);
    let img_url = shown_image.as_deref().unwrap_or("");
    let has_image = !img_url.is_empty()
        && (img_url.starts_with("http://")
            || img_url.starts_with("https://")
            || (img_url.starts_with("/") && !img_url.starts_with("//")));
    // D9/D11: a line the shop cannot price shows a dash and the sentence that
    // names a human, never a baht sign followed by a zero. `thb_or_dash` is the UI's one money
    // renderer, and the dash never travels alone -- silence is listed beside
    // invention in D11's `must_not_emit`, so emitting the dash without the
    // sentence commits the other half of the same offence.
    let price_str = crate::ui::components::bike_card::thb_or_dash(item.price);
    let price_note = (!item.is_priced()).then(|| t(lang, T_BIKE_PRICE_ON_REQUEST));

    rsx! {
        div { class: "cart-item",
            if has_image {
                img {
                    class: "cart-item-image",
                    src: "{img_url}?v=2",
                    alt: "{shown_name}"
                }
            }
            div { class: "cart-item-details",
                h4 { class: "cart-item-name", "{shown_name}" }
                p { class: "cart-item-price", "{price_str}" }
                if let Some(note) = price_note {
                    // Styled by `.cart-item-price-note` in styles/main.css, not
                    // inline: the class had no rule behind it and
                    // `every_jsx_class_literal_has_a_css_rule` (src/main.rs:1736)
                    // failed on it. One definition, in the sheet that already
                    // owns `.cart-item`.
                    p { class: "cart-item-price-note", "{note}" }
                }
                div { class: "cart-item-controls",
                    Button {
                        variant: ButtonVariant::Secondary,
                        size: crate::ui::components::ButtonSize::Small,
                        "-",
                    }
                    span { class: "cart-item-quantity", "{item.quantity}" }
                    Button {
                        variant: ButtonVariant::Secondary,
                        size: crate::ui::components::ButtonSize::Small,
                        "+",
                    }
                }
            }
            Button {
                variant: ButtonVariant::Danger,
                size: crate::ui::components::ButtonSize::Small,
                class: "cart-item-remove",
                "🗑️"
            }
        }
    }
}
