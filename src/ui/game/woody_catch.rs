// Woody Catch — Dioxus/WASM arcade game
//
// Port of WoodyCatchGame.tsx (React/TS) to Rust/Dioxus 0.6
// 4-lane catch game: buds fall, player moves left/right, catch = points
//
// Controls: ArrowLeft/Right or A/D keys, tap lane on mobile
// State managed via Dioxus Signals + gloo-timers game loop

use crate::trios::i18n::{t, T_CLOSE, T_GARDEN_GAME_HIGH_SCORES, T_GARDEN_GAME_NO_SCORES};
use crate::ui::api::context::api_base_url;
use crate::ui::telegram::{use_telegram_id, use_telegram_init_data};
use dioxus::prelude::*;
use wasm_bindgen::prelude::*;
use web_sys::window;

// ── Types ─────────────────────────────────────────────────────────────────────

#[derive(Clone, PartialEq, Debug)]
pub struct BudType {
    pub emoji: &'static str,
    pub pts: u32,
    pub color: &'static str,
    pub glow: &'static str,
}

const BUD_TYPES: [BudType; 4] = [
    BudType {
        emoji: "🌿",
        pts: 10,
        color: "#22c55e",
        glow: "rgba(34,197,94,0.6)",
    },
    BudType {
        emoji: "💜",
        pts: 20,
        color: "#a855f7",
        glow: "rgba(168,85,247,0.6)",
    },
    BudType {
        emoji: "⭐",
        pts: 30,
        color: "#fbbf24",
        glow: "rgba(251,191,36,0.6)",
    },
    BudType {
        emoji: "💎",
        pts: 50,
        color: "#06b6d4",
        glow: "rgba(6,182,212,0.8)",
    },
];

// ── Helpers ───────────────────────────────────────────────────────────────────

fn get_high_score() -> u32 {
    window()
        .and_then(|w| w.local_storage().ok().flatten())
        .and_then(|s| s.get_item("wcg_hs").ok().flatten())
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(0)
}

fn set_high_score(score: u32) {
    if let Some(storage) = window().and_then(|w| w.local_storage().ok().flatten()) {
        let _ = storage.set_item("wcg_hs", &score.to_string());
    }
}

// ── Global leaderboard helpers ────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Deserialize)]
struct GlobalHighScoreEntry {
    rank: i64,
    display_name: String,
    high_score: i64,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct GlobalHighScoresResponse {
    entries: Vec<GlobalHighScoreEntry>,
    #[serde(default)]
    total: i64,
}

async fn fetch_global_high_scores(limit: u32) -> Result<GlobalHighScoresResponse, String> {
    let base = api_base_url();
    let url = format!("{}/api/game/high-scores?limit={}", base, limit);
    let text = crate::ui::api::http::fetch_text(&url).await?;
    serde_json::from_str::<GlobalHighScoresResponse>(&text).map_err(|e| format!("Parse error: {e}"))
}

async fn submit_global_high_score(
    telegram_id: i64,
    init_data: &str,
    score: u64,
) -> Result<(i64, i64), String> {
    let base = api_base_url();
    let url = format!("{}/api/game/high-scores", base);
    let body = serde_json::json!({
        "telegram_id": telegram_id,
        "score": score as i64,
        "display_name": format!("Player {}", telegram_id),
    })
    .to_string();
    let text = crate::ui::api::http::post_json_authed(&url, init_data, &body).await?;
    let resp: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("Parse error: {e}"))?;
    if !resp
        .get("success")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
    {
        return Err("submit_failed".into());
    }
    let rank = resp.get("rank").and_then(|v| v.as_i64()).unwrap_or(0);
    let high_score = resp.get("high_score").and_then(|v| v.as_i64()).unwrap_or(0);
    Ok((rank, high_score))
}

// ── Game component ────────────────────────────────────────────────────────────

