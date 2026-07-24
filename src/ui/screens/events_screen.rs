// Events calendar screen — weekly day picker + event list + booking modal.
//
// MVP booking is free: users reserve a seat; capacity and idempotency are
// enforced server-side. Paid flow will reuse the same endpoint once price
// handling is wired.

use crate::trios::i18n::{
    t, T_BACK, T_EVENTS_BOOK, T_EVENTS_BOOKED, T_EVENTS_BOOK_FREE, T_EVENTS_CAPACITY,
    T_EVENTS_DATE, T_EVENTS_ERROR, T_EVENTS_LOADING, T_EVENTS_NO_EVENTS, T_EVENTS_PRICE,
    T_EVENTS_SOLD_OUT, T_EVENTS_SUBTITLE, T_EVENTS_TITLE, T_EVENTS_WEEKDAY_FRI,
    T_EVENTS_WEEKDAY_MON, T_EVENTS_WEEKDAY_SAT, T_EVENTS_WEEKDAY_SUN, T_EVENTS_WEEKDAY_THU,
    T_EVENTS_WEEKDAY_TUE, T_EVENTS_WEEKDAY_WED,
};
use crate::ui::api::context::api_base_url;
use crate::ui::api::http::{
    fetch_text_authed_full, fetch_text_full, post_json_authed_idempotent_full,
};
use crate::ui::api::types::Event as CalendarEvent;
use crate::ui::components::bottom_nav::BottomNav;
use crate::ui::routes::Route;
use crate::ui::share::{ProductKind, SharedProduct};
use crate::ui::telegram::{use_telegram_id, use_telegram_init_data};
use chrono::{Datelike, Days, FixedOffset, NaiveDate, Utc, Weekday};
use dioxus::prelude::*;
use serde_json::json;

const BANGKOK_OFFSET_SECONDS: i32 = 7 * 3600;

/// Metrics are backend-only; the WASM UI cannot reach `crate::metrics`.
/// This wrapper no-ops in the WASM build and delegates to the real helpers
/// when the backend crate is being compiled (e.g. unit tests that include
/// UI modules are rare, but the cfg keeps both targets green).
#[cfg(target_arch = "wasm32")]
fn track_event(_name: &str, _detail: &str) {}

#[cfg(all(not(target_arch = "wasm32"), feature = "backend"))]
fn track_event(name: &str, detail: &str) {
    match name {
        "shared" => crate::metrics::event_shared(detail),
        "booking_attempted" => crate::metrics::event_booking_attempted(detail),
        "booking_succeeded" => crate::metrics::event_booking_succeeded(detail),
        "booking_failed" => crate::metrics::event_booking_failed(detail),
        "detail_opened" => crate::metrics::event_detail_opened(detail),
        _ => {}
    }
}

fn bangkok_offset() -> FixedOffset {
    FixedOffset::east_opt(BANGKOK_OFFSET_SECONDS)
        .unwrap_or_else(|| FixedOffset::east_opt(0).expect("UTC offset 0 is valid"))
}

fn bangkok_now() -> chrono::DateTime<FixedOffset> {
    Utc::now().with_timezone(&bangkok_offset())
}

pub(crate) fn parse_event_start(iso: &str) -> Option<chrono::DateTime<FixedOffset>> {
    iso.parse::<chrono::DateTime<Utc>>()
        .ok()
        .map(|dt| dt.with_timezone(&bangkok_offset()))
}

fn weekday_label(wd: Weekday, lang: crate::trios::core::Lang) -> String {
    let key = match wd {
        Weekday::Mon => T_EVENTS_WEEKDAY_MON,
        Weekday::Tue => T_EVENTS_WEEKDAY_TUE,
        Weekday::Wed => T_EVENTS_WEEKDAY_WED,
        Weekday::Thu => T_EVENTS_WEEKDAY_THU,
        Weekday::Fri => T_EVENTS_WEEKDAY_FRI,
        Weekday::Sat => T_EVENTS_WEEKDAY_SAT,
        Weekday::Sun => T_EVENTS_WEEKDAY_SUN,
    };
    t(lang, key).to_string()
}

