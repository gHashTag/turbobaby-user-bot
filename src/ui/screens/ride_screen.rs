//! TurboBaby Ride — an endless fleet-backed motorbike run.
//!
//! The screen is a thin shell: it owns the HUD and the payout, while the game
//! itself is `assets/game/ride.js`, loaded on demand together with three.js.
//! Keeping the renderer out of the wasm bundle is deliberate — that bundle is
//! downloaded by every customer who opens the shop, and most of them will
//! never start the game.
//!
//! The bridge is a `turbobaby:ride` CustomEvent rather than a wasm-bindgen
//! callback, matching how the Telegram contact bridge already works here.

use crate::trios::i18n::{tf, Key};
use crate::ui::api::context::api_base_url;
use crate::ui::components::bottom_nav::BottomNav;
use crate::ui::telegram::{use_telegram_id, use_telegram_init_data, TelegramApp};
use dioxus::prelude::*;

/// Copy lives here rather than in the shared i18n table because it is specific
/// to one screen and would otherwise bloat every other page's translation
/// lookup.
fn ru_en(lang: crate::trios::core::Lang, ru: &'static str, en: &'static str) -> &'static str {
    if lang == crate::trios::core::Lang::English {
        en
    } else {
        ru
    }
}

/// The game itself, with no page chrome. Sized to fill whatever it is dropped
/// into rather than the viewport, because it is embedded as a tab on the
/// garden screen — which owns the tab bar and the bottom nav — as well as
/// being reachable on its own route.
#[component]
pub fn RideGame() -> Element {
    let telegram_id = use_telegram_id();
    let init_data = use_telegram_init_data();
    let lang = crate::ui::lang::current_lang();

    let mut distance = use_signal(|| 0i64);
    let mut stars = use_signal(|| 0i64);
    let mut speed = use_signal(|| 0i64);
    let mut running = use_signal(|| false);
    let mut finished = use_signal(|| false);
    let mut payout_note = use_signal(String::new);
    let mut submitted = use_signal(|| false);

    // Submit the run once, when it ends: the score to the leaderboard and the
    // stars to the balance. The server caps the payout per day, so this cannot
    // mint currency even if the client lies about the count.
    let submit = use_callback(move |(dist, collected): (i64, i64)| {
        let Some(tid) = telegram_id else {
            payout_note.set(
                ru_en(
                    lang,
                    "Откройте в Telegram, чтобы сохранить результат",
                    "Open in Telegram to save your run",
                )
                .to_string(),
            );
            return;
        };
        let init = init_data.clone();
        spawn(async move {
            let base = api_base_url();
            let score_body =
                format!(r#"{{"telegram_id":{tid},"score":{dist},"display_name":"Player {tid}"}}"#);
            let _ = crate::ui::api::http::post_json_authed(
                &format!("{base}/api/game/high-scores"),
                &init,
                &score_body,
            )
            .await;

            if collected <= 0 {
                return;
            }
            // A fresh key per run: replaying it must not pay twice.
            let key = uuid::Uuid::new_v4().to_string();
            let stars_body = format!(
                r#"{{"telegram_id":{tid},"amount":{collected},"source":"ride","reason":"run_complete","external_tx_id":"{key}"}}"#
            );
            match crate::ui::api::http::post_json_authed(
                &format!("{base}/api/stars/add"),
                &init,
                &stars_body,
            )
            .await
            {
                Ok(body) => {
                    let capped = body.contains("\"capped\":true");
                    payout_note.set(if capped {
                        ru_en(
                            lang,
                            "Дневной лимит звёзд достигнут",
                            "Daily star limit reached",
                        )
                        .to_string()
                    } else {
                        tf(
                            lang,
                            if lang == crate::trios::core::Lang::English {
                                "+{0} ⭐ added to your balance" as Key
                            } else {
                                "+{0} ⭐ зачислено на баланс" as Key
                            },
                            &[collected.to_string()],
                        )
                    });
                }
                Err(_) => payout_note.set(
                    ru_en(
                        lang,
                        "Не удалось сохранить — нет связи",
                        "Could not save — no connection",
                    )
                    .to_string(),
                ),
            }
        });
    });

    // Listen for the game's events. The listener is owned by this component
    // and dropped with it, so mounting Ride twice cannot leave two payout
    // paths subscribed to the same CustomEvent.
    #[cfg(target_arch = "wasm32")]
    use_hook_with_cleanup(
        move || {
            use wasm_bindgen::JsCast;
            let win = web_sys::window()?;
            // Same hazard as the main button: raw JS, no Dioxus scope. See
            // `crate::ui::telegram::in_dioxus_scope`.
            let scope = current_scope_id().ok();
            let listener = gloo_events::EventListener::new(&win, "turbobaby:ride", move |event| {
                let detail = event
                    .dyn_ref::<web_sys::CustomEvent>()
                    .map(|e| e.detail())
                    .unwrap_or(wasm_bindgen::JsValue::NULL);
                let get_number = |k: &str| {
                    js_sys::Reflect::get(&detail, &wasm_bindgen::JsValue::from_str(k))
                        .ok()
                        .and_then(|v| v.as_f64())
                        .unwrap_or(0.0) as i64
                };
                let get_text = |k: &str| {
                    js_sys::Reflect::get(&detail, &wasm_bindgen::JsValue::from_str(k))
                        .ok()
                        .and_then(|v| v.as_string())
                        .unwrap_or_default()
                };
                let kind = get_text("kind");
                crate::ui::telegram::in_dioxus_scope(scope, "ride", || match kind.as_str() {
                    "tick" => {
                        distance.set(get_number("distance"));
                        stars.set(get_number("stars"));
                        speed.set(get_number("speed"));
                    }
                    "end" => {
                        let final_distance = get_number("distance");
                        let final_stars = get_number("stars");
                        distance.set(final_distance);
                        stars.set(final_stars);
                        running.set(false);
                        finished.set(true);
                        if !submitted() {
                            submitted.set(true);
                            submit.call((final_distance, final_stars));
                        }
                    }
                    "unavailable" => {
                        running.set(false);
                        finished.set(false);
                        let reason = get_text("reason");
                        payout_note.set(
                            if reason == "empty" {
                                ru_en(
                                    lang,
                                    "Сейчас нет свободных байков для заезда.",
                                    "No bike is available to ride right now.",
                                )
                            } else {
                                ru_en(
                                    lang,
                                    "Заезд сейчас недоступен — попробуйте ещё раз.",
                                    "The ride is unavailable — please try again.",
                                )
                            }
                            .to_string(),
                        );
                    }
                    _ => {}
                });
            });
            Some(std::rc::Rc::new(listener))
        },
        |_listener: Option<std::rc::Rc<gloo_events::EventListener>>| {},
    );

    // Boot the game module. The import is dynamic so three.js is fetched only
    // now, not as part of the shop's bundle.
    let boot = use_callback(move |_: ()| {
        running.set(true);
        finished.set(false);
        submitted.set(false);
        payout_note.set(String::new());
        distance.set(0);
        stars.set(0);
        #[cfg(target_arch = "wasm32")]
        {
            let strings = serde_json::json!({
                "loading": ru_en(
                    lang,
                    "Загружаем свободные байки...",
                    "Loading the bikes in stock...",
                ),
                "rosterUnavailable": ru_en(
                    lang,
                    "Список байков недоступен — сейчас не на чем ехать.",
                    "The bike list is unavailable, so there is nothing to ride yet.",
                ),
                "rosterEmpty": ru_en(
                    lang,
                    "Сейчас нет свободных байков для заезда.",
                    "No bike is available to ride right now.",
                ),
                "riding": ru_en(lang, "Едем на", "Riding"),
            })
            .to_string();
            let js = r#"(function(){
                var generation = (window.__rideGeneration || 0) + 1;
                window.__rideGeneration = generation;
                var host = document.getElementById('ride-host');
                if(!host) return;
                if (window.__rideStop) { try { window.__rideStop(); } catch(e) {} }
                window.__rideStop = null;
                host.innerHTML = '';
                import('/assets/game/ride.js').then(function(m){
                    if (window.__rideGeneration !== generation || !host.isConnected) return;
                    var stop = m.start(host, {
                        strings: __RIDE_STRINGS__,
                        onTick: function(s){
                            window.dispatchEvent(new CustomEvent('turbobaby:ride',
                                {detail: {kind:'tick', distance:s.distance, stars:s.stars, speed:s.speed}}));
                        },
                        onEnd: function(s){
                            window.dispatchEvent(new CustomEvent('turbobaby:ride',
                                {detail: {kind:'end', distance:s.distance, stars:s.stars}}));
                        },
                        onRoster: function(s){
                            if (!s || !s.reason) return;
                            window.dispatchEvent(new CustomEvent('turbobaby:ride',
                                {detail: {kind:'unavailable', reason:s.reason}}));
                        }
                    });
                    if (window.__rideGeneration !== generation || !host.isConnected) {
                        try { stop(); } catch(e) {}
                        return;
                    }
                    window.__rideStop = stop;
                }).catch(function(e){
                    if (window.__rideGeneration !== generation || !host.isConnected) return;
                    window.dispatchEvent(new CustomEvent('turbobaby:ride',
                        {detail: {kind:'unavailable', reason:'import'}}));
                    console.error('ride load failed', e);
                });
            })()"#
                .replace("__RIDE_STRINGS__", &strings);
            let _ = js_sys::eval(&js);
        }
    });

    // Stop the game when the screen goes away, or it keeps rendering — and
    // holding a GL context — behind the rest of the app.
    #[cfg(target_arch = "wasm32")]
    use_drop(move || {
        let _ = js_sys::eval(
            "window.__rideGeneration=(window.__rideGeneration||0)+1;if(window.__rideStop){try{window.__rideStop();}catch(e){}}window.__rideStop=null;",
        );
    });

    let title = "🏍️ TURBOBABY RIDE";
    let play = ru_en(lang, "▶ Поехали", "▶ Ride");
    let again = ru_en(lang, "↻ Ещё раз", "↻ Again");
    let task = ru_en(lang, "Задание: собери 10 звёзд", "Task: collect 10 stars");
    let hint = ru_en(
        lang,
        "Веди пальцем влево-вправо. Собирай звёзды, объезжай конусы.",
        "Drag left and right. Collect stars, dodge cones.",
    );

    rsx! {
        // `position: relative` matters: the canvas and every HUD layer below is
        // absolutely positioned against this box, so without it they would
        // escape the tab and cover the whole page.
        div { style: "position:relative;width:100%;height:calc(100vh - 152px - env(safe-area-inset-bottom));min-height:340px;background:#d8eef5;color:#123;overflow:hidden;",

            // The canvas lives here; the HUD floats above it.
            div { id: "ride-host", style: "position:absolute;inset:0;" }

            // ── HUD ──────────────────────────────────────────────────────
            if running() {
                div { style: "position:absolute;top:12px;left:14px;z-index:5;pointer-events:none;",
                    div { style: "display:flex;align-items:center;gap:6px;font-size:26px;font-weight:800;color:#fff;text-shadow:2px 2px 0 rgba(0,0,0,0.35);",
                        span { "⭐" }
                        span { "{stars}" }
                    }
                    div { style: "margin-top:8px;font-size:12px;font-weight:700;color:#fff;text-shadow:1px 1px 0 rgba(0,0,0,0.35);",
                        "{task}"
                    }
                    div { style: "font-size:12px;color:rgba(255,255,255,0.85);text-shadow:1px 1px 0 rgba(0,0,0,0.35);",
                        "{stars} / 10"
                    }
                }
                div { style: "position:absolute;top:12px;right:14px;z-index:5;pointer-events:none;background:rgba(0,0,0,0.25);border-radius:14px;padding:6px 12px;color:#fff;font-weight:700;font-size:14px;",
                    "{distance} м · {speed} GU"
                }
            }

            // ── Start / result card ──────────────────────────────────────
            if !running() {
                div { style: "position:absolute;inset:0;z-index:6;display:flex;flex-direction:column;align-items:center;justify-content:center;gap:14px;padding:0 24px;background:radial-gradient(circle at 50% 40%, rgba(12,20,26,0.35), rgba(8,12,18,0.82));",

                    // The mascot, so the game reads as Woody's before a single
                    // frame of it has been seen.
                    img {
                        src: "/assets/logo.jpg",
                        alt: "Woody",
                        style: "width:104px;height:104px;object-fit:cover;border:4px solid #39ff14;box-shadow:0 0 24px rgba(57,255,20,0.45), 4px 4px 0 #000;",
                    }
                    h1 { style: "font-size:26px;font-weight:800;color:#fff;text-shadow:3px 3px 0 rgba(0,0,0,0.4);letter-spacing:2px;margin:0;",
                        "{title}"
                    }
                    if finished() {
                        div { style: "text-align:center;color:#fff;background:rgba(0,0,0,0.35);border:3px solid rgba(57,255,20,0.5);padding:12px 22px;",
                            div { style: "font-size:38px;font-weight:800;line-height:1;", "{distance} м" }
                            div { style: "font-size:19px;margin-top:6px;", "⭐ {stars}" }
                            if !payout_note.read().is_empty() {
                                div { style: "font-size:13px;margin-top:8px;color:#ffe600;", "{payout_note}" }
                            }
                        }
                    } else {
                        p { style: "text-align:center;font-size:13px;color:rgba(255,255,255,0.9);max-width:280px;margin:0;line-height:1.5;",
                            "{hint}"
                        }
                        if !payout_note.read().is_empty() {
                            div { style: "font-size:13px;color:#ffe600;text-align:center;max-width:280px;", "{payout_note}" }
                        }
                    }
                    button {
                        style: "margin-top:6px;padding:14px 30px;background:#39ff14;color:#000;border:4px solid #2d9e0f;font-size:16px;font-weight:800;box-shadow:4px 4px 0 #000;cursor:pointer;",
                        onclick: move |_| {
                            boot.call(());
                        },
                        if finished() { "{again}" } else { "{play}" }
                    }
                }
            }

        }
    }
}

/// Standalone Ride route. The historical `/skate` URL is an alias in
/// `routes.rs`; there is only one renderer and one payout source.
#[component]
pub fn RideScreen() -> Element {
    let tg = TelegramApp::init();
    tg.show_back_button();

    rsx! {
        div { style: "min-height:100vh;background:#0f0f1a;padding-bottom:calc(96px + env(safe-area-inset-bottom));",
            RideGame {}
            BottomNav { cart_count: 0 }
        }
    }
}
