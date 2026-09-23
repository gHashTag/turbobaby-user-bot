// Order detail screen — deep-link destination for order notifications.
//
// Shows the full order (items, totals, delivery info, status stepper) and is
// reachable both from the `/orders/:id` route and from taps on order cards in
// the orders list.

use crate::trios::i18n::{
    t, tf, T_CART_DELIVERY, T_CART_SUBTOTAL, T_MODAL_CANCEL, T_MODAL_CONFIRM, T_ORDERS_ORDER,
    T_ORDERS_TITLE, T_ORDER_DETAIL_BACK, T_ORDER_DETAIL_BONUS, T_ORDER_DETAIL_CANCEL,
    T_ORDER_DETAIL_CANCEL_CONFIRM, T_ORDER_DETAIL_LIVE, T_ORDER_DETAIL_NOT_FOUND,
    T_ORDER_DETAIL_STARS, T_ORDER_DETAIL_TOTAL, T_ORDER_REORDER,
};
use crate::trios::order_status_view::arm_of;
use crate::ui::api::context::api_base_url;
use crate::ui::api::http::{merge_server_cart, post_client_event};
use crate::ui::components::bottom_nav::BottomNav;
use crate::ui::components::skeleton::{Skeleton, SkeletonShape};
use crate::ui::components::StatusStepper;
use crate::ui::routes::Route;
use crate::ui::state::{Cart, CartItem, CartItemType};
use crate::ui::telegram::{
    use_telegram_id, use_telegram_init_data, HapticNotification, TelegramApp,
};
use dioxus::prelude::*;
use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Deserialize)]
struct ApiOrderDetail {
    id: String,
    items: Vec<ApiOrderItem>,
    // The four money figures, each an Option: a payload that omits one reads
    // as an absence and renders as a dash, instead of losing this screen.
    #[serde(flatten)]
    money: crate::trios::pricing::OrderMoney,
    status: String,
    created_at: String,
    shop_id: Option<String>,
    delivery_address: Option<String>,
    delivery_notes: Option<String>,
    customer_phone: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
struct ApiOrderItem {
    strain_id: Option<String>,
    strain_name: Option<String>,
    accessory_id: Option<String>,
    accessory_name: Option<String>,
    tea_id: Option<String>,
    tea_name: Option<String>,
    set_id: Option<String>,
    set_name: Option<String>,
    quantity: f64,
    #[serde(default)]
    unit_price: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct OrderDetailResponse {
    order: ApiOrderDetail,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[allow(dead_code)]
struct OrderStatusResp {
    status: String,
    #[serde(default)]
    delivery_zone_name: Option<String>,
    #[serde(default)]
    min_eta_minutes: Option<u32>,
    #[serde(default)]
    max_eta_minutes: Option<u32>,
}

/// Loop #12: convert a backend order item into a local [`CartItem`] so it can
/// be merged into the server-side cart with current DB prices.
///
/// A line the shop cannot price is dropped, never priced at zero.
///
/// `published()` is the same filter the catalog renders through: `None`, NaN,
/// infinity, negatives and `0.0` all mean "this value was never published"
/// (D9). It used to be `unwrap_or(0.0)` here, which turned a missing price
/// into a confident `฿0` — a number nobody measured, presented as fact, and
/// the exact failure `#7` exists to prevent. The happy path re-prices against
/// the server anyway; the invented zero only ever showed up in the offline
/// fallback cart, which is the worst place for it.
///
/// Dropping is what the caller already does with any line it cannot identify
/// — every call site is a `filter_map`. Bike lines never reach this code at
/// all: `src/api/orders.rs:957` clears `unit_price` on them on purpose (a
/// rental's money lives in its `deal`), and none of the four id branches
/// below matches a bike, so a reorder has never included one.
fn api_order_item_to_cart_item(item: &ApiOrderItem) -> Option<CartItem> {
    let (id, name, item_type, price_hint) = if let Some(ref sid) = item.strain_id {
        (
            sid.clone(),
            // The *field* keeps its name — it is the wire contract for orders
            // already in the database, and renaming it would make historical
            // orders unreadable. The *label* does not: this fallback is what a
            // customer sees when an old item carries an id but no name, and
            // «Strain» is exactly the vocabulary #2 removes. "Товар" says the
            // same thing without naming the previous shop's goods.
            item.strain_name.clone().unwrap_or_else(|| "Товар".into()),
            CartItemType::Strain,
            crate::ui::components::bike_card::published(item.unit_price)?,
        )
    } else if let Some(ref set_id) = item.set_id {
        (
            set_id.clone(),
            item.set_name.clone().unwrap_or_else(|| "Set".into()),
            CartItemType::Set,
            crate::ui::components::bike_card::published(item.unit_price)?,
        )
    } else if let Some(ref aid) = item.accessory_id {
        (
            aid.clone(),
            item.accessory_name
                .clone()
                .unwrap_or_else(|| "Accessory".into()),
            CartItemType::Accessory,
            crate::ui::components::bike_card::published(item.unit_price)?,
        )
    } else {
        let tid = item.tea_id.as_ref()?;
        (
            tid.clone(),
            item.tea_name.clone().unwrap_or_else(|| "Drink".into()),
            CartItemType::Tea,
            crate::ui::components::bike_card::published(item.unit_price)?,
        )
    };
    Some(CartItem {
        id,
        name,
        // `price_hint` came through `published()?` above, so it is a real
        // number by the time it reaches here: the reorder path drops a line it
        // cannot price rather than carrying the absence forward.
        price: Some(price_hint),
        quantity: item.quantity.max(1.0) as u32,
        image_url: None,
        item_type,
        fulfillment: None,
    })
}

fn item_name(item: &ApiOrderItem) -> String {
    item.strain_name
        .clone()
        .or_else(|| item.accessory_name.clone())
        .or_else(|| item.tea_name.clone())
        .or_else(|| item.set_name.clone())
        .unwrap_or_else(|| "Unknown".to_string())
}

/// What one order line costs in total, or a dash when the shop never priced it.
///
/// Goes through the same `published` filter as the catalog (D9): `None`, NaN,
/// infinity, negatives and `0.0` all mean "never published". A bike line is
/// deliberately in that set — `src/api/orders.rs:957` clears `unit_price` on
/// bikes because a rental's money lives in its `deal`, so multiplying a day
/// rate by a unit count here would print a figure the door never quoted.
fn line_total(item: &ApiOrderItem) -> String {
    let total = crate::ui::components::bike_card::published(item.unit_price)
        .map(|p| p * item.quantity.max(0.0));
    crate::ui::components::bike_card::thb_or_dash(total)
}

/// Render the body of an order detail card. Extracted into its own component so
/// the screen-level resource read does not borrow across Dioxus event handlers
/// (which must be `'static`).
#[component]
fn OrderDetailCard(
    order: ApiOrderDetail,
    telegram_id: i64,
    init_data: String,
    live_status_res: Resource<Option<OrderStatusResp>>,
    cart: Signal<Cart>,
    lang: crate::trios::core::Lang,
    on_cancel_answered: EventHandler<()>,
) -> Element {
    let nav = navigator();
    let short_id: String = order
        .id
        .chars()
        .rev()
        .take(6)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    let live_status = live_status_res
        .read()
        .as_ref()
        .and_then(|o| o.as_ref())
        .map(|s| s.status.clone());
    let display_status = live_status.as_deref().unwrap_or(&order.status);
    let arm = arm_of(display_status);
    let status_color = arm.color();
    let status_label = t(lang, arm.label_key());
    let date_str = order
        .created_at
        .split('T')
        .next()
        .unwrap_or(&order.created_at)
        .to_string();
    let shop = order.shop_id.as_deref().unwrap_or("TurboBaby");
    // Every figure through the one order rule; an absent one is the line's dash.
    let dash = crate::ui::components::bike_card::DASH;
    let money = crate::trios::pricing::order_money_text(&order.money, dash);
    let is_terminal = arm.reorder_offered();
    let is_pending = arm.customer_may_cancel();
    // What has been read of the status since the last cancel answer. The screen
    // drops its reading when an answer arrives (`on_cancel_answered`), so
    // "nothing landed" means "not read again yet".
    let since_answer = crate::trios::api_errors::StatusSinceAnswer::from_landing(
        live_status_res.read().as_ref().map(Option::is_some),
        is_pending,
    );
    let order_for_reorder = order.clone();
    let reorder_nav = nav;
    let reorder_cart = cart;
    let init_data_for_reorder = init_data.clone();
    let init_data_for_cancel = init_data.clone();
    let mut show_cancel_confirm = use_signal(|| false);
    let mut cancel_progress = use_signal(|| crate::trios::api_errors::OrderCancelProgress::Idle);
    let reorder_loading = use_signal(|| false);
    let reorder_label = t(lang, T_ORDER_REORDER);

    rsx! {
        div { style: "
            background: #16213e; border: 4px solid {status_color};
            border-radius: 0; padding: 16px;
            box-shadow: 4px 4px 0 #000;
        ",
            div { style: "display: flex; justify-content: space-between; align-items: center; margin-bottom: 12px;",
                span { style: "font-size: 16px; color: #e8e8e8; font-weight: 700;",
                    "{tf(lang, T_ORDERS_ORDER, std::slice::from_ref(&short_id))}"
                }
                span { style: "
                    font-size: 13px; padding: 4px 10px;
                    background: {status_color}22; color: {status_color};
                ", "{status_label}" }
            }

            StatusStepper { status: display_status.to_string(), margin_bottom: 16 }

            if live_status.is_some() {
                div { style: "display: flex; align-items: center; gap: 6px; font-size: 11px; color: #39ff14; margin-bottom: 10px;",
                    span { style: "width: 6px; height: 6px; border-radius: 50%; background: #39ff14; display: inline-block;" }
                    "{t(lang, T_ORDER_DETAIL_LIVE)}"
                }
            }

            div { style: "margin-bottom: 12px;",
                div { style: "font-size: 12px; color: #8b8b9e; margin-bottom: 6px;",
                    "{tf(lang, crate::trios::i18n::T_CART_ITEMS, &[order.items.len().to_string()])}"
                }
                for item in order.items.iter() {
                    div { style: "display: flex; justify-content: space-between; font-size: 14px; margin-bottom: 4px;",
                        span { style: "color: #e8e8e8;", "{item_name(item)} x{item.quantity as i32}" }
                        span { style: "color: #8b8b9e;",
                            // An absent line price renders as a dash, not as an
                            // empty cell. `unwrap_or_default()` here produced
                            // `""`, which reads on screen as "this line is
                            // free" — the same invented-zero D9 forbids, just
                            // spelled with no characters at all.
                            "{line_total(item)}"
                        }
                    }
                }
            }

            div { style: "border-top: 1px dashed #2a2a4a; padding-top: 12px; margin-bottom: 12px;",
                div { style: "display: flex; justify-content: space-between; font-size: 14px; margin-bottom: 4px;",
                    span { style: "color: #8b8b9e;", "{t(lang, T_CART_SUBTOTAL)}" }
                    span { style: "color: #e8e8e8;", "{money.subtotal}" }
                }
                if let Some(bonus) = &money.bonus {
                    div { style: "display: flex; justify-content: space-between; font-size: 14px; margin-bottom: 4px;",
                        span { style: "color: #8b8b9e;", "{t(lang, T_ORDER_DETAIL_BONUS)}" }
                        span { style: "color: #ff4757;", "{bonus}" }
                    }
                }
                if let Some(stars) = &money.stars {
                    div { style: "display: flex; justify-content: space-between; font-size: 14px; margin-bottom: 4px;",
                        span { style: "color: #8b8b9e;", "{t(lang, T_ORDER_DETAIL_STARS)}" }
                        span { style: "color: #ff4757;", "{stars}" }
                    }
                }
                div { style: "display: flex; justify-content: space-between; font-size: 14px; margin-bottom: 4px;",
                    span { style: "color: #8b8b9e;", "{t(lang, T_CART_DELIVERY)}" }
                    span { style: "color: #e8e8e8;", "{shop}" }
                }
                div { style: "display: flex; justify-content: space-between; font-size: 18px; margin-top: 8px; padding-top: 8px; border-top: 1px solid #2a2a4a;",
                    span { style: "font-weight: 800; color: #e8e8e8;", "{t(lang, T_ORDER_DETAIL_TOTAL)}" }
                    span { style: "font-weight: 800; color: #ffe600; text-shadow: 2px 2px 0 #000;", "{money.total}" }
                }
            }

            div { style: "border-top: 1px dashed #2a2a4a; padding-top: 12px; margin-bottom: 12px; font-size: 13px; color: #8b8b9e;",
                if let Some(addr) = &order.delivery_address {
                    div { style: "margin-bottom: 4px;", "📍 {addr.clone()}" }
                }
                if let Some(notes) = &order.delivery_notes {
                    div { style: "margin-bottom: 4px;", "📝 {notes.clone()}" }
                }
                if let Some(phone) = &order.customer_phone {
                    div { style: "margin-bottom: 4px;", "📞 {phone.clone()}" }
                }
                div { style: "margin-top: 4px;", "🕒 {date_str}" }
            }

            // The answer is told ABOVE the pending gate: a refusal's re-read can
            // remove the whole block below, and the sentence must survive it.
            if let Some(line) = crate::trios::api_errors::order_cancel_line(lang, cancel_progress(), since_answer) {
                div { style: "font-size: 13px; margin-bottom: 12px; color: {line.tone.color()};", "{line.text}" }
            }

            if is_pending && crate::trios::api_errors::order_cancel_offer_shown(cancel_progress()) {
                if show_cancel_confirm() {
                    div { style: "border: 3px solid #ff4757; background: #2a0a0a; padding: 12px; margin-bottom: 12px; box-shadow: 2px 2px 0 #000;",
                        div { style: "font-size: 13px; color: #e8e8e8; margin-bottom: 10px;", "{t(lang, T_ORDER_DETAIL_CANCEL_CONFIRM)}" }
                        div { style: "display: flex; gap: 8px;",
                            button {
                                style: "flex: 1; font-size: 13px; font-weight: 700; padding: 10px; background: #ff4757; color: #fff; border: 3px solid #b92b3a; cursor: pointer;",
                                disabled: !crate::trios::api_errors::order_cancel_may_send(cancel_progress(), since_answer),
                                onclick: move |_| {
                                    cancel_progress.set(crate::trios::api_errors::OrderCancelProgress::InFlight);
                                    // AGENTS.md lesson 4: the dialog closes on the server's word, never before it.
                                    spawn(confirm_cancel(
                                        order.id.clone(),
                                        init_data_for_cancel.clone(),
                                        telegram_id,
                                        cancel_progress,
                                        show_cancel_confirm,
                                        on_cancel_answered,
                                    ));
                                },
                                "{t(lang, T_MODAL_CONFIRM)}"
                            }
                            button {
                                style: "flex: 1; font-size: 13px; font-weight: 700; padding: 10px; background: #2a2a4a; color: #e8e8e8; border: 3px solid #1a1a2e; cursor: pointer;",
                                onclick: move |_| { show_cancel_confirm.set(false); },
                                "{t(lang, T_MODAL_CANCEL)}"
                            }
                        }
                    }
                } else {
                    button {
                        style: "font-size:13px;font-weight:700;width:100%;padding:12px;background:#ff4757;color:#fff;border:3px solid #b92b3a;box-shadow:2px 2px 0 #000;cursor:pointer;margin-bottom:12px;",
                        onclick: move |_| { show_cancel_confirm.set(true); },
                        "{t(lang, T_ORDER_DETAIL_CANCEL)}"
                    }
                }
            }

            if is_terminal {
                button {
                    style: "font-size:14px;font-weight:700;width:100%;padding:12px;background:#39ff14;color:#000;border:3px solid #2d9e0f;box-shadow:2px 2px 0 #000;cursor:pointer;",
                    disabled: reorder_loading(),
                    onclick: move |_| {
                        let order_items = order_for_reorder.items.clone();
                        let init = init_data_for_reorder.clone();
                        let tid = telegram_id;
                        let mut cart_sig = reorder_cart;
                        let nav = reorder_nav;
                        let mut loading = reorder_loading;
                        spawn(async move {
                            loading.set(true);
                            let local_items: Vec<CartItem> = order_items
                                .iter()
                                .filter_map(api_order_item_to_cart_item)
                                .collect();
                            match merge_server_cart(&api_base_url(), &init, tid, &local_items,
                            ).await {
                                Ok(fresh_cart) => {
                                    cart_sig.set(fresh_cart);
                                    TelegramApp::init().haptic_notification(HapticNotification::Success);
                                    let _ = post_client_event(&api_base_url(), "reorder_clicked", "order_detail").await;
                                    nav.push(Route::Cart {});
                                }
                                Err(_) => {
                                    // Fallback: load items with the stale price hint rather than leaving
                                    // the user without a reorder path.
                                    cart_sig.write().clear();
                                    for item in local_items {
                                        cart_sig.write().add_item(item);
                                    }
                                    TelegramApp::init().haptic_notification(HapticNotification::Success);
                                    let _ = post_client_event(&api_base_url(), "reorder_clicked", "order_detail").await;
                                    nav.push(Route::Cart {});
                                }
                            }
                            loading.set(false);
                        });
                    },
                    "{reorder_label}"
                }
            }
        }
    }
}

#[component]
pub fn OrderDetailScreen(id: String) -> Element {
    let telegram_id = use_telegram_id().unwrap_or(0);
    let init_data = use_telegram_init_data();
    let init_data_for_detail = init_data.clone();
    let order_id = id.clone();

    let detail_resource = use_resource(move || {
        let init = init_data_for_detail.clone();
        let oid = order_id.clone();
        async move {
            if telegram_id == 0 {
                return Err("No telegram_id".to_string());
            }
            let base = api_base_url();
            let url = format!(
                "{}/api/orders/{}/details?telegram_id={}",
                base, oid, telegram_id
            );
            crate::ui::api::local_client::LocalClient::new()
                .get(&url)
                .header("X-Telegram-Init-Data", init)
                .send()
                .await
                .map_err(|e| e.to_string())?
                .json::<OrderDetailResponse>()
                .await
                .map(|r| r.order)
                .map_err(|e| e.to_string())
        }
    });

    // Cycle #90: live status polling so the detail screen stays current after
    // a Telegram push notification deep-links the user here.
    let mut poll_tick = use_signal(|| 0u32);
    use_effect(move || {
        spawn(async move {
            for tick in 0..360u32 {
                gloo_timers::future::sleep(core::time::Duration::from_millis(10000)).await;
                poll_tick.set(tick + 1);
            }
        });
    });
    let mut live_status_res = {
        let id = id.clone();
        let init = init_data.clone();
        use_resource(move || {
            let id = id.clone();
            let init = init.clone();
            let _ = poll_tick();
            async move {
                if telegram_id == 0 {
                    return None;
                }
                let base = api_base_url();
                let url = format!(
                    "{}/api/orders/{}/status?telegram_id={}",
                    base, id, telegram_id
                );
                crate::ui::api::local_client::LocalClient::new()
                    .get(&url)
                    .header("X-Telegram-Init-Data", init)
                    .send()
                    .await
                    .ok()?
                    .json::<OrderStatusResp>()
                    .await
                    .ok()
            }
        })
    };

    let nav = navigator();
    let lang = crate::ui::lang::current_lang();
    let cart = use_context::<Signal<Cart>>();
    let title = t(lang, T_ORDERS_TITLE);
    let back_label = t(lang, T_ORDER_DETAIL_BACK);

    rsx! {
        div { style: "
            min-height: 100vh;
            background: #0f0f1a;
            color: #e8e8e8;
            padding-bottom: calc(96px + env(safe-area-inset-bottom));
        ",
            div { style: "padding: 16px; display: flex; align-items: center; gap: 12px;",
                button {
                    style: "font-size: 13px; padding: 8px 12px; background: #2a2a4a; color: #e8e8e8; border: 3px solid #1a1a2e; box-shadow: 2px 2px 0 #000; cursor: pointer;",
                    onclick: move |_| { nav.push(Route::Orders {}); },
                    "{back_label}"
                }
                h1 { style: "font-size: 18px; font-weight: 800; color: #39ff14; text-shadow: 2px 2px 0 #000;", "{title}" }
            }

            div { style: "padding: 0 16px;",
                {
                    let detail_opt = detail_resource.read().clone();
                    match detail_opt {
                        Some(Ok(order)) => rsx! {
                            OrderDetailCard {
                                order,
                                telegram_id,
                                init_data: init_data.clone(),
                                live_status_res,
                                cart,
                                lang,
                                // AGENTS.md lesson 5: the card reports an answer and this
                                // screen, which owns the resource, drops the stale reading
                                // and reads the status again -- past the hour's poll too.
                                on_cancel_answered: EventHandler::new(move |_| {
                                    live_status_res.clear();
                                    live_status_res.restart();
                                }),
                            }
                        },
                        Some(Err(e)) => rsx! {
                            div { style: "text-align: center; padding: 40px 16px;",
                                div { style: "font-size: 70px; margin-bottom: 12px;", "⚠️" }
                                p { style: "font-size: 13px; color: #ff4757;", "{t(lang, T_ORDER_DETAIL_NOT_FOUND)}" }
                                p { style: "font-size: 12px; color: #8b8b9e; margin-top: 8px;", "{e}" }
                            }
                        },
                        None => rsx! {
                            div { style: "display:flex;flex-direction:column;gap:12px;",
                                Skeleton { shape: SkeletonShape::Orders }
                                Skeleton { shape: SkeletonShape::Orders }
                            }
                        },
                    }
                }
            }

            BottomNav {}
        }
    }
}

/// Send the customer's cancellation and tell the card what came back.
///
/// Until 2026-09-22 the answer was bound to an unread local and the dialog was
/// closed on every path, so a success, a refusal and a lost connection looked
/// the same. What the answer MEANS -- and what the customer is told of it --
/// is read in `src/trios/api_errors.rs`, where the host can test it; this task
/// only sends, reads the status code, and reports.
///
/// It writes the card's own two signals, never the screen's: the fresh status
/// reading is the screen's to take, through `on_answered` (AGENTS.md lesson 5).
async fn confirm_cancel(
    order_id: String,
    init: String,
    telegram_id: i64,
    mut progress: Signal<crate::trios::api_errors::OrderCancelProgress>,
    mut show_confirm: Signal<bool>,
    on_answered: EventHandler<()>,
) {
    use crate::trios::api_errors::{
        order_cancel_answer, order_cancel_dialog_closes, OrderCancelAnswer, OrderCancelProgress,
    };
    let url = format!(
        "{}/api/orders/{}/cancel?telegram_id={}",
        api_base_url(),
        order_id,
        telegram_id
    );
    let status = crate::ui::api::local_client::LocalClient::new()
        .post(&url)
        .header("X-Telegram-Init-Data", init)
        .send()
        .await
        .ok()
        .map(|response| response.status().as_u16());
    let answer = order_cancel_answer(status);
    if order_cancel_dialog_closes(answer) {
        show_confirm.set(false);
    }
    progress.set(OrderCancelProgress::Answered(answer));
    // Every answer, a success included, is followed by a fresh reading: the last
    // one predates the answer, and after no answer it is the only way to learn
    // whether the cancellation landed before anything is sent again.
    on_answered.call(());
    TelegramApp::init().haptic_notification(if answer == OrderCancelAnswer::Cancelled {
        HapticNotification::Success
    } else {
        HapticNotification::Error
    });
}
