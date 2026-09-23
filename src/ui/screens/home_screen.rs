use crate::trios::i18n::{
    t, tf, T_HOME_REORDER_CTA, T_HOME_REORDER_LAST, T_HOME_REORDER_STATUS, T_HOME_SUBTITLE,
    T_TRUST_AGE, T_TRUST_GACP, T_TRUST_MEDICAL, T_TRUST_SUPPORT,
};
use crate::ui::api::context::api_base_url;
use crate::ui::api::http::{fetch_text_authed, merge_server_cart, post_client_event};
use crate::ui::assets;
use crate::ui::components::bottom_nav::BottomNav;
use crate::ui::routes::Route;
use crate::ui::share::{PendingOrder, PendingReorder, SharedProduct};
use crate::ui::state::{Cart, CartItem, CartItemType};
use crate::ui::telegram::{use_telegram_id, use_telegram_init_data};
use dioxus::prelude::*;
use serde::Deserialize;

/// Loop #15: lightweight order-detail DTO used by the proactive reorder deep-link.
#[derive(Debug, Deserialize)]
struct ReorderOrderDetailResponse {
    order: ReorderOrder,
}

#[derive(Debug, Deserialize)]
struct ReorderOrder {
    items: Vec<ReorderOrderItem>,
}

#[derive(Debug, Deserialize)]
struct ReorderOrderItem {
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
fn reorder_item_to_cart_item(item: &ReorderOrderItem) -> Option<CartItem> {
    let (id, name, item_type, price_hint) = if let Some(ref sid) = item.strain_id {
        (
            sid.clone(),
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

/// Loop #16: compact user order for the home reorder widget.
#[derive(Debug, Clone, Deserialize)]
struct HomeOrder {
    id: String,
    status: String,
    #[serde(default)]
    total: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct HomeOrdersResponse {
    orders: Vec<HomeOrder>,
}

async fn fetch_home_last_order(
    telegram_id: i64,
    init_data: &str,
) -> Result<Option<HomeOrder>, String> {
    let base = api_base_url();
    let url = format!("{}/api/orders/user/{}", base, telegram_id);
    let (status, body) = crate::ui::api::http::fetch_text_authed_full(&url, init_data)
        .await
        .map_err(|e| format!("fetch orders: {e}"))?;
    if !(200..300).contains(&status) {
        return Err(format!("orders HTTP {status}"));
    }
    let resp = serde_json::from_str::<HomeOrdersResponse>(&body)
        .map_err(|e| format!("orders parse: {e}"))?;
    Ok(resp.orders.into_iter().next())
}

fn status_color(status: &str) -> &'static str {
    match status.to_lowercase().as_str() {
        "pending" => "#ffe600",
        "confirmed" => "#00e5ff",
        "preparing" => "#ff9d00",
        "ready" => "#39ff14",
        "out_for_delivery" => "#00e5ff",
        "completed" | "delivered" => "#39ff14",
        "cancelled" | "rejected" => "#ff4757",
        _ => "#8b8b9e",
    }
}

fn status_label_key(status: &str) -> crate::trios::i18n::Key {
    match status.to_lowercase().as_str() {
        "pending" => crate::trios::i18n::T_ORDERS_STATUS_PENDING,
        "confirmed" => crate::trios::i18n::T_ORDERS_STATUS_CONFIRMED,
        "preparing" => crate::trios::i18n::T_ORDERS_STATUS_PREPARING,
        "ready" => crate::trios::i18n::T_ORDERS_STATUS_READY,
        "out_for_delivery" => crate::trios::i18n::T_ORDERS_STATUS_OUT_FOR_DELIVERY,
        "completed" | "delivered" => crate::trios::i18n::T_ORDERS_STATUS_DELIVERED,
        "cancelled" | "rejected" => crate::trios::i18n::T_ORDERS_STATUS_CANCELLED,
        _ => crate::trios::i18n::T_ORDERS_STATUS_UNKNOWN,
    }
}

#[component]
pub fn HomeScreen() -> Element {
    let cart = use_context::<Signal<Cart>>();
    let cart_count: u32 = cart.read().items.iter().map(|i| i.quantity).sum();

    // Deep-link landing: when the app opens with a shared product, navigate
    // from the home route to the catalog screen that owns the product.
    let pending = use_context::<Signal<Option<SharedProduct>>>();
    let mut pending_order = use_context::<Signal<PendingOrder>>();
    let mut pending_cart = use_context::<Signal<bool>>();
    let mut pending_reorder = use_context::<Signal<PendingReorder>>();
    let nav = navigator();
    let telegram_id = use_telegram_id();
    let init_data = use_telegram_init_data();
    let cart_for_reorder = use_context::<Signal<Cart>>();

    use_effect(move || {
        if let Some(target) = pending.read().clone() {
            // Navigate to the catalog screen that owns the shared product.
            // The target screen will open the product modal and clear the target.
            nav.push(target.kind.route());
        }
        let order_id = pending_order.read().0.clone();
        if let Some(order_id) = order_id {
            // Cycle #80: order deep link opens the dedicated detail screen.
            pending_order.set(PendingOrder(None));
            nav.push(Route::OrderDetail { id: order_id });
        }
        // Loop #12: cart deep link sends the user straight to the cart.
        if pending_cart() {
            pending_cart.set(false);
            nav.push(Route::Cart {});
        }
        // Two `startapp=garden` branches stood here — one for the bare
        // `garden` link the watering reminder sent, one for an invite carrying
        // a referrer. Neither has anywhere to go: the garden is removed (D5),
        // and the payloads no longer parse, so nothing sets a flag to consume.
        // An old reminder link now gets the bot's ordinary welcome and the
        // customer lands on the fleet, which is the whole shop.
        // Loop #15: proactive reorder deep-link loads the order items into the
        // server-side cart with current DB prices and lands on /cart.
        let maybe_reorder = pending_reorder.read().0.clone();
        if let Some(order_id) = maybe_reorder {
            pending_reorder.set(PendingReorder(None));
            let tid = telegram_id.unwrap_or(0);
            let init = init_data.clone();
            let mut cart_sig = cart_for_reorder;
            let nav = nav;
            spawn(async move {
                if tid == 0 {
                    return;
                }
                let url = format!(
                    "{}/api/orders/{}/details?telegram_id={}",
                    api_base_url(),
                    order_id,
                    tid
                );
                if let Ok(text) = fetch_text_authed(&url, &init).await {
                    if let Ok(resp) = serde_json::from_str::<ReorderOrderDetailResponse>(&text) {
                        let items: Vec<CartItem> = resp
                            .order
                            .items
                            .iter()
                            .filter_map(reorder_item_to_cart_item)
                            .collect();
                        match merge_server_cart(&api_base_url(), &init, tid, &items).await {
                            Ok(fresh_cart) => {
                                cart_sig.set(fresh_cart);
                            }
                            Err(_) => {
                                let mut local = Cart::new();
                                for item in items {
                                    local.add_item(item);
                                }
                                cart_sig.set(local);
                            }
                        }
                        nav.push(Route::Cart {});
                    }
                }
            });
        }
    });

    let home_subtitle = t(crate::ui::lang::current_lang(), T_HOME_SUBTITLE).to_string();
    // Подписи плиток каталога убраны вместе с сеткой CATEGORIES: все шесть вели
    // на снятые с витрины экраны (см. комментарий ниже на месте самой сетки).
    // `add_to_cart_label` ушла следом за карточкой «сорта дня»: на главной
    // больше нет ни одной кнопки, кладущей товар в корзину напрямую.

    // Secret admin entry: 5 rapid taps on the logo navigates to /admin.
    // Easier than remembering /admin URL or relying on bot command menu.
    let mut logo_taps = use_signal(|| 0u32);
    let mut last_tap = use_signal(|| 0u64);
    let nav_for_logo = nav;
    let logo_onclick = move |_| {
        let now = js_sys::Date::now() as u64;
        let dt = now.saturating_sub(last_tap());
        // Reset counter if taps pause for more than 800 ms.
        let count = if dt > 800 { 1 } else { logo_taps() + 1 };
        logo_taps.set(count);
        last_tap.set(now);
        if count >= 5 {
            logo_taps.set(0);
            nav_for_logo.push(Route::Admin {});
        }
    };

    let telegram_id = use_telegram_id();
    let init_data = use_telegram_init_data();
    // Loop #16: last-order widget resource.
    let init_data_for_orders = init_data.clone();
    let orders_resource = use_resource(move || {
        let tid = telegram_id;
        let init = init_data_for_orders.clone();
        async move {
            match tid {
                Some(id) if id > 0 => fetch_home_last_order(id, &init).await.ok().flatten(),
                _ => None,
            }
        }
    });

    rsx! {
        div { style: "min-height:100vh;background:#0f0f1a;color:#e8e8e8;padding-bottom:calc(96px + env(safe-area-inset-bottom));",

            div { style: "text-align:center;padding:20px 16px 16px;position:relative;z-index:1;",
                img {
                    src: "{assets::logo::MAIN}",
                    alt: "TurboBaby",
                    style: "height:120px;width:auto;display:block;margin:0 auto;box-shadow:0 0 20px rgba(0,0,0,0.5);cursor:pointer;user-select:none;transition:transform 0.1s;touch-action:manipulation;",
                    onclick: logo_onclick,
                }
                h1 { style: "font-size:24px;font-weight:800;color:#f5f5f7;text-shadow:2px 2px 0 #000,0 0 10px rgba(0,0,0,0.6);letter-spacing:2px;margin-top:12px;",
                    "TURBOBABY"
                }
                p { style: "font-size:13px;color:#888;margin-top:6px;", "{home_subtitle}" }
            }

            // Виджет сада убран вместе с самим садом (D5): маршрут, экран и
            // таблицы `garden_plants`, `garden_rewards`, `garden_config`
            // удалены. D19 прячет наследие Woody — он не требует оставлять
            // ссылку на экран, которого нет.

            // Loop #16: last-order widget — one-tap reorder surface.
            {
                match orders_resource.read_unchecked().clone() {
                    Some(Some(order)) => {
                        let lang = crate::ui::lang::current_lang();
                        let short_id: String = order
                            .id
                            .chars()
                            .rev()
                            .take(6)
                            .collect::<Vec<_>>()
                            .into_iter()
                            .rev()
                            .collect();
                        let status_label = status_label_key(&order.status);
                        let status_color = status_color(&order.status);
                        let total_str = crate::trios::pricing::order_total_text(order.total, crate::ui::components::bike_card::DASH);
                        let order_id_for_reorder = order.id.clone();
                        let init_for_reorder = init_data.clone();
                        let tid_for_reorder = telegram_id.unwrap_or(0);
                        let cart_for_reorder = cart;
                        let nav_for_reorder = nav;
                        let reorder_cta = t(lang, T_HOME_REORDER_CTA).to_string();
                        rsx! {
                            div { style: "margin:0 16px 16px;background:linear-gradient(135deg,#1a1a2e,#16213e);border:4px solid {status_color};box-shadow:4px 4px 0 #000;padding:14px;cursor:pointer;display:flex;align-items:center;gap:12px;",
                                Link {
                                    to: Route::OrderDetail { id: order.id.clone() },
                                    style: "display:flex;align-items:center;gap:12px;flex:1;min-width:0;text-decoration:none;",
                                    div { style: "font-size:36px;line-height:1;", "🔄" }
                                    div { style: "flex:1;min-width:0;",
                                        // `flex-wrap` + `min-width:0` on the row: without them the
                                        // label and the status refuse to shrink below their content,
                                        // and a long status ("Ожидает подтверждения") overflowed the
                                        // Link box and ran underneath the reorder button.
                                        div { style: "display:flex;align-items:baseline;flex-wrap:wrap;gap:2px 8px;margin-bottom:4px;min-width:0;",
                                            span { style: "font-size:12px;font-weight:700;color:{status_color};text-transform:uppercase;letter-spacing:1px;white-space:nowrap;", {t(lang, T_HOME_REORDER_LAST)} }
                                            span { style: "font-size:11px;color:#8b8b9e;min-width:0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;", "{t(lang, T_HOME_REORDER_STATUS)}: {t(lang, status_label)}" }
                                        }
                                        div { style: "font-size:14px;font-weight:700;color:#e8e8e8;white-space:nowrap;overflow:hidden;text-overflow:ellipsis;margin-bottom:6px;",
                                            "{tf(lang, crate::trios::i18n::T_ORDERS_ORDER, std::slice::from_ref(&short_id))} · {total_str}"
                                        }
                                    }
                                }
                                button {
                                    // `flex:0 0 auto` so the button keeps its own width instead of
                                    // being squeezed — or overlapped — by the text beside it.
                                    style: "flex:0 0 auto;background:#39ff14;color:#000;padding:8px 12px;border:3px solid #2d9e0f;font-size:12px;font-weight:700;box-shadow:2px 2px 0 #000;white-space:nowrap;cursor:pointer;",
                                    onclick: move |e: Event<MouseData>| {
                                        e.stop_propagation();
                                        let oid = order_id_for_reorder.clone();
                                        let init = init_for_reorder.clone();
                                        let tid = tid_for_reorder;
                                        let mut cart_sig = cart_for_reorder;
                                        let nav = nav_for_reorder;
                                        let base = api_base_url();
                                        spawn(async move {
                                            let detail_url = format!("{}/api/orders/{}/details?telegram_id={}", base, oid, tid);
                                            if let Ok(text) = fetch_text_authed(&detail_url, &init).await {
                                                if let Ok(resp) = serde_json::from_str::<ReorderOrderDetailResponse>(&text) {
                                                    let items: Vec<CartItem> = resp.order.items.iter().filter_map(reorder_item_to_cart_item).collect();
                                                    match merge_server_cart(&base, &init, tid, &items).await {
                                                        Ok(fresh_cart) => { cart_sig.set(fresh_cart); }
                                                        Err(_) => {
                                                            let mut local = Cart::new();
                                                            for item in items { local.add_item(item); }
                                                            cart_sig.set(local);
                                                        }
                                                    }
                                                    let _ = post_client_event(&base, "reorder_clicked", "home").await;
                                                    nav.push(Route::Cart {});
                                                }
                                            }
                                        });
                                    },
                                    "{reorder_cta}"
                                }
                            }
                        }
                    }
                    Some(None) | None => rsx! {},
                }
            }

            // Карусель наборов убрана: каждая карточка вела в `Route::Sets`,
            // а сами наборы сняты с витрины миграцией 085. Состояние загрузки
            // при этом рисовало заголовок «НАБОРЫ» и два плейсхолдера при
            // каждом открытии экрана — ещё до того, как API успевал ответить
            // пустотой.

            // Блок «Сорт дня» убран целиком, а не перенацелен.
            //
            // Он читал `/api/strains/strain-of-day` — маршрут, которого в
            // роутере больше нет: миграция 083 удаляет каннабис-каталог, и
            // сервер отдаёт на этот путь index.html SPA-фолбэка. Разбор JSON
            // падал, ветка `Some(Err(_))` рисовала заголовок «⭐ Сорт дня» и
            // скелетон — и так на первом экране у каждого посетителя, всегда.
            // Это и был последний живой каннабис-текст в продукте (#2, #26).
            //
            // Мотоциклетного аналога у «сорта дня» нет: витрина — это флот, а
            // не одна позиция со скидкой дня. Когда владелец захочет «байк
            // недели», это будет новый блок поверх `/api/bikes`, а не
            // переименование этого.

            // Блок «КАТЕГОРИИ» (сетка 3x2) убран: из шести плиток жила одна.
            // Menu вела на каталог байков, а Sets, Sommelier, Accessories, Tea
            // и Garden — на витрины каннабис-эпохи, которые миграция 083
            // удаляет, а 085 снимает с публикации. Маршруты и экраны остальных
            // сохранены в routes/screens, как и у блоков ниже; у сада не
            // сохранено ничего — он удалён целиком (D5).
            //
            // Та же отставка уже была проведена в components/bottom_nav.rs;
            // здесь её не повторили, потому что tests/customer_surface_wiring.rs
            // читал только nav и никогда этот файл.

            // Раздел «Приключения» (ежедневный квест, охота за сокровищами,
            // AR-охота, локационный квест) убран с главной по просьбе владельца:
            // разделы не работали. Маршруты и экраны сохранены в routes/screens,
            // так что блок можно вернуть, когда квесты будут доделаны.

            // Tech Tree временно скрыт (по просьбе владельца). Код сохранён в routes/screens.
            // div { style: "padding:0 16px 16px;",
            //     h2 { style: "...", "🔧 More" }
            //     div { style: "display:grid;grid-template-columns:1fr;gap:12px;",
            //         Link { to: Route::TechTree {}, ... }
            //     }
            // }

            // Trust / compliance footer
            div { style: "padding:0 16px 24px;",
                div { style: "
                    background:#16213e;border:4px solid #2a2a4a;
                    box-shadow:4px 4px 0 #000;
                    padding:14px;
                ",
                    div { style: "display:flex;flex-wrap:wrap;gap:8px;margin-bottom:10px;",
                        span { style: "font-size:12px;font-weight:700;background:rgba(57,255,20,0.15);color:#39ff14;padding:4px 8px;border:2px solid #39ff14;", {t(crate::ui::lang::current_lang(), T_TRUST_GACP)} }
                        span { style: "font-size:12px;font-weight:700;background:rgba(255,71,87,0.15);color:#ff4757;padding:4px 8px;border:2px solid #ff4757;", {t(crate::ui::lang::current_lang(), T_TRUST_AGE)} }
                    }
                    p { style: "font-size:12px;color:#8b8b9e;line-height:1.4;margin-bottom:10px;", {t(crate::ui::lang::current_lang(), T_TRUST_MEDICAL)} }
                    a { style: "font-size:13px;font-weight:700;color:#00e5ff;text-decoration:underline;",
                        href: "https://t.me/turboagent_phuket_bot",
                        target: "_blank",
                        {t(crate::ui::lang::current_lang(), T_TRUST_SUPPORT)}
                    }
                }
            }

            BottomNav { cart_count }
        }
    }
}
