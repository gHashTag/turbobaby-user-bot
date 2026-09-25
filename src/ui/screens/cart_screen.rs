// Cart Screen — Interactive with global Cart signal
use crate::trios::i18n::{
    t, tf, T_BIKE_PRICE_ON_REQUEST, T_CART_BACK_MENU, T_CART_BONUS_NUDGE, T_CART_BROWSE_MENU,
    T_CART_CHECKOUT, T_CART_DECREASE_QTY, T_CART_DELIVERY, T_CART_DELIVERY_FREE, T_CART_DINE_IN,
    T_CART_EMPTY, T_CART_EMPTY_DESC, T_CART_IMAGE_ALT, T_CART_ITEMS, T_CART_LINE_EACH,
    T_CART_REMOVE, T_CART_SUBTOTAL, T_CART_SYNCING, T_CART_TAKEAWAY, T_CART_TITLE, T_PLACE_ORDER,
    T_TOTAL,
};
use crate::ui::api::context::api_base_url;
use crate::ui::api::http::fetch_text_authed;
use crate::ui::api::types::ServerCart;
use crate::ui::components::bottom_nav::BottomNav;
use crate::ui::components::empty_state::EmptyState;
use crate::ui::routes::Route;
use crate::ui::state::{Cart, CartItem, CartItemType};
use crate::ui::telegram::{
    use_main_button_click, use_telegram_id, use_telegram_init_data, HapticNotification, TelegramApp,
};
use dioxus::prelude::*;

/// A money slot on this screen: the number, or a dash when there is none.
///
/// `thb_or_dash` is the UI's one money renderer (D15) and it puts `None`, NaN,
/// infinity, a negative and `0.0` in the same bucket -- all of them mean nobody
/// published this figure (D9). It used to be `format_baht`, which takes an
/// `f64` and has nothing to say about absence: `sanitize_money` turned every
/// one of those into `0`, so a cart the shop could not price displayed as free.
fn format_price(price: Option<f64>) -> String {
    crate::ui::components::bike_card::thb_or_dash(price)
}

