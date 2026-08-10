// Events calendar screen — month day picker + event list + booking modal.
//
// MVP booking is free: users reserve a seat; capacity and idempotency are
// enforced server-side. Paid flow will reuse the same endpoint once price
// handling is wired.

use crate::trios::i18n::{
    t, tf, T_BACK, T_EVENTS_ALREADY_BOOKED, T_EVENTS_BOOK, T_EVENTS_BOOKED, T_EVENTS_BOOK_FREE,
    T_EVENTS_CANCEL, T_EVENTS_CAPACITY, T_EVENTS_DATE, T_EVENTS_ERROR, T_EVENTS_EVENT_NOT_FOUND,
    T_EVENTS_FREE_BADGE, T_EVENTS_GALLERY, T_EVENTS_INSUFFICIENT_STARS, T_EVENTS_MONTH_APR,
    T_EVENTS_MONTH_AUG, T_EVENTS_MONTH_DEC, T_EVENTS_MONTH_FEB, T_EVENTS_MONTH_JAN,
    T_EVENTS_MONTH_JUL, T_EVENTS_MONTH_JUN, T_EVENTS_MONTH_MAR, T_EVENTS_MONTH_MAY,
    T_EVENTS_MONTH_NOV, T_EVENTS_MONTH_OCT, T_EVENTS_MONTH_SEP, T_EVENTS_MY_BOOKINGS,
    T_EVENTS_NEXT_PHOTO, T_EVENTS_NO_BOOKINGS, T_EVENTS_NO_EVENTS, T_EVENTS_OK,
    T_EVENTS_OPEN_DETAILS, T_EVENTS_PHOTO_N, T_EVENTS_PREV_PHOTO, T_EVENTS_PRICE,
    T_EVENTS_PRICE_STARS, T_EVENTS_RETRY, T_EVENTS_SEAT, T_EVENTS_SEATS, T_EVENTS_SELECT_SEATS,
    T_EVENTS_SHARE_EVENT, T_EVENTS_SOLD_OUT, T_EVENTS_SOLD_OUT_BADGE, T_EVENTS_SUBTITLE,
    T_EVENTS_TELEGRAM_REQUIRED, T_EVENTS_TIME, T_EVENTS_TITLE, T_EVENTS_VIDEO,
    T_EVENTS_WEEKDAY_FRI, T_EVENTS_WEEKDAY_MON, T_EVENTS_WEEKDAY_SAT, T_EVENTS_WEEKDAY_SUN,
    T_EVENTS_WEEKDAY_THU, T_EVENTS_WEEKDAY_TUE, T_EVENTS_WEEKDAY_WED,
};
use crate::ui::api::context::api_base_url;
use crate::ui::api::http::{
    fetch_text_authed_full, fetch_text_full, post_json_authed_idempotent_full,
};
use crate::ui::api::types::Event as CalendarEvent;
use crate::ui::components::bottom_nav::BottomNav;
use crate::ui::components::skeleton::{Skeleton, SkeletonShape};
use crate::ui::routes::Route;
use crate::ui::share::{ProductKind, SharedProduct};
use crate::ui::telegram::{use_telegram_id, use_telegram_init_data};
use chrono::{Datelike, FixedOffset, NaiveDate, Utc, Weekday};
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

fn month_key(month: u32) -> crate::trios::i18n::Key {
    match month {
        1 => T_EVENTS_MONTH_JAN,
        2 => T_EVENTS_MONTH_FEB,
        3 => T_EVENTS_MONTH_MAR,
        4 => T_EVENTS_MONTH_APR,
        5 => T_EVENTS_MONTH_MAY,
        6 => T_EVENTS_MONTH_JUN,
        7 => T_EVENTS_MONTH_JUL,
        8 => T_EVENTS_MONTH_AUG,
        9 => T_EVENTS_MONTH_SEP,
        10 => T_EVENTS_MONTH_OCT,
        11 => T_EVENTS_MONTH_NOV,
        12 => T_EVENTS_MONTH_DEC,
        _ => T_EVENTS_MONTH_JAN,
    }
}

#[derive(Props, PartialEq, Clone)]
struct WeekSelectorProps {
    selected: Signal<NaiveDate>,
    today: NaiveDate,
}