#[component]
pub fn WoodyCatch() -> Element {
    // ── State signals ────────────────────────────────────────────────────────
    let score = use_signal(|| 0u32);
    let lives = use_signal(|| 3u32);
    let level = use_signal(|| 1u32);
    let combo = use_signal(|| 0u32);
    let playing = use_signal(|| false);
    let game_over = use_signal(|| false);
    let high_score = use_signal(get_high_score);

    // Loop #18: global high-score integration.
    let mut show_scores = use_signal(|| false);
    let score_submitted = use_signal(|| false);
    let user_rank = use_signal(|| None::<i64>);
    let telegram_id = use_telegram_id().unwrap_or(0);
    let init_data = use_telegram_init_data();

    // ── Bridge to the 3D game ────────────────────────────────────────────────
    // The game itself is assets/game/catch.js on the shared three.js engine.
    // This component keeps only what a canvas cannot own: the HUD, the
    // leaderboard, and the score submission. Events come back over a
    // `woody:catch` CustomEvent, matching how the skate game and the Telegram
    // contact bridge already talk to Rust.
    #[cfg(target_arch = "wasm32")]
    {
        let mut score = score;
        let mut lives = lives;
        let mut level = level;
        let mut combo = combo;
        let mut playing = playing;
        let mut game_over = game_over;
        let mut high_score = high_score;
        use_hook(move || {
            let Some(win) = window() else { return };
            let listener = gloo_events::EventListener::new(&win, "woody:catch", move |event| {
                let detail = event
                    .dyn_ref::<web_sys::CustomEvent>()
                    .map(|e| e.detail())
                    .unwrap_or(wasm_bindgen::JsValue::NULL);
                let num = |k: &str| {
                    js_sys::Reflect::get(&detail, &wasm_bindgen::JsValue::from_str(k))
                        .ok()
                        .and_then(|v| v.as_f64())
                        .unwrap_or(0.0) as u32
                };
                let kind = js_sys::Reflect::get(&detail, &wasm_bindgen::JsValue::from_str("kind"))
                    .ok()
                    .and_then(|v| v.as_string())
                    .unwrap_or_default();
                match kind.as_str() {
                    "tick" => {
                        score.set(num("score"));
                        lives.set(num("lives"));
                        level.set(num("level"));
                        combo.set(num("combo"));
                    }
                    "end" => {
                        let final_score = num("score");
                        score.set(final_score);
                        level.set(num("level"));
                        playing.set(false);
                        game_over.set(true);
                        if final_score > *high_score.read() {
                            high_score.set(final_score);
                            set_high_score(final_score);
                        }
                    }
                    _ => {}
                }
            });
            listener.forget();
        });
    }

    // Stop the game when the tab goes away, or it keeps rendering — and holding
    // a GL context — behind the rest of the app.
    #[cfg(target_arch = "wasm32")]
    use_drop(move || {
        let _ = js_sys::eval(
            "if(window.__catchStop){try{window.__catchStop();}catch(e){}window.__catchStop=null;}",
        );
    });

    // ── Start / Restart ──────────────────────────────────────────────────────
    let start_game = use_callback(move |_: dioxus::prelude::Event<MouseData>| {
        let mut score = score;
        let mut lives = lives;
        let mut level = level;
        let mut combo = combo;
        let mut playing = playing;
        let mut game_over = game_over;
        let mut score_submitted = score_submitted;
        let mut user_rank = user_rank;
        *score.write() = 0;
        *lives.write() = 3;
        *level.write() = 1;
        *combo.write() = 0;
        *playing.write() = true;
        *game_over.write() = false;
        score_submitted.set(false);
        user_rank.set(None);

        // Dynamic import: three.js is fetched now, not as part of the shop's
        // wasm bundle that every customer downloads.
        #[cfg(target_arch = "wasm32")]
        {
            let js = r#"(function(){
                var host = document.getElementById('catch-host');
                if(!host) return;
                if (window.__catchStop) { try { window.__catchStop(); } catch(e) {} }
                host.innerHTML = '';
                import('/assets/game/catch.js').then(function(m){
                    window.__catchStop = m.start(host, {
                        onTick: function(s){
                            window.dispatchEvent(new CustomEvent('woody:catch',
                                {detail: {kind:'tick', score:s.score, lives:s.lives, level:s.level, combo:s.combo}}));
                        },
                        onEnd: function(s){
                            window.dispatchEvent(new CustomEvent('woody:catch',
                                {detail: {kind:'end', score:s.score, level:s.level}}));
                        }
                    });
                }).catch(function(e){
                    window.dispatchEvent(new CustomEvent('woody:catch',
                        {detail: {kind:'end', score:0, level:1}}));
                    console.error('catch load failed', e);
                });
            })()"#;
            let _ = js_sys::eval(js);
        }
    });

    // Loop #18: submit final score to the global leaderboard once per game-over.
    {
        let mut submitted = score_submitted;
        let mut rank = user_rank;
        let init = init_data.clone();
        let tid = telegram_id;
        use_effect(move || {
            let over = *game_over.read();
            let score = *score.read();
            let already = *submitted.read();
            if over && score > 0 && tid != 0 && !already {
                submitted.set(true);
                let init = init.clone();
                spawn(async move {
                    if let Ok((r, _)) = submit_global_high_score(tid, &init, score as u64).await {
                        rank.set(Some(r));
                    }
                });
            }
        });
    }

    // ── Computed values for render ───────────────────────────────────────────
    let cur_score = *score.read();
    let cur_lives = *lives.read();
    let cur_level = *level.read();
    let cur_combo = *combo.read();
    let cur_hs = *high_score.read();
    let is_playing = *playing.read();
    let is_over = *game_over.read();
    let mult = (1.0 + (cur_combo / 5) as f32 * 0.5).min(3.0);

    let lang = crate::ui::lang::current_lang();

    // ── Render ───────────────────────────────────────────────────────────────
    rsx! {
        div {
            style: "
                width: 100%; max-width: 400px; margin: 0 auto;
                border-radius: 24px;
                background: linear-gradient(145deg, #0c1222 0%, #1a0a2e 50%, #0c1222 100%);
                box-shadow: 0 0 60px rgba(139,92,246,0.15), inset 0 0 80px rgba(0,0,0,0.5);
                padding: 16px; position: relative; overflow: hidden;
                font-family: system-ui, -apple-system, sans-serif;
                min-height: 100vh;
                /* Rapid taps must not trigger double-tap zoom / text selection. */
                touch-action: manipulation;
                -webkit-user-select: none; user-select: none;
                -webkit-tap-highlight-color: transparent;
            ",

            // ── Ambient glow ────────────────────────────────────────────────
            div { style: "
                position: absolute; top: 0; left: 50%; transform: translateX(-50%);
                width: 200px; height: 200px;
                background: radial-gradient(circle, rgba(34,197,94,0.1) 0%, transparent 70%);
                pointer-events: none;
            " }

            // ── HUD ─────────────────────────────────────────────────────────
            div { style: "
                display: flex; justify-content: space-between; align-items: center;
                margin-bottom: 12px; position: relative; z-index: 10;
            ",
                // Score
                div { style: "text-align: left;",
                    div { style: "
                        font-size: 28px; font-weight: 800; color: #fff;
                        text-shadow: 0 0 20px rgba(34,197,94,0.5);
                    ", "{cur_score}" }
                    div { style: "font-size: 16px; color: rgba(255,255,255,0.4); letter-spacing: 2px;", "SCORE" }
                }
                // Level + Combo
                div { style: "text-align: center;",
                    div { style: "
                        font-size: 12px; font-weight: 700; color: #a78bfa;
                        background: rgba(139,92,246,0.15); padding: 4px 14px;
                        border-radius: 20px; border: 2px solid rgba(139,92,246,0.3);
                    ", "LV.{cur_level}" }
                    if cur_combo >= 3 {
                        div { style: "
                            margin-top: 4px; font-size: 11px; font-weight: 700;
                            color: #f59e0b; text-shadow: 0 0 10px rgba(245,158,11,0.5);
                        ",
                            "{cur_combo}x "
                            span { style: "font-size: 9px; color: #fb923c;",
                                "×{mult:.1}"
                            }
                        }
                    }
                }
                // Lives
                div { style: "display: flex; gap: 3px;",
                    for i in 0u32..3u32 {
                        span {
                            key: "{i}",
                            style: if i < cur_lives {
                                "font-size: 16px; transition: all 0.2s;"
                            } else {
                                "font-size: 16px; filter: grayscale(1) opacity(0.3); transition: all 0.2s;"
                            },
                            if i < cur_lives { "❤️" } else { "🖤" }
                        }
                    }
                }
            }

            // ── Game field ──────────────────────────────────────────────────
            div { style: "
                position: relative; height: 480px; border-radius: 16px; overflow: hidden;
                background: linear-gradient(180deg, #0f2027 0%, #203a43 40%, #2c5364 100%);
                box-shadow: inset 0 0 40px rgba(0,0,0,0.5), 0 0 0 1px rgba(255,255,255,0.05);
            ",

                // ── 3D canvas ───────────────────────────────────────────────
                // Everything that used to be absolutely-positioned emoji —
                // lanes, falling drops, the basket, the lane arrows — is now
                // drawn by assets/game/catch.js. `position: relative` on the
                // parent keeps the canvas inside this card.
                div { id: "catch-host", style: "position:absolute;inset:0;" }


                // ── Start overlay ────────────────────────────────────────────
                if !is_playing && !is_over {
                    div { style: "
                        position: absolute; inset: 0;
                        background: rgba(0,0,0,0.92); backdrop-filter: blur(8px);
                        display: flex; align-items: center; justify-content: center; z-index: 20;
                    ",
                        onclick: move |e| start_game.call(e),
                        div { style: "text-align: center; color: #fff; padding: 24px;",
                            div { style: "font-size: 42px; margin-bottom: 8px;", "🌿🐦" }
                            h1 { style: "
                                font-size: 22px; font-weight: 800; margin-bottom: 8px;
                                background: linear-gradient(135deg, #22c55e, #a78bfa);
                                -webkit-background-clip: text; -webkit-text-fill-color: transparent;
                                background-clip: text;
                            ", "WOODY CATCH" }
                            p { style: "color: rgba(255,255,255,0.5); font-size: 12px; margin-bottom: 16px;",
                                "Лови падающие шишки!"
                            }
                            // Bud type legend
                            div { style: "display: flex; justify-content: center; gap: 12px; margin-bottom: 16px;",
                                for (i, bt) in BUD_TYPES.iter().enumerate() {
                                    div {
                                        key: "{i}",
                                        style: "display: flex; align-items: center; gap: 4px; font-size: 12px;",
                                        span { style: "filter: drop-shadow(0 0 6px {bt.glow});", "{bt.emoji}" }
                                        span { style: "color: {bt.color}; font-weight: 600;", "{bt.pts}" }
                                    }
                                }
                            }
                            if cur_hs > 0 {
                                div { style: "color: #fbbf24; font-size: 11px; margin-bottom: 16px;",
                                    "🏆 Рекорд: {cur_hs}"
                                }
                            }
                            button { style: "
                                background: linear-gradient(135deg, #22c55e, #16a34a); border: none;
                                padding: 14px 36px; font-size: 12px; font-weight: 700;
                                color: #fff; border-radius: 30px; cursor: pointer;
                                box-shadow: 0 8px 24px rgba(34,197,94,0.4);
                            ",
                                "▶ ИГРАТЬ"
                            }
                            button {
                                style: "margin-top:10px;padding:8px 16px;background:transparent;border:2px solid rgba(255,255,255,0.2);border-radius:20px;color:#fff;font-size:11px;font-weight:700;cursor:pointer;",
                                onclick: move |_| show_scores.set(true),
                                "🏆 {t(lang, T_GARDEN_GAME_HIGH_SCORES)}"
                            }
                            p { style: "color: rgba(255,255,255,0.3); font-size: 9px; margin-top: 12px;",
                                "Тап по дорожке или ← →"
                            }
                        }
                    }
                }

                // ── Game over overlay ────────────────────────────────────────
                if is_over {
                    div { style: "
                        position: absolute; inset: 0;
                        background: rgba(0,0,0,0.92); backdrop-filter: blur(8px);
                        display: flex; align-items: center; justify-content: center; z-index: 20;
                    ",
                        div { style: "text-align: center; color: #fff; padding: 24px;",
                            h1 { style: "font-size: 24px; font-weight: 800; color: #ef4444; margin-bottom: 8px;",
                                "💨 GAME OVER"
                            }
                            div { style: "
                                font-size: 48px; font-weight: 800; color: #fff;
                                text-shadow: 0 0 30px rgba(34,197,94,0.5); margin: 16px 0;
                            ", "{cur_score}" }
                            p { style: "color: rgba(255,255,255,0.5); margin-bottom: 8px;",
                                "Уровень {cur_level}"
                            }
                            if cur_score >= cur_hs && cur_score > 0 {
                                div { style: "color: #fbbf24; font-size: 12px; margin-bottom: 8px;",
                                    "🎉 НОВЫЙ РЕКОРД!"
                                }
                            }
                            div { style: "color: #fbbf24; font-size: 11px; margin-bottom: 16px;",
                                "🏆 {cur_hs.max(cur_score)}"
                            }
                            button {
                                style: "
                                    background: linear-gradient(135deg, #22c55e, #16a34a); border: none;
                                    padding: 14px 36px; font-size: 12px; font-weight: 700;
                                    color: #fff; border-radius: 30px; cursor: pointer;
                                    box-shadow: 0 8px 24px rgba(34,197,94,0.4);
                                ",
                                onclick: move |e| start_game.call(e),
                                "🔄 ЕЩЁ РАЗ"
                            }
                            button {
                                style: "margin-top:10px;padding:8px 16px;background:transparent;border:2px solid rgba(255,255,255,0.2);border-radius:20px;color:#fff;font-size:11px;font-weight:700;cursor:pointer;",
                                onclick: move |_| show_scores.set(true),
                                "🏆 {t(lang, T_GARDEN_GAME_HIGH_SCORES)}"
                            }
                        }
                    }
                }
            } // game field

            // ── High score footer ────────────────────────────────────────────
            div { style: "
                margin-top: 12px; text-align: center;
                font-size: 9px; color: rgba(255,255,255,0.3);
            ",
                "🏆 Рекорд: {cur_hs}  •  ← → или тап по дорожке"
            }

            // Loop #18: global leaderboard modal.
            if show_scores() {
                HighScoresModal { open: show_scores, lang }
            }
        }
    }
}

/// Loop #18: public Woody Catch leaderboard overlay.
#[component]
fn HighScoresModal(open: Signal<bool>, lang: crate::trios::Lang) -> Element {
    let data = use_resource(|| async move { fetch_global_high_scores(20).await });

    rsx! {
        div {
            style: "position:fixed;inset:0;z-index:1001;background:rgba(0,0,0,0.92);display:flex;align-items:center;justify-content:center;padding:16px;",
            onclick: move |_| open.set(false),
            div {
                style: "background:#1a1a2e;max-width:380px;width:100%;max-height:80vh;overflow:auto;border:4px solid #22c55e;box-shadow:4px 4px 0 #000;padding:20px;border-radius:12px;text-align:center;",
                onclick: move |e: Event<MouseData>| e.stop_propagation(),
                div { style: "font-size:36px;margin-bottom:12px;", "🏆" }
                div { style: "font-size:15px;font-weight:700;color:#22c55e;margin-bottom:16px;", "{t(lang, T_GARDEN_GAME_HIGH_SCORES)}" }
                {
                    match &*data.read() {
                        Some(Ok(resp)) if !resp.entries.is_empty() => {
                            let entries = resp.entries.clone();
                            rsx! {
                                div { style: "display:flex;flex-direction:column;gap:8px;margin-bottom:16px;",
                                    for e in entries {
                                        div { style: "display:flex;align-items:center;gap:10px;background:#0f0f1a;border:2px solid #2a2a4a;border-radius:10px;padding:10px 12px;",
                                            div { style: "font-size:13px;font-weight:700;color:#8b8b9e;min-width:30px;text-align:center;", "#{e.rank}" }
                                            div { style: "flex:1;text-align:left;font-size:13px;font-weight:700;color:#e8e8e8;", "{e.display_name}" }
                                            div { style: "font-size:13px;font-weight:700;color:#fbbf24;", "{e.high_score}" }
                                        }
                                    }
                                }
                            }
                        }
                        Some(Ok(_)) => rsx! {
                            div { style: "color:#8b8b9e;font-size:12px;margin-bottom:16px;", "{t(lang, T_GARDEN_GAME_NO_SCORES)}" }
                        },
                        Some(Err(_)) => rsx! {
                            div { style: "color:#ff6b7a;font-size:12px;margin-bottom:16px;", "Load failed" }
                        },
                        None => rsx! {
                            div { style: "color:#8b8b9e;font-size:12px;margin-bottom:16px;", "Loading..." }
                        },
                    }
                }
                button {
                    style: "padding:10px 20px;background:transparent;color:#e8e8e8;border:2px solid #2a2a4a;border-radius:8px;font-size:12px;font-weight:700;cursor:pointer;",
                    onclick: move |_| open.set(false),
                    "{t(lang, T_CLOSE)}"
                }
            }
        }
    }
}