#[component]
pub fn CartScreen() -> Element {
    let mut cart = use_context::<Signal<Cart>>();
    let items = cart.read().items.clone();
    let total = cart.read().total;
    let item_count: u32 = items.iter().map(|i| i.quantity).sum();
    let lang = crate::ui::lang::current_lang();
    // D11: the dash never travels alone. A cart with no total says what a bike
    // with no rate says -- a human quotes this price -- because silence is
    // listed beside invention in `must_not_emit`, and a customer staring at a
    // dash with no sentence has been served the silence half.
    let unpriced_note = cart
        .read()
        .has_unpriced_line()
        .then(|| t(lang, T_BIKE_PRICE_ON_REQUEST));
    let cart_title = t(lang, T_CART_TITLE);
    let cart_empty = t(lang, T_CART_EMPTY);
    let cart_empty_desc = t(lang, T_CART_EMPTY_DESC);
    let browse_menu = t(lang, T_CART_BROWSE_MENU);
    let items_label = tf(lang, T_CART_ITEMS, &[item_count.to_string()]);
    let tg = TelegramApp::init();
    let total_str = format_price(total);
    let telegram_id = use_telegram_id();
    let init_data = use_telegram_init_data();

    // Loop #14: surface the customer's bonus balance on the cart screen to
    // nudge them toward checkout where it can be redeemed.
    let loyalty_res = {
        let init = init_data.clone();
        use_resource(move || {
            let init = init.clone();
            async move {
                let tid = telegram_id?;
                let url = format!("{}/api/loyalty/{}", api_base_url(), tid);
                let text = fetch_text_authed(&url, &init).await.ok()?;
                #[derive(serde::Deserialize)]
                struct ProfileResp {
                    bonus_balance: f64,
                }
                #[derive(serde::Deserialize)]
                struct LoyaltyResp {
                    profile: ProfileResp,
                }
                serde_json::from_str::<LoyaltyResp>(&text).ok()
            }
        })
    };
    // Loop #15: show a server-cart sync indicator so the user knows when the
    // cart is being reconciled with the server. If the local cart is empty and
    // the server has items, recover them here (the startup path in app.rs also
    // does this, but a direct deep-link to /cart may arrive after that effect).
    let server_cart_res: Resource<Result<ServerCart, String>> = use_resource(move || {
        let init = init_data.clone();
        async move {
            let tid = telegram_id.ok_or_else(|| {
                crate::trios::api_errors::friendly_response_error(
                    crate::ui::lang::current_lang(),
                    0,
                )
            })?;
            let url = format!("{}/api/cart?telegram_id={}", api_base_url(), tid);
            let text = fetch_text_authed(&url, &init).await.map_err(|_| {
                crate::trios::api_errors::friendly_response_error(
                    crate::ui::lang::current_lang(),
                    0,
                )
            })?;
            serde_json::from_str::<ServerCart>(&text).map_err(|_| {
                crate::trios::api_errors::friendly_response_error(
                    crate::ui::lang::current_lang(),
                    0,
                )
            })
        }
    });
    use_effect(move || {
        let maybe = server_cart_res.read().as_ref().cloned();
        if let Some(Ok(server_cart)) = maybe {
            if cart.read().items.is_empty() && !server_cart.items.is_empty() {
                let mut recovered = Cart::new();
                for item in server_cart.items {
                    if let Some(ci) = CartItem::from_server(item) {
                        recovered.add_item(ci);
                    }
                }
                cart.set(recovered);
            }
        }
    });
    let is_syncing = server_cart_res.read().is_none();

    let bonus_balance = loyalty_res
        .read()
        .as_ref()
        .and_then(|opt| opt.as_ref())
        .map(|r| r.profile.bonus_balance)
        .unwrap_or(0.0)
        .max(0.0);
    let bonus_nudge_text = if bonus_balance > 0.0 {
        Some(tf(
            lang,
            T_CART_BONUS_NUDGE,
            &[crate::trios::pricing::format_baht(bonus_balance)],
        ))
    } else {
        None
    };

    let nav = navigator();
    use_main_button_click(move || {
        // Same gate as the in-app button below: the native MainButton is a
        // second door into checkout, and closing only one of them would let a
        // cart the shop cannot price through the other.
        if !cart.read().items.is_empty() && !cart.read().has_unpriced_line() {
            nav.push(Route::Checkout {});
        }
    });
    if !items.is_empty() && unpriced_note.is_none() {
        tg.set_main_button_text(&format!("{} — {}", t(lang, T_PLACE_ORDER), total_str));
        tg.enable_main_button();
        tg.show_back_button();
    } else if !items.is_empty() {
        // The cart is not empty, it is unquotable: the back button stays, the
        // primary action does not, and the summary above says why.
        tg.hide_main_button();
        tg.show_back_button();
    } else {
        tg.hide_main_button();
        tg.hide_back_button();
    }
    // MainButton is now wired through a DOM CustomEvent bridge. The in-app
    // checkout button remains as a visible, accessible fallback. Telegram
    // MainButton acts as a native primary action and back navigation affordance.

    rsx! {
        div { style: "
            min-height: 100vh;
            background: #0f0f1a;
            color: #e8e8e8;
            padding-bottom: calc(96px + env(safe-area-inset-bottom));
        ",
            // Header
            div { style: "padding: 20px 16px 16px; text-align: center;",
                h1 { style: "font-size: 24px; font-weight: 800; color: #39ff14; text-shadow: 3px 3px 0 #000, 0 0 10px rgba(57,255,20,0.5); letter-spacing: 2px;", "{cart_title}" }
                if !items.is_empty() {
                    p { style: "font-size: 15px; color: #8b8b9e; margin-top: 4px;", "{items_label}" }
                }
                if is_syncing {
                    div { style: "margin-top: 10px; font-size: 13px; color: #8b8b9e; display: flex; align-items: center; justify-content: center; gap: 6px;",
                        span { "🔄" }
                        "{t(lang, T_CART_SYNCING)}"
                    }
                }
            }

            // Cart content
            div { style: "padding: 0 16px;",
                {
                    if items.is_empty() {
                        rsx! {
                            div { style: "padding: 40px 0;",
                                EmptyState {
                                    icon: "🛒".to_string(),
                                    title: cart_empty.to_string(),
                                    description: cart_empty_desc.to_string(),
                                    glass: true,
                                    action: rsx! {
                                        Link { to: Route::Menu {},
                                            button { style: "
                                                font-size: 15px; font-weight: 700; padding: 14px 20px;
                                                background: #39ff14; color: #000;
                                                border: 4px solid #2d9e0f; border-radius: 0;
                                                cursor: pointer; box-shadow: 3px 3px 0 #000;
                                                width: 100%; max-width: 320px;
                                            ", "{browse_menu} →" }
                                        }
                                    },
                                }
                                // A second button offering «В наборы» stood here,
                                // linking to `Route::Sets` — a surface migration
                                // 085 unpublishes (`sets`, `accessory_sets`).
                                // The empty cart is the *only* state this screen
                                // can be in (`api/cart.rs::parse_kind` accepts
                                // strain/set/accessory/tea, and 083 drops
                                // `strains` while 085 unpublishes the rest), so
                                // this was the whole screen for every renter who
                                // opened the tab.
                            }
                        }
                    } else {
                        rsx! {
                            {
                                items.into_iter().map(|item| {
                                    cart_item_row(item, lang)
                                })
                            }

                            // Summary
                            div { style: "
                                background: #16213e; border: 4px solid #2a2a4a;
                                border-radius: 0; padding: 14px;
                                box-shadow: 4px 4px 0 #000; margin-top: 8px;
                            ",
                                div { style: "display: flex; justify-content: space-between; margin-bottom: 6px; font-size: 15px;",
                                    span { style: "color: #8b8b9e;", "{t(lang, T_CART_SUBTOTAL)}" }
                                    span { "{format_price(total)}" }
                                }
                                div { style: "display: flex; justify-content: space-between; margin-bottom: 6px; font-size: 15px;",
                                    span { style: "color: #8b8b9e;", "{t(lang, T_CART_DELIVERY)}" }
                                    span { style: "color: #39ff14;", "{t(lang, T_CART_DELIVERY_FREE)}" }
                                }
                                if let Some(ref nudge) = bonus_nudge_text {
                                    div { style: "display: flex; justify-content: space-between; font-size: 13px; color: #39ff14; margin-bottom: 6px;",
                                        span { "{nudge}" }
                                    }
                                }
                                div { style: "display: flex; justify-content: space-between; font-size: 15px; font-weight: 800; padding-top: 6px; border-top: 1px solid #2a2a4a; margin-top: 6px;",
                                    span { "{t(lang, T_TOTAL)}" }
                                    span { style: "font-size: 20px; font-weight: 800; color: #ffe600; text-shadow: 2px 2px 0 #000;", "{format_price(total)}" }
                                }
                                if let Some(ref note) = unpriced_note {
                                    div { style: "font-size: 13px; color: #888; font-style: italic; margin-top: 6px;",
                                        span { "{note}" }
                                    }
                                }
                            }

                            // Actions
                            div { style: "display: flex; gap: 10px; margin-top: 16px;",
                                Link { to: Route::Menu {},
                                    button { style: "
                                        font-size: 14px; font-weight: 700; flex: 1; padding: 12px 20px;
                                        background: transparent; color: #e8e8e8;
                                        border: 4px solid #2a2a4a; border-radius: 0;
                                        cursor: pointer; box-shadow: 3px 3px 0 #000;
                                        transition: transform 0.1s, box-shadow 0.1s;
                                    ", "{t(lang, T_CART_BACK_MENU)}" }
                                }
                                // Checkout is unreachable while a line has no
                                // price: the order body would carry a subtotal
                                // and a total computed from the priced lines
                                // alone, and the customer would be quoted a
                                // number nobody measured. The checkout screen
                                // refuses the same cart on submit -- this is the
                                // earlier of the two refusals, not the only one.
                                if unpriced_note.is_some() {
                                    button {
                                        disabled: true,
                                        style: "
                                            font-size: 14px; font-weight: 700; flex: 2; padding: 12px 20px;
                                            background: #2a2a4a; color: #8b8b9e;
                                            border: 4px solid #2a2a4a; border-radius: 0;
                                            cursor: not-allowed; box-shadow: 3px 3px 0 #000;
                                        ",
                                        "{t(lang, T_CART_CHECKOUT)}"
                                    }
                                } else {
                                    Link { to: Route::Checkout {},
                                        button { style: "
                                            font-size: 14px; font-weight: 700; flex: 2; padding: 12px 20px;
                                            background: #39ff14; color: #000;
                                            border: 4px solid #2d9e0f; border-radius: 0;
                                            cursor: pointer; box-shadow: 3px 3px 0 #000;
                                            transition: transform 0.1s, box-shadow 0.1s;
                                        ", "{t(lang, T_CART_CHECKOUT)}" }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            BottomNav {}
        }
    }
}

fn cart_item_row(item: CartItem, lang: crate::trios::core::Lang) -> Element {
    let mut cart = use_context::<Signal<Cart>>();
    let item_id_for_minus = item.id.clone();
    let item_id_for_plus = item.id.clone();
    let item_id_for_remove = item.id.clone();
    let price_str = format_price(item.price);
    let qty = item.quantity;
    // `cart_line_total` is the shared rule (D15): an absent, zero, negative or
    // non-finite unit price has no line total either, so the line shows a dash
    // in both slots rather than a price of `-` beside a total of `0`.
    let line_total_str = format_price(crate::trios::pricing::cart_line_total(item.price, qty));
    let row_key = item.id.clone();
    // Owner, 2026-09-25, answer 3: a line of the old catalogue prints the
    // neutral name, never the one it was stored with, and not the picture it
    // was stored with either (on this device too): the server's rule for both.
    let kind = crate::ui::api::http::cart_item_type_to_kind(&item.item_type);
    let shown_name = crate::trios::legacy_view::shown_cart_line_name(lang, kind, &item.name);
    let shown_image = crate::trios::legacy_view::cart_line_image(kind, &item.image_url);
    let image_alt = tf(lang, T_CART_IMAGE_ALT, std::slice::from_ref(&shown_name));

    rsx! {
        div {
            key: "{row_key}",
            // `flex-wrap` matters: three 44px controls plus a count need ~162px,
            // and a long product name needs the rest. On a narrow phone they do
            // not both fit, and without wrapping the row simply overflowed the
            // card — the delete button was clipped off the right edge.
            style: "
            display: flex; align-items: center; flex-wrap: wrap; gap: 10px;
            background: #16213e; border: 4px solid #2a2a4a;
            border-radius: 0; padding: 10px; margin-bottom: 8px;
            box-shadow: 4px 4px 0 #000;
        ",
            if let Some(ref url) = shown_image {
                img {
                    src: "{url}",
                    alt: "{image_alt}",
                    style: "width:44px;height:44px;object-fit:cover;background:#0f0f1a;border:2px solid #2a2a4a;flex-shrink:0;"
                }
            } else {
                div { style: "
                    width: 44px; height: 44px; background: #16213e;
                    border-radius: 0; display: flex; align-items: center;
                    justify-content: center; font-size: 22px; flex-shrink: 0;
                ", "🏍" }
            }
            // `min-width: 0` is the whole fix for the text column: a flex item
            // defaults to `min-width: auto`, so it refuses to shrink below its
            // own content and a long name pushes everything else out of the
            // card. The basis keeps it from collapsing to nothing once the row
            // is allowed to wrap.
            div { style: "flex: 1 1 150px; min-width: 0;",
                div { style: "font-size: 16px; font-weight: 700; margin-bottom: 2px; overflow-wrap: anywhere;", "{shown_name}" }
                div { style: "font-size: 14px; color: #8b8b9e;",
                    {tf(lang, T_CART_LINE_EACH, &[price_str.clone(), line_total_str.clone()])}
                }
                // A3: per-drink dine-in / takeaway toggle (drinks only).
                if item.item_type == CartItemType::Tea {
                    {
                        let cur = item.fulfillment.clone().unwrap_or_else(|| "takeaway".to_string());
                        let din_active = cur == "dine_in";
                        let id_din = item.id.clone();
                        let id_take = item.id.clone();
                        let din_style = format!(
                            "font-size:12px;padding:4px 8px;border:3px solid {};background:{};color:{};border-radius:0;cursor:pointer;white-space:nowrap;",
                            if din_active { "#39ff14" } else { "#2a2a4a" },
                            if din_active { "#39ff14" } else { "transparent" },
                            if din_active { "#000" } else { "#8b8b9e" },
                        );
                        let take_style = format!(
                            "font-size:12px;padding:4px 8px;border:3px solid {};background:{};color:{};border-radius:0;cursor:pointer;white-space:nowrap;",
                            if !din_active { "#39ff14" } else { "#2a2a4a" },
                            if !din_active { "#39ff14" } else { "transparent" },
                            if !din_active { "#000" } else { "#8b8b9e" },
                        );
                        rsx! {
                            div { style: "display:flex;gap:6px;margin-top:6px;",
                                button {
                                    style: "{din_style}",
                                    onclick: move |_| {
                                        let mut c = cart.write();
                                        if let Some(it) = c.items.iter_mut().find(|i| i.id == id_din) {
                                            it.fulfillment = Some("dine_in".to_string());
                                        }
                                    },
                                    "{t(lang, T_CART_DINE_IN)}"
                                }
                                button {
                                    style: "{take_style}",
                                    onclick: move |_| {
                                        let mut c = cart.write();
                                        if let Some(it) = c.items.iter_mut().find(|i| i.id == id_take) {
                                            it.fulfillment = Some("takeaway".to_string());
                                        }
                                    },
                                    "{t(lang, T_CART_TAKEAWAY)}"
                                }
                            }
                        }
                    }
                }
            }
            // `flex: 0 0 auto` so the controls keep their own width instead of
            // being squeezed; `margin-left: auto` keeps them right-aligned both
            // on one line and when they wrap onto their own.
            div { style: "display: flex; align-items: center; gap: 6px; flex: 0 0 auto; margin-left: auto;",
                button {
                    style: "
                        width: 44px; height: 44px;
                        border: 4px solid #2a2a4a; background: #16213e;
                        color: #e8e8e8; border-radius: 0; cursor: pointer;
                        font-size: 16px; display: flex; align-items: center; justify-content: center;
                        box-shadow: 3px 3px 0 #000;
                        transition: transform 0.1s, box-shadow 0.1s;
                    ",
                    "aria-label": "{t(lang, T_CART_DECREASE_QTY)}",
                    onclick: move |_| {
                        let mut c = cart.write();
                        if let Some(item) = c.items.iter_mut().find(|i| i.id == item_id_for_minus) {
                            if item.quantity > 1 {
                                item.quantity -= 1;
                            } else {
                                let id = item_id_for_minus.clone();
                                drop(c);
                                cart.write().remove_item(&id);
                                return;
                            }
                        }
                        c.recalculate_total();
                    },
                    "−"
                }
                span { style: "font-size: 15px; min-width: 18px; text-align: center;", "{qty}" }
                button {
                    style: "
                        width: 44px; height: 44px;
                        border: 4px solid #2a2a4a; background: #16213e;
                        color: #e8e8e8; border-radius: 0; cursor: pointer;
                        font-size: 16px; display: flex; align-items: center; justify-content: center;
                        box-shadow: 3px 3px 0 #000;
                        transition: transform 0.1s, box-shadow 0.1s;
                    ",
                    onclick: move |_| {
                        cart.write().add_item(CartItem {
                            id: item_id_for_plus.clone(),
                            name: item.name.clone(),
                            price: item.price,
                            quantity: 1,
                            image_url: None,
                            item_type: item.item_type.clone(),
                            fulfillment: None,
                        });
                        crate::ui::telegram::TelegramApp::init().haptic_notification(crate::ui::telegram::HapticNotification::Success);
                    },
                    "+"
                }
                button {
                    style: "
                        width: 44px; height: 44px;
                        border: 4px solid #ff4757; background: rgba(255,71,87,0.1);
                        color: #ff4757; border-radius: 0; cursor: pointer;
                        font-size: 16px; display: flex; align-items: center; justify-content: center;
                        box-shadow: 3px 3px 0 #000;
                        transition: transform 0.1s, box-shadow 0.1s;
                    ",
                    "aria-label": "{t(lang, T_CART_REMOVE)}",
                    onclick: move |_| {
                        let id = item_id_for_remove.clone();
                        cart.write().remove_item(&id);
                        TelegramApp::init().haptic_notification(HapticNotification::Warning);
                    },
                    "🗑"
                }
            }
        }
    }
}