/// Day picker covering a whole month, with month-to-month navigation.
///
/// Was a seven-day strip anchored on this week's Monday, which made the rest
/// of the month unreachable — you could not schedule an event for the 28th.
/// The strip now spans the selected month and scrolls horizontally.
#[component]
fn WeekSelector(props: WeekSelectorProps) -> Element {
    let mut selected = props.selected;
    let today = props.today;
    let lang = crate::ui::lang::current_lang();

    // Which month the strip is showing. Follows `selected` so picking a day in
    // September and coming back keeps you in September.
    let anchor = crate::trios::calendar::first_of_month(*selected.read());
    let month_title = format!("{} {}", t(lang, month_key(anchor.month())), anchor.year());

    rsx! {
        div { style: "display:flex;align-items:center;justify-content:space-between;gap:8px;padding:0 12px 8px;",
            button {
                style: "min-width:44px;min-height:44px;background:#1a1a2e;color:#39ff14;border:2px solid #2a2a4a;border-radius:10px;cursor:pointer;font-size:16px;",
                aria_label: "previous month",
                onclick: move |_| {
                    let prev = crate::trios::calendar::shift_month(*selected.read(), -1);
                    selected.set(prev);
                },
                "‹"
            }
            span { style: "flex:1;text-align:center;font-size:12px;font-weight:700;color:#39ff14;text-transform:uppercase;letter-spacing:1px;",
                "{month_title}"
            }
            button {
                style: "min-width:44px;min-height:44px;background:#1a1a2e;color:#39ff14;border:2px solid #2a2a4a;border-radius:10px;cursor:pointer;font-size:16px;",
                aria_label: "next month",
                onclick: move |_| {
                    let next = crate::trios::calendar::shift_month(*selected.read(), 1);
                    selected.set(next);
                },
                "›"
            }
        }
        div { style: "display:flex;gap:6px;padding:0 12px 12px;overflow-x:auto;-webkit-overflow-scrolling:touch;",
            {
                crate::trios::calendar::visible_month_days(anchor, today)
                    .into_iter()
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
                        // Day number only: the month name now lives in the
                        // header, and 31 "9 августа" chips would not scroll
                        // usably on a phone.
                        let md = day.day().to_string();
                        rsx! {
                            button {
                                // `flex:0 0 auto` — with `flex:1` a 31-day strip
                                // would squeeze every chip below the 44px tap
                                // target instead of scrolling.
                                style: "flex:0 0 auto;min-width:44px;display:flex;flex-direction:column;align-items:center;justify-content:center;padding:8px 6px;border:2px solid {border};background:{bg};color:{color};border-radius:10px;cursor:pointer;font-family:'Press Start 2P',monospace;",
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
    let avail = ev
        .max_seats
        .map(|cap| cap.saturating_sub(ev.seats_taken as i32));
    let has_baht = ev.price_baht.map_or(false, |p| p > 0.0);
    let has_stars = ev.price_stars.map_or(false, |s| s > 0);
    let is_free = !has_baht && !has_stars;
    let price_label = if has_stars {
        Some(format!("{} ⭐", ev.price_stars.unwrap_or(0)))
    } else if has_baht {
        Some(format!("{:.0} ฿", ev.price_baht.unwrap_or(0.0)))
    } else {
        None
    };
    let lang = crate::ui::lang::current_lang();
    let time_line = start_label
        .as_ref()
        .map(|label| tf(lang, T_EVENTS_TIME, &[label.clone()]));
    let avail_label = avail.map(|a| tf(lang, T_EVENTS_SEATS, &[a.to_string()]));

    let share_id = ev.id.clone();
    let share_name = ev.display_title();
    let thumb = ev.image_url.as_deref().filter(|s| {
        s.starts_with("http://")
            || s.starts_with("https://")
            || (s.starts_with("/") && !s.starts_with("//"))
    });
    rsx! {
        div {
            role: "button",
            "aria-label": t(lang, T_EVENTS_OPEN_DETAILS),
            style: "width:100%;min-height:300px;text-align:left;background:#1a1a2e;border:3px solid #2a2a4a;border-radius:12px;overflow:hidden;cursor:pointer;box-shadow:3px 3px 0 #000;position:relative;display:flex;flex-direction:column;",
            onclick: move |_| props.on_select.call(ev.clone()),
            // Full-card cover image like strain cards.
            div { style: "position:absolute;inset:0;z-index:0;",
                if let Some(ref url) = thumb {
                    img { src: "{url}", alt: "{share_name}", style: "width:100%;height:100%;object-fit:contain;background:#0f0f1a;" }
                } else {
                    div { style: "width:100%;height:100%;background:linear-gradient(135deg,#2a2a4a,#0f0f1a);display:flex;align-items:center;justify-content:center;font-size:48px;", "📅" }
                }
                // Bottom gradient so text is readable over any image.
                div { style: "position:absolute;inset:0;background:linear-gradient(to bottom, rgba(15,15,26,0.35) 0%, rgba(15,15,26,0.65) 60%, rgba(15,15,26,0.92) 100%);" }
            }
            // Share button floats above the image.
            button {
                style: "position:absolute;top:8px;right:8px;z-index:2;background:rgba(0,0,0,0.5);border:2px solid #39ff14;color:#39ff14;border-radius:50%;font-size:18px;cursor:pointer;width:44px;height:44px;display:flex;align-items:center;justify-content:center;",
                "aria-label": t(lang, T_EVENTS_SHARE_EVENT),
                onclick: move |e| {
                    e.stop_propagation();
                    track_event("shared", "event");
                    crate::ui::share::share_product(crate::ui::share::ProductKind::Event, &share_id, &share_name);
                },
                "↗"
            }
            // Text content sits at the bottom over the gradient.
            div { style: "position:relative;z-index:1;margin-top:auto;padding:14px;display:flex;flex-direction:column;gap:6px;",
                div { style: "display:flex;justify-content:space-between;align-items:flex-start;",
                    div { style: "font-size:17px;font-weight:800;color:#fff;text-shadow:2px 2px 0 #000;", "{ev.display_title()}" }
                }
                if let Some(ref line) = time_line {
                    div { style: "font-size:13px;color:#e8e8e8;text-shadow:1px 1px 0 #000;", "{line}" }
                }
                if let Some(ref loc) = ev.location_text {
                    div { style: "font-size:12px;color:#b388ff;text-shadow:1px 1px 0 #000;", "📍 {loc}" }
                }
                div { style: "display:flex;gap:8px;flex-wrap:wrap;margin-top:4px;",
                    if ev.is_sold_out() {
                        span { style: "font-size:10px;color:#ff4757;background:rgba(0,0,0,0.5);border:2px solid #ff4757;padding:2px 6px;", {t(lang, T_EVENTS_SOLD_OUT_BADGE)} }
                    } else if let Some(ref seats_line) = avail_label {
                        span { style: "font-size:10px;color:#39ff14;background:rgba(0,0,0,0.5);border:2px solid #39ff14;padding:2px 6px;", "{seats_line}" }
                    }
                    if is_free {
                        span { style: "font-size:10px;color:#39ff14;background:rgba(0,0,0,0.5);border:2px solid #39ff14;padding:2px 6px;", {t(lang, T_EVENTS_FREE_BADGE)} }
                    } else if let Some(ref p) = price_label {
                        span { style: "font-size:10px;color:#ffe600;background:rgba(0,0,0,0.5);border:2px solid #ffe600;padding:2px 6px;", "{p}" }
                    }
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
    let gallery_label = t(lang, T_EVENTS_GALLERY).to_string();
    let video_label = t(lang, T_EVENTS_VIDEO).to_string();
    let thumb = ev.image_url.as_deref().filter(|s| {
        s.starts_with("http://")
            || s.starts_with("https://")
            || (s.starts_with("/") && !s.starts_with("//"))
    });
    let valid_photos: Vec<String> = ev
        .photos
        .iter()
        .filter(|s| {
            s.starts_with("http://")
                || s.starts_with("https://")
                || (s.starts_with("/") && !s.starts_with("//"))
        })
        .cloned()
        .collect();
    let has_gallery = !valid_photos.is_empty();
    let has_video = ev
        .video_url
        .as_deref()
        .filter(|s| {
            s.starts_with("http://")
                || s.starts_with("https://")
                || (s.starts_with("/") && !s.starts_with("//"))
        })
        .is_some();
    let mut selected_photo = use_signal(|| 0usize);
    let mut seats_to_book = use_signal(|| 1u32);

    let starts = parse_event_start(&ev.starts_at);
    let has_started = starts.map_or(false, |dt| Utc::now() >= dt.with_timezone(&Utc));
    let has_stars_price = ev.price_stars.map_or(false, |s| s > 0);
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

    let price_badge = {
        let has_baht = ev.price_baht.map_or(false, |p| p > 0.0);
        if has_stars_price {
            let price_label = t(lang, T_EVENTS_PRICE_STARS).to_string();
            let price_str = ev.price_stars.unwrap_or(0).to_string();
            rsx! {
                div { style: "font-size:12px;color:#39ff14;background:rgba(57,255,20,0.1);padding:6px 10px;border:2px solid #39ff14;",
                    "{price_label}: {price_str} ⭐"
                }
            }
        } else if has_baht {
            let price_label = t(lang, T_EVENTS_PRICE).to_string();
            let price_str = format!("{:.0}", ev.price_baht.unwrap_or(0.0));
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
    };

    let book_label = t(lang, T_EVENTS_BOOK).to_string();
    let sold_label = t(lang, T_EVENTS_SOLD_OUT).to_string();
    let started_label = t(lang, T_EVENTS_DATE).to_string();
    let telegram_required = t(lang, T_EVENTS_TELEGRAM_REQUIRED).to_string();
    let book_button_text = if matches!(&*state.read(), BookingState::Loading) {
        "⏳".to_string()
    } else if ev.is_sold_out() {
        sold_label
    } else if has_started {
        started_label
    } else if props.telegram_id.is_none() {
        telegram_required
    } else if has_stars_price {
        let seats = seats_to_book().max(1);
        format!(
            "{} ({} ⭐)",
            book_label,
            ev.price_stars.unwrap_or(0) * seats as i64
        )
    } else {
        book_label
    };

    let btn_style = if can_book {
        "margin-top:20px;width:100%;padding:14px;background:#39ff14;color:#000;border:none;font-size:14px;font-weight:800;box-shadow:3px 3px 0 #000;cursor:pointer;"
    } else {
        "margin-top:20px;width:100%;padding:14px;background:#2a2a4a;color:#888;border:none;font-size:14px;font-weight:800;cursor:not-allowed;"
    };

    let max_available = ev.seats_available.map(|a| a.max(1) as u32).unwrap_or(10);
    let seat_selector = if can_book {
        let cur = seats_to_book().clamp(1, max_available);
        let at_min = cur <= 1;
        let at_max = cur >= max_available;
        let min_op = if at_min { "0.35" } else { "1" };
        let min_cur = if at_min { "not-allowed" } else { "pointer" };
        let max_op = if at_max { "0.35" } else { "1" };
        let max_cur = if at_max { "not-allowed" } else { "pointer" };
        let seats_label = t(lang, T_EVENTS_SELECT_SEATS).to_string();
        rsx! {
            div { style: "margin-top:16px;",
                div { style: "font-size:12px;color:#8b8b9e;margin-bottom:6px;", "{seats_label}" }
                div { style: "display:flex;align-items:center;justify-content:center;gap:12px;",
                    button {
                        style: "width:44px;height:44px;font-size:22px;font-weight:800;background:#2a2a4a;color:#e8e8e8;border:4px solid #1a1a2e;box-shadow:2px 2px 0 #000;line-height:1;opacity:{min_op};cursor:{min_cur};",
                        disabled: at_min,
                        onclick: move |e: Event<MouseData>| { e.stop_propagation(); seats_to_book.set(seats_to_book().saturating_sub(1).max(1)); },
                        "−"
                    }
                    span { style: "font-size:20px;font-weight:800;color:#fff;min-width:40px;text-align:center;", "{cur}" }
                    button {
                        style: "width:44px;height:44px;font-size:22px;font-weight:800;background:#2a2a4a;color:#e8e8e8;border:4px solid #1a1a2e;box-shadow:2px 2px 0 #000;line-height:1;opacity:{max_op};cursor:{max_cur};",
                        disabled: at_max,
                        onclick: move |e: Event<MouseData>| { e.stop_propagation(); seats_to_book.set((seats_to_book() + 1).min(max_available)); },
                        "+"
                    }
                }
            }
        }
    } else {
        rsx! {}
    };

    let ev_for_book = ev.clone();
    let booking_area = match state.read().clone() {
        BookingState::Idle | BookingState::Loading => rsx! {
            button {
                style: btn_style,
                disabled: !can_book || matches!(&*state.read(), BookingState::Loading),
                onclick: move |_: Event<MouseData>| {
                    if let Some(tid) = props.telegram_id {
                        let ev_clone = ev_for_book.clone();
                        let init = props.init_data.clone();
                        state.set(BookingState::Loading);
                        spawn(async move {
                            let base = api_base_url();
                            let url = format!("{}/api/events/{}/book", base, urlencoding::encode(&ev_clone.id));
                            // Include a millisecond counter so repeated taps within the same
                            // session create distinct attempts, while retries of an accidental
                            // double-tap are still collapsed by the server-side duplicate guard.
                            let idempotency_key = format!("evt_{}_{}_{}", ev_clone.id, tid, js_sys::Date::now() as u64);
                            let seats = seats_to_book().clamp(1, max_available) as i32;
                            let body = json!({ "telegram_id": tid, "seats": seats }).to_string();
                            track_event("booking_attempted", "B");
                            match post_json_authed_idempotent_full(&url, &init, &idempotency_key, &body).await {
                                Ok((status, _)) if (200..300).contains(&status) => {
                                    track_event("booking_succeeded", "B");
                                    state.set(BookingState::Done { seats });
                                }
                                Ok((status, body)) => {
                                    if status == 402 {
                                        track_event("booking_failed", "insufficient_stars");
                                        let msg = t(lang, T_EVENTS_INSUFFICIENT_STARS).to_string();
                                        state.set(BookingState::Error(msg));
                                    } else if status == 409 {
                                        track_event("booking_failed", "already_booked");
                                        let msg = t(lang, T_EVENTS_ALREADY_BOOKED).to_string();
                                        state.set(BookingState::Error(msg));
                                    } else {
                                        let snippet: String = body.chars().take(120).collect();
                                        track_event("booking_failed", &format!("http_{status}"));
                                        state.set(BookingState::Error(format!("HTTP {status}: {snippet}")));
                                    }
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
            let seat_word = t(lang, T_EVENTS_SEAT).to_string();
            let ok = t(lang, T_EVENTS_OK).to_string();
            rsx! {
                div { style: "margin-top:20px;padding:14px;background:rgba(57,255,20,0.15);border:2px solid #39ff14;color:#39ff14;font-size:13px;text-align:center;",
                    "✅ {booked} ({seats} {seat_word})"
                }
                button {
                    style: "margin-top:12px;width:100%;padding:12px;background:#1a1a2e;color:#e8e8e8;border:2px solid #39ff14;font-size:13px;cursor:pointer;",
                    onclick: move |_| {
                        state.set(BookingState::Idle);
                        props.on_booked.call(());
                    },
                    "{ok}"
                }
            }
        }
        BookingState::Error(msg) => {
            let err_text = t(lang, T_EVENTS_ERROR).to_string();
            let retry = t(lang, T_EVENTS_RETRY).to_string();
            rsx! {
                div { style: "margin-top:20px;padding:12px;background:rgba(255,71,87,0.15);border:2px solid #ff4757;color:#ff4757;font-size:12px;",
                    "⚠ {err_text}: {msg}"
                }
                button {
                    style: "margin-top:12px;width:100%;padding:12px;background:#1a1a2e;color:#e8e8e8;border:2px solid #39ff14;font-size:13px;cursor:pointer;",
                    onclick: move |_| state.set(BookingState::Idle),
                    "{retry}"
                }
            }
        }
    };

    rsx! {
        div {
            role: "dialog",
            "aria-modal": "true",
            "aria-label": "{title} details",
            style: "position:fixed;inset:0;z-index:1000;background:rgba(0,0,0,0.85);display:flex;align-items:flex-end;justify-content:center;",
            onclick: move |_: Event<MouseData>| props.on_close.call(()),
            div {
                style: "width:100%;max-height:90vh;background:#0f0f1a;border-top:4px solid #39ff14;border-radius:20px 20px 0 0;padding:20px 16px 80px;overflow-y:auto;",
                onclick: move |e: Event<MouseData>| e.stop_propagation(),
                div { style: "display:flex;justify-content:space-between;align-items:flex-start;",
                    h2 { style: "font-size:18px;font-weight:800;color:#39ff14;text-shadow:2px 2px 0 #000;margin:0;", "{title}" }
                    button {
                        style: "background:transparent;border:none;color:#ff4757;font-size:24px;width:44px;height:44px;display:flex;align-items:center;justify-content:center;cursor:pointer;",
                        "aria-label": back_label.clone(),
                        onclick: move |_: Event<MouseData>| props.on_close.call(()),
                        "✕"
                    }
                }
                if has_video {
                    div { style: "margin-top:12px;",
                        div { style: "font-size:12px;color:#888;margin-bottom:4px;", "🎥 {video_label}" }
                        video {
                            src: "{ev.video_url.as_deref().unwrap_or(\"\")}",
                            controls: true,
                            style: "width:100%;max-height:220px;border-radius:8px;background:#000;",
                        }
                    }
                } else if let Some(ref url) = thumb {
                    // The poster is the whole point of an event listing — a
                    // time, a place and a face on a flyer. `object-fit:cover`
                    // in a 200px letterbox cropped the top and bottom off
                    // every portrait poster, which is exactly where that
                    // information sits.
                    //
                    // `height:auto` keeps the natural aspect ratio, so nothing
                    // is cut; `max-height` in viewport units stops a very tall
                    // flyer from pushing the booking button off-screen, and
                    // `contain` keeps the ratio when that cap bites.
                    img {
                        src: "{url}",
                        alt: "{title}",
                        style: "width:100%;height:auto;max-height:60vh;object-fit:contain;background:#0f0f1a;border-radius:8px;margin-top:12px;display:block;",
                    }
                }
                if has_gallery {
                    div { style: "margin-top:12px;",
                        div { style: "font-size:12px;color:#888;margin-bottom:6px;", "📷 {gallery_label}" }
                        {
                            let photos = valid_photos.clone();
                            let photo_count = photos.len();
                            let has_many = photo_count > 1;
                            rsx! {
                                // Gallery frames stay fixed-height so the
                                // prev/next controls do not jump between
                                // photos of different shapes; the photo itself
                                // is `contain`, so a portrait shot letterboxes
                                // against the dark background instead of
                                // losing its edges.
                                div { style: "position:relative;width:100%;height:260px;background:#0f0f1a;border-radius:8px;overflow:hidden;border:1px solid #2a2a4a;",
                                    {
                                        let idx = *selected_photo.read();
                                        if let Some(url) = photos.get(idx) {
                                            let u = url.clone();
                                            let alt = title.clone();
                                            rsx! { img { src: "{u}", alt: "{alt}", style: "width:100%;height:100%;object-fit:contain;" } }
                                        } else {
                                            rsx! {}
                                        }
                                    }
                                    if has_many {
                                        button {
                                            style: "position:absolute;left:4px;top:50%;transform:translateY(-50%);width:44px;height:44px;background:rgba(0,0,0,0.6);color:#e8e8e8;border:none;border-radius:50%;font-size:16px;cursor:pointer;display:flex;align-items:center;justify-content:center;",
                                            "aria-label": t(lang, T_EVENTS_PREV_PHOTO),
                                            onclick: move |_| {
                                                let next = selected_photo.read().saturating_sub(1);
                                                selected_photo.set(next);
                                            },
                                            "‹"
                                        }
                                        button {
                                            style: "position:absolute;right:4px;top:50%;transform:translateY(-50%);width:44px;height:44px;background:rgba(0,0,0,0.6);color:#e8e8e8;border:none;border-radius:50%;font-size:16px;cursor:pointer;display:flex;align-items:center;justify-content:center;",
                                            "aria-label": t(lang, T_EVENTS_NEXT_PHOTO),
                                            onclick: move |_| {
                                                let next = (*selected_photo.read() + 1).min(photo_count - 1);
                                                selected_photo.set(next);
                                            },
                                            "›"
                                        }
                                    }
                                }
                            }
                        }
                        if valid_photos.len() > 1 {
                            { let total = valid_photos.len(); let current = *selected_photo.read(); rsx! {
                                div { style: "display:flex;justify-content:center;gap:4px;margin-top:6px;",
                                    {
                                        (0..total).map(move |i| {
                                            let active = i == current;
                                            rsx! {
                                                button {
                                                    key: "{i}",
                                                    style: if active { "width:8px;height:8px;border-radius:50%;border:none;background:#39ff14;cursor:pointer;" } else { "width:8px;height:8px;border-radius:50%;border:none;background:#2a2a4a;cursor:pointer;" },
                                                    "aria-label": tf(lang, T_EVENTS_PHOTO_N, &[(i + 1).to_string()]),
                                                    onclick: move |_| selected_photo.set(i),
                                                }
                                            }
                                        })
                                    }
                                }
                            }}
                        }
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
                { seat_selector }
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
    let no_events = t(lang, T_EVENTS_NO_EVENTS).to_string();

    let mut events_resource: Resource<Result<Vec<CalendarEvent>, String>> =
        use_resource(use_reactive!(|selected| {
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
                let url = format!(
                    "{}/api/events?from={}T00:00:00%2B07:00&to={}T00:00:00%2B07:00",
                    base, from, to
                );
                match crate::ui::api::http::fetch_text_full(&url).await {
                    Ok((status, body)) if (200..300).contains(&status) => {
                        let parsed: serde_json::Value =
                            serde_json::from_str(&body).unwrap_or_default();
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
                    None => {
                        rsx! {
                            div { style: "display:flex;flex-direction:column;gap:12px;",
                                Skeleton { shape: SkeletonShape::Event }
                                Skeleton { shape: SkeletonShape::Event }
                                Skeleton { shape: SkeletonShape::Event }
                            }
                        }
                    },
                    Some(Err(e)) => {
                        let err_msg = e.clone();
                        let retry = t(lang, T_EVENTS_RETRY).to_string();
                        rsx! {
                            div { style: "text-align:center;padding:40px 0;",
                                p { style: "color:#ff4757;font-size:12px;", "{err_msg}" }
                                button {
                                    style: "margin-top:12px;padding:8px 16px;background:#1a1a2e;border:2px solid #39ff14;color:#39ff14;font-size:12px;cursor:pointer;",
                                    onclick: refresh,
                                    "↻ {retry}"
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
                    if let Some(e) = parsed
                        .get("event")
                        .and_then(|v| serde_json::from_value(v.clone()).ok())
                    {
                        event.set(Some(e));
                    } else {
                        error.set(Some(
                            t(crate::ui::lang::current_lang(), T_EVENTS_EVENT_NOT_FOUND)
                                .to_string(),
                        ));
                    }
                }
                Ok((status, _)) => error.set(Some(format!("HTTP {status}"))),
                Err(e) => error.set(Some(e.to_string())),
            }
            loading.set(false);
        });
    });

    let ev = event.read().clone();
    let go_back = move |_| {
        nav.push(Route::Events {});
    };
    rsx! {
        div { style: "min-height:100vh;background:#0f0f1a;color:#e8e8e8;padding-bottom:80px;",
            if *loading.read() {
                div { style: "padding:0 16px;",
                    Skeleton { shape: SkeletonShape::Sotd }
                    div { style: "margin-top:12px;", Skeleton { shape: SkeletonShape::Text, width: Some("60%".into()) } }
                    div { style: "margin-top:8px;", Skeleton { shape: SkeletonShape::TextSm, width: Some("80%".into()) } }
                }
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
    let my_bookings_title = t(lang, T_EVENTS_MY_BOOKINGS).to_string();
    let no_bookings = t(lang, T_EVENTS_NO_BOOKINGS).to_string();
    let cancel_label = t(lang, T_EVENTS_CANCEL).to_string();
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
            let _ = crate::ui::api::http::put_json_authed(&url, &init, "{}").await;
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
                h1 { style: "font-size:20px;font-weight:800;color:#39ff14;text-shadow:2px 2px 0 #000;", "{my_bookings_title}" }
            }
            if *loading.read() {
                div { style: "padding:0 16px;",
                    Skeleton { shape: SkeletonShape::Orders }
                    Skeleton { shape: SkeletonShape::Orders }
                    Skeleton { shape: SkeletonShape::Orders }
                }
            } else if let Some(err) = error.read().clone() {
                div { style: "text-align:center;padding:40px 0;color:#ff4757;font-size:12px;", "{err}" }
            } else if !has_bookings {
                div { style: "text-align:center;padding:40px 0;color:#888;font-size:12px;", "{no_bookings}" }
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
                                            "{cancel_label}"
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