fn month_day_label(date: NaiveDate, lang: crate::trios::core::Lang) -> String {
    let month = match lang {
        crate::trios::core::Lang::Russian => match date.month() {
            1 => "янв",
            2 => "фев",
            3 => "мар",
            4 => "апр",
            5 => "май",
            6 => "июн",
            7 => "июл",
            8 => "авг",
            9 => "сен",
            10 => "окт",
            11 => "ноя",
            12 => "дек",
            _ => "",
        },
        _ => match date.month() {
            1 => "Jan",
            2 => "Feb",
            3 => "Mar",
            4 => "Apr",
            5 => "May",
            6 => "Jun",
            7 => "Jul",
            8 => "Aug",
            9 => "Sep",
            10 => "Oct",
            11 => "Nov",
            12 => "Dec",
            _ => "",
        },
    };
    format!("{} {}", date.day(), month)
}

fn this_week_monday(today: NaiveDate) -> NaiveDate {
    let wd = today.weekday().num_days_from_monday() as i64;
    today
        .checked_sub_days(Days::new(wd as u64))
        .unwrap_or(today)
}

#[derive(Props, PartialEq, Clone)]
struct WeekSelectorProps {
    selected: Signal<NaiveDate>,
    today: NaiveDate,
}

#[component]
fn WeekSelector(props: WeekSelectorProps) -> Element {
    let mut selected = props.selected;
    let today = props.today;
    let monday = this_week_monday(today);
    let lang = crate::ui::lang::current_lang();

    rsx! {
        div { style: "display:flex;justify-content:space-between;gap:6px;padding:0 12px 12px;overflow-x:auto;",
            {
                (0..7)
                    .filter_map(|i| monday.checked_add_days(Days::new(i)))
                    .map(|day| {
                        let is_selected = day == *selected.read();
                        let is_today = day == today;
                        let (bg, border, color) = if is_selected {
                            ("#39ff14", "#39ff14", "#000")
                        } else if is_today {
                            ("rgba(57,255,20,0.15)", "#39ff14", "#39ff14")
                        } else {
                            ("#1a1a2e", "#2a2a4a", "#e8e8e8")
                        };
                        let label = weekday_label(day.weekday(), lang);
                        let md = month_day_label(day, lang);
                        rsx! {
                            button {
                                style: "flex:1;min-width:44px;display:flex;flex-direction:column;align-items:center;justify-content:center;padding:8px 4px;border:2px solid {border};background:{bg};color:{color};border-radius:10px;cursor:pointer;font-family:'Press Start 2P',monospace;",
                                onclick: move |_| selected.set(day),
                                span { style: "font-size:10px;text-transform:uppercase;", "{label}" }
                                span { style: "font-size:12px;font-weight:700;margin-top:2px;", "{md}" }
                            }
                        }
                    })
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
enum BookingState {
    Idle,
    Loading,
    Done { seats: i32 },
    Error(String),
}

#[derive(Props, PartialEq, Clone)]
struct EventCardProps {
    ev: CalendarEvent,
    on_select: EventHandler<CalendarEvent>,
}

#[component]
fn EventCard(props: EventCardProps) -> Element {
    let ev = props.ev;
    let start = parse_event_start(&ev.starts_at);
    let start_label = start.map(|s| s.format("%H:%M").to_string());
    let avail = ev.max_seats.map(|cap| cap.saturating_sub(ev.seats_taken as i32));
    let is_free = ev.price_baht.map_or(true, |p| p <= 0.0);
    let price_label = ev.price_baht.filter(|p| *p > 0.0).map(|p| format!("{:.0} ฿", p));

    let share_id = ev.id.clone();
    let share_name = ev.display_title();
    rsx! {
        div {
            role: "button",
            "aria-label": "Open event details",
            style: "width:100%;text-align:left;background:#1a1a2e;border:3px solid #2a2a4a;border-radius:12px;padding:14px;display:flex;flex-direction:column;gap:6px;cursor:pointer;box-shadow:3px 3px 0 #000;",
            onclick: move |_| props.on_select.call(ev.clone()),
            div { style: "display:flex;justify-content:space-between;align-items:flex-start;",
                div { style: "font-size:15px;font-weight:700;color:#e8e8e8;", "{ev.display_title()}" }
                button {
                    style: "background:transparent;border:none;color:#39ff14;font-size:18px;cursor:pointer;width:44px;height:44px;display:flex;align-items:center;justify-content:center;",
                    "aria-label": "Share event",
                    onclick: move |e| {
                        e.stop_propagation();
                        track_event("shared", "event");
                        crate::ui::share::share_product(crate::ui::share::ProductKind::Event, &share_id, &share_name);
                    },
                    "↗"
                }
            }
            if let Some(ref label) = start_label {
                div { style: "font-size:12px;color:#888;", "🕒 {label}" }
            }
            if let Some(ref loc) = ev.location_text {
                div { style: "font-size:11px;color:#b388ff;", "📍 {loc}" }
            }
            div { style: "display:flex;gap:8px;flex-wrap:wrap;margin-top:4px;",
                if ev.is_sold_out() {
                    span { style: "font-size:10px;color:#ff4757;border:2px solid #ff4757;padding:2px 6px;", "SOLD OUT" }
                } else if let Some(a) = avail {
                    span { style: "font-size:10px;color:#39ff14;border:2px solid #39ff14;padding:2px 6px;", "{a} seats" }
                }
                if is_free {
                    span { style: "font-size:10px;color:#39ff14;border:2px solid #39ff14;padding:2px 6px;", "FREE" }
                } else if let Some(ref p) = price_label {
                    span { style: "font-size:10px;color:#ffe600;border:2px solid #ffe600;padding:2px 6px;", "{p}" }
                }
            }
        }
    }
}

#[derive(Props, PartialEq, Clone)]
struct EventBookingModalProps {
    ev: CalendarEvent,
    telegram_id: Option<i64>,
    init_data: String,
    on_close: EventHandler<()>,
    on_booked: EventHandler<()>,
}

#[component]
fn EventBookingModal(props: EventBookingModalProps) -> Element {
    let mut state = use_signal(|| BookingState::Idle);
    let ev = props.ev;
    let lang = crate::ui::lang::current_lang();
    let title = ev.display_title();
    let desc = ev.display_description();
    let back_label = t(lang, T_BACK).to_string();

    let starts = parse_event_start(&ev.starts_at);
    let has_started = starts.map_or(false, |dt| Utc::now() >= dt.with_timezone(&Utc));
    let can_book = props.telegram_id.is_some() && !ev.is_sold_out() && !has_started;

    let date_label = starts.map(|s| s.format("%d %b %Y • %H:%M").to_string());

    let cap_badge = ev.max_seats.map(|cap| {
        let taken = ev.seats_taken;
        let avail = cap.saturating_sub(taken as i32);
        let cap_label = t(lang, T_EVENTS_CAPACITY).to_string();
        rsx! {
            div { style: "font-size:12px;color:#ffe600;background:rgba(255,230,0,0.1);padding:6px 10px;border:2px solid #ffe600;",
                "{cap_label}: {taken}/{cap} ({avail} free)"
            }
        }
    });

    let price_badge = ev.price_baht.map(|price| {
        if price > 0.0 {
            let price_label = t(lang, T_EVENTS_PRICE).to_string();
            let price_str = format!("{:.0}", price);
            rsx! {
                div { style: "font-size:12px;color:#39ff14;background:rgba(57,255,20,0.1);padding:6px 10px;border:2px solid #39ff14;",
                    "{price_label}: {price_str} ฿"
                }
            }
        } else {
            let free_label = t(lang, T_EVENTS_BOOK_FREE).to_string();
            rsx! {
                div { style: "font-size:12px;color:#39ff14;background:rgba(57,255,20,0.1);padding:6px 10px;border:2px solid #39ff14;",
                    "{free_label}"
                }
            }
        }
    });

    let book_label = t(lang, T_EVENTS_BOOK).to_string();
    let sold_label = t(lang, T_EVENTS_SOLD_OUT).to_string();
    let started_label = t(lang, T_EVENTS_DATE).to_string();
    let book_button_text = if matches!(&*state.read(), BookingState::Loading) {
        "⏳".to_string()
    } else if ev.is_sold_out() {
        sold_label
    } else if has_started {
        started_label
    } else if props.telegram_id.is_none() {
        "Telegram required".to_string()
    } else {
        book_label
    };

    let btn_style = if can_book {
        "margin-top:20px;width:100%;padding:14px;background:#39ff14;color:#000;border:none;font-size:14px;font-weight:800;box-shadow:3px 3px 0 #000;cursor:pointer;"
    } else {
        "margin-top:20px;width:100%;padding:14px;background:#2a2a4a;color:#888;border:none;font-size:14px;font-weight:800;cursor:not-allowed;"
    };

    let ev_for_book = ev.clone();
    let booking_area = match state.read().clone() {
        BookingState::Idle | BookingState::Loading => rsx! {
            button {
                style: btn_style,
                disabled: !can_book || matches!(&*state.read(), BookingState::Loading),
                onclick: move |_| {
                    if let Some(tid) = props.telegram_id {
                        let ev_clone = ev_for_book.clone();
                        let init = props.init_data.clone();
                        state.set(BookingState::Loading);
                        spawn(async move {
                            let base = api_base_url();
                            let url = format!("{}/api/events/{}/book", base, urlencoding::encode(&ev_clone.id));
                            let idempotency_key = format!("evt_{}_{}_1", ev_clone.id, tid);
                            let body = json!({ "telegram_id": tid, "seats": 1 }).to_string();
                            track_event("booking_attempted", "A");
                            match post_json_authed_idempotent_full(&url, &init, &idempotency_key, &body).await {
                                Ok((status, _)) if (200..300).contains(&status) => {
                                    track_event("booking_succeeded", "A");
                                    state.set(BookingState::Done { seats: 1 });
                                }
                                Ok((status, body)) => {
                                    let snippet: String = body.chars().take(120).collect();
                                    track_event("booking_failed", &format!("http_{status}"));
                                    state.set(BookingState::Error(format!("HTTP {status}: {snippet}")));
                                }
                                Err(e) => {
                                    track_event("booking_failed", "network");
                                    state.set(BookingState::Error(e.to_string()));
                                }
                            }
                        });
                    }
                },
                "{book_button_text}"
            }
        },
        BookingState::Done { seats } => {
            let booked = t(lang, T_EVENTS_BOOKED).to_string();
            rsx! {
                div { style: "margin-top:20px;padding:14px;background:rgba(57,255,20,0.15);border:2px solid #39ff14;color:#39ff14;font-size:13px;text-align:center;",
                    "✅ {booked} ({seats} seat)"
                }
                button {
                    style: "margin-top:12px;width:100%;padding:12px;background:#1a1a2e;color:#e8e8e8;border:2px solid #39ff14;font-size:13px;cursor:pointer;",
                    onclick: move |_| {
                        state.set(BookingState::Idle);
                        props.on_booked.call(());
                    },
                    "OK"
                }
            }
        }
        BookingState::Error(msg) => {
            let err_text = t(lang, T_EVENTS_ERROR).to_string();
            rsx! {
                div { style: "margin-top:20px;padding:12px;background:rgba(255,71,87,0.15);border:2px solid #ff4757;color:#ff4757;font-size:12px;",
                    "⚠ {err_text}: {msg}"
                }
                button {
                    style: "margin-top:12px;width:100%;padding:12px;background:#1a1a2e;color:#e8e8e8;border:2px solid #39ff14;font-size:13px;cursor:pointer;",
                    onclick: move |_| state.set(BookingState::Idle),
                    "Retry"
                }
            }
        }
    };

    rsx! {
        div {
            role: "dialog",
            "aria-modal": "true",
            "aria-label": "{title} details",
            style: "position:fixed;inset:0;z-index:50;background:rgba(0,0,0,0.85);display:flex;align-items:flex-end;justify-content:center;",
            onclick: move |_| props.on_close.call(()),
            div {
                style: "width:100%;max-height:90vh;background:#0f0f1a;border-top:4px solid #39ff14;border-radius:20px 20px 0 0;padding:20px 16px 24px;overflow-y:auto;",
                onclick: move |e| e.stop_propagation(),
                div { style: "display:flex;justify-content:space-between;align-items:flex-start;",
                    h2 { style: "font-size:18px;font-weight:800;color:#39ff14;text-shadow:2px 2px 0 #000;margin:0;", "{title}" }
                    button {
                        style: "background:transparent;border:none;color:#ff4757;font-size:24px;width:44px;height:44px;display:flex;align-items:center;justify-content:center;cursor:pointer;",
                        "aria-label": back_label.clone(),
                        onclick: move |_| props.on_close.call(()),
                        "✕"
                    }
                }
                if let Some(ref label) = date_label {
                    div { style: "font-size:12px;color:#888;margin-top:8px;", "🗓️ {label}" }
                }
                if let Some(ref loc) = ev.location_text {
                    div { style: "font-size:12px;color:#b388ff;margin-top:4px;", "📍 {loc}" }
                }
                if let Some(ref d) = desc {
                    p { style: "font-size:13px;color:#e8e8e8;line-height:1.5;margin-top:12px;", "{d}" }
                }
                div { style: "margin-top:16px;display:flex;gap:12px;flex-wrap:wrap;",
                    { cap_badge }
                    { price_badge }
                }
                { booking_area }
            }
        }
    }
}

#[component]
pub fn EventsScreen() -> Element {
    let today = bangkok_now().date_naive();
    let selected = use_signal(|| today);
    let mut selected_event = use_signal(|| None::<CalendarEvent>);
    let telegram_id = use_telegram_id();
    let init_data = use_telegram_init_data();
    let lang = crate::ui::lang::current_lang();

    // Deep-link events: if the app opened with a shared event, jump to the
    // event detail route instead of the calendar list.
    let nav = use_navigator();
    let mut pending = use_context::<Signal<Option<SharedProduct>>>();
    use_effect(move || {
        let target = pending.read().clone();
        if let Some(target) = target {
            if target.kind == ProductKind::Event {
                pending.set(None);
                track_event("detail_opened", "share");
                nav.push(Route::EventDetail { id: target.id });
            }
        }
    });

    let title = t(lang, T_EVENTS_TITLE).to_string();
    let subtitle = t(lang, T_EVENTS_SUBTITLE).to_string();
    let loading = t(lang, T_EVENTS_LOADING).to_string();
    let no_events = t(lang, T_EVENTS_NO_EVENTS).to_string();

    let mut events_resource: Resource<Result<Vec<CalendarEvent>, String>> = use_resource(use_reactive!(|selected| {
        let from = selected.to_string();
        let to = selected
            .read()
            .succ_opt()
            .map(|d| d.to_string())
            .unwrap_or_else(|| from.clone());
        async move {
            let base = api_base_url();
            // Shop timezone is Asia/Bangkok UTC+7; ask the backend for events
            // that start within that day, not UTC midnight.
            let url = format!("{}/api/events?from={}T00:00:00%2B07:00&to={}T00:00:00%2B07:00", base, from, to);
            match crate::ui::api::http::fetch_text_full(&url).await {
                Ok((status, body)) if (200..300).contains(&status) => {
                    let parsed: serde_json::Value = serde_json::from_str(&body).unwrap_or_default();
                    let events: Vec<CalendarEvent> = parsed
                        .get("events")
                        .and_then(|v| v.as_array())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|v| serde_json::from_value(v.clone()).ok())
                                .collect()
                        })
                        .unwrap_or_default();
                    Ok(events)
                }
                Ok((status, _)) => Err(format!("HTTP {status}")),
                Err(e) => Err(e.to_string()),
            }
        }
    }));

    let events_for_day = use_memo(move || {
        let sel = selected.read().clone();
        match &*events_resource.read() {
            Some(Ok(list)) => list
                .iter()
                .filter(|e| {
                    parse_event_start(&e.starts_at)
                        .map(|dt| dt.date_naive() == sel)
                        .unwrap_or(false)
                })
                .cloned()
                .collect::<Vec<_>>(),
            _ => Vec::new(),
        }
    });

    let refresh = move |_| {
        events_resource.restart();
    };

    let list = events_for_day.read().clone();
    let has_events = !list.is_empty();

    let event_nodes = if has_events {
        rsx! {
            div { style: "display:flex;flex-direction:column;gap:12px;",
                {
                    list.into_iter()
                        .map(move |ev| {
                            rsx! {
                                EventCard {
                                    ev,
                                    on_select: EventHandler::new(move |e: CalendarEvent| {
                                        selected_event.set(Some(e));
                                    }),
                                }
                            }
                        })
                }
            }
        }
    } else {
        rsx! { div { style: "text-align:center;padding:40px 0;color:#888;font-size:12px;", "{no_events}" } }
    };

    let modal = selected_event.read().clone().map(|ev| {
        rsx! {
            EventBookingModal {
                ev,
                telegram_id,
                init_data: init_data.clone(),
                on_close: EventHandler::new(move |_| selected_event.set(None)),
                on_booked: EventHandler::new(move |_| {
                    selected_event.set(None);
                    events_resource.restart();
                }),
            }
        }
    });

    rsx! {
        div { style: "min-height:100vh;background:#0f0f1a;color:#e8e8e8;padding-bottom:80px;",
            div { style: "padding:20px 16px 8px;text-align:center;",
                h1 { style: "font-size:22px;font-weight:800;color:#39ff14;text-shadow:3px 3px 0 #000;letter-spacing:2px;", "{title}" }
                p { style: "font-size:12px;color:#888;margin-top:4px;", "{subtitle}" }
            }
            WeekSelector { selected, today }
            div { style: "padding:0 16px;",
                match &*events_resource.read() {
                    None => rsx! { div { style: "text-align:center;padding:40px 0;color:#888;font-size:12px;", "{loading}" } },
                    Some(Err(e)) => {
                        let err_msg = e.clone();
                        rsx! {
                            div { style: "text-align:center;padding:40px 0;",
                                p { style: "color:#ff4757;font-size:12px;", "{err_msg}" }
                                button {
                                    style: "margin-top:12px;padding:8px 16px;background:#1a1a2e;border:2px solid #39ff14;color:#39ff14;font-size:12px;cursor:pointer;",
                                    onclick: refresh,
                                    "↻ Retry"
                                }
                            }
                        }
                    }
                    Some(Ok(_)) => event_nodes,
                }
            }
            { modal }
            BottomNav {}
        }
    }
}

#[component]
pub fn EventDetailScreen(id: String) -> Element {
    let telegram_id = use_telegram_id();
    let init_data = use_telegram_init_data();
    let mut event = use_signal(|| None::<CalendarEvent>);
    let mut loading = use_signal(|| true);
    let mut error = use_signal(|| None::<String>);
    let nav = use_navigator();

    use_effect(move || {
        let id = id.clone();
        spawn(async move {
            let url = format!("{}/api/events/{}", api_base_url(), urlencoding::encode(&id));
            track_event("detail_opened", "route");
            match fetch_text_full(&url).await {
                Ok((status, body)) if (200..300).contains(&status) => {
                    let parsed: serde_json::Value = serde_json::from_str(&body).unwrap_or_default();
                    if let Some(e) = parsed.get("event").and_then(|v| serde_json::from_value(v.clone()).ok()) {
                        event.set(Some(e));
                    } else {
                        error.set(Some("Event not found".to_string()));
                    }
                }
                Ok((status, _)) => error.set(Some(format!("HTTP {status}"))),
                Err(e) => error.set(Some(e.to_string())),
            }
            loading.set(false);
        });
    });

    let ev = event.read().clone();
    let go_back = move |_| { nav.push(Route::Events {}); };
    rsx! {
        div { style: "min-height:100vh;background:#0f0f1a;color:#e8e8e8;padding-bottom:80px;",
            if *loading.read() {
                div { style: "text-align:center;padding:40px 0;color:#888;font-size:12px;", "Loading..." }
            } else if let Some(err) = error.read().clone() {
                div { style: "text-align:center;padding:40px 0;color:#ff4757;font-size:12px;", "{err}" }
            } else if let Some(ev) = ev {
                EventBookingModal {
                    ev,
                    telegram_id,
                    init_data: init_data.clone(),
                    on_close: EventHandler::new(go_back.clone()),
                    on_booked: EventHandler::new(go_back),
                }
            }
            BottomNav {}
        }
    }
}

#[derive(Clone, Debug, serde::Deserialize)]
#[allow(dead_code)]
struct MyBooking {
    id: String,
    event_id: String,
    seats: i32,
    status: String,
    #[serde(default)]
    event_title: String,
    #[serde(default)]
    event_title_en: Option<String>,
    #[serde(default)]
    event_starts_at: Option<String>,
    #[serde(default)]
    event_location_text: Option<String>,
}

#[component]
pub fn MyBookingsScreen() -> Element {
    let telegram_id = use_telegram_id();
    let init_data = use_telegram_init_data();
    let mut bookings: Signal<Vec<MyBooking>> = use_signal(Vec::new);
    let mut loading = use_signal(|| true);
    let mut error = use_signal(|| None::<String>);
    let refresh = use_signal(|| 0u32);

    let init_data_for_effect = init_data.clone();
    let init_data_for_cancel = init_data.clone();

    use_effect(move || {
        let _ = refresh.read();
        let Some(tid) = telegram_id else {
            loading.set(false);
            return;
        };
        let init = init_data_for_effect.clone();
        spawn(async move {
            let url = format!(
                "{}/api/events/my-bookings?telegram_id={}",
                api_base_url(),
                tid
            );
            match fetch_text_authed_full(&url, &init).await {
                Ok((status, body)) if (200..300).contains(&status) => {
                    let parsed: serde_json::Value = serde_json::from_str(&body).unwrap_or_default();
                    let list: Vec<MyBooking> = parsed
                        .get("bookings")
                        .and_then(|v| v.as_array())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|v| serde_json::from_value(v.clone()).ok())
                                .collect()
                        })
                        .unwrap_or_default();
                    bookings.set(list);
                }
                Ok((status, _)) => error.set(Some(format!("HTTP {status}"))),
                Err(e) => error.set(Some(e.to_string())),
            }
            loading.set(false);
        });
    });

    let lang = crate::ui::lang::current_lang();
    let back_label = t(lang, T_BACK).to_string();
    let nav = use_navigator();

    let list = bookings.read().clone();
    let has_bookings = !list.is_empty();

    let cancel = use_callback(move |booking_id: String| {
        let tid = telegram_id.unwrap_or(0);
        if tid == 0 {
            return;
        }
        let init = init_data_for_cancel.clone();
        let mut refresh_sig = refresh.clone();
        spawn(async move {
            let url = format!(
                "{}/api/events/bookings/{}/cancel?telegram_id={}",
                api_base_url(),
                urlencoding::encode(&booking_id),
                tid
            );
            let _ = crate::ui::api::http::put_json_authed(
                &url, &init, "{}",
            ).await;
            let next = *refresh_sig.read() + 1;
            refresh_sig.set(next);
        });
    });

    rsx! {
        div { style: "min-height:100vh;background:#0f0f1a;color:#e8e8e8;padding-bottom:80px;",
            div { style: "padding:20px 16px 16px;text-align:center;position:relative;",
                button {
                    style: "position:absolute;left:16px;top:20px;background:transparent;border:none;color:#e8e8e8;font-size:20px;width:44px;height:44px;display:flex;align-items:center;justify-content:center;cursor:pointer;",
                    "aria-label": back_label.clone(),
                    onclick: move |_| { nav.push(Route::Events {}); },
                    "←"
                }
                h1 { style: "font-size:20px;font-weight:800;color:#39ff14;text-shadow:2px 2px 0 #000;", "My bookings" }
            }
            if *loading.read() {
                div { style: "text-align:center;padding:40px 0;color:#888;font-size:12px;", "Loading..." }
            } else if let Some(err) = error.read().clone() {
                div { style: "text-align:center;padding:40px 0;color:#ff4757;font-size:12px;", "{err}" }
            } else if !has_bookings {
                div { style: "text-align:center;padding:40px 0;color:#888;font-size:12px;", "No bookings yet" }
            } else {
                div { style: "padding:0 16px;display:flex;flex-direction:column;gap:10px;",
                    {
                        list.into_iter().map(move |b| {
                            let start = b.event_starts_at.as_deref().and_then(parse_event_start).map(|dt| dt.format("%d %b %Y • %H:%M").to_string()).unwrap_or_default();
                            let show_title = crate::ui::lang::localized(
                                &b.event_title,
                                b.event_title_en.as_deref(),
                            );
                            let can_cancel = b.status == "confirmed" || b.status == "waitlisted";
                            let bid = b.id.clone();
                            let bid2 = b.id.clone();
                            let status_color = if b.status == "confirmed" { "#39ff14" } else if b.status == "waitlisted" { "#ffe600" } else { "#ff4757" };
                            rsx! {
                                div { key: "{bid2}", style: "background:#1a1a2e;border:2px solid #2a2a4a;border-radius:10px;padding:12px;display:flex;flex-direction:column;gap:6px;",
                                    div { style: "font-size:14px;font-weight:700;color:#e8e8e8;", "{show_title}" }
                                    div { style: "font-size:11px;color:#888;", "{start}" }
                                    if let Some(loc) = b.event_location_text {
                                        div { style: "font-size:11px;color:#b388ff;", "📍 {loc}" }
                                    }
                                    div { style: "font-size:11px;color:{status_color};text-transform:uppercase;", "{b.status}" }
                                    if can_cancel {
                                        button {
                                            style: "margin-top:6px;padding:8px 12px;background:#ff4757;color:#fff;border:none;border-radius:4px;font-size:12px;cursor:pointer;",
                                            onclick: move |_| cancel(bid.clone()),
                                            "Cancel"
                                        }
                                    }
                                }
                            }
                        })
                    }
                }
            }
            BottomNav {}
        }
    }
}
