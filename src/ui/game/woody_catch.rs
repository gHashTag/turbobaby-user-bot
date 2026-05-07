// Woody Catch — Dioxus/WASM arcade game
//
// Port of WoodyCatchGame.tsx (React/TS) to Rust/Dioxus 0.6
// 4-lane catch game: buds fall, player moves left/right, catch = points
//
// Controls: ArrowLeft/Right or A/D keys, tap lane on mobile
// State managed via Dioxus Signals + gloo-timers game loop

use dioxus::prelude::*;
use gloo_timers::future::TimeoutFuture;
use wasm_bindgen::prelude::*;
use web_sys::window;

// ── Types ─────────────────────────────────────────────────────────────────────

#[derive(Clone, PartialEq, Debug)]
pub struct Bud {
    pub id: u32,
    pub lane: u8,
    pub y: f32,
    pub speed: f32,
    pub type_idx: u8,
    pub rot: f32,
}

#[derive(Clone, PartialEq, Debug)]
pub struct BudType {
    pub emoji: &'static str,
    pub pts: u32,
    pub color: &'static str,
    pub glow: &'static str,
}

const BUD_TYPES: [BudType; 4] = [
    BudType { emoji: "🌿", pts: 10, color: "#22c55e", glow: "rgba(34,197,94,0.6)" },
    BudType { emoji: "💜", pts: 20, color: "#a855f7", glow: "rgba(168,85,247,0.6)" },
    BudType { emoji: "⭐", pts: 30, color: "#fbbf24", glow: "rgba(251,191,36,0.6)" },
    BudType { emoji: "💎", pts: 50, color: "#06b6d4", glow: "rgba(6,182,212,0.8)" },
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

/// Pseudo-random u32 using js Math.random() under WASM
fn rand_u32() -> u32 {
    let v: f64 = js_sys::Math::random();
    (v * (u32::MAX as f64)) as u32
}

fn rand_f32() -> f32 {
    js_sys::Math::random() as f32
}

fn rand_lane() -> u8 {
    (rand_u32() % 4) as u8
}

fn rand_type(level: u32) -> u8 {
    let r = rand_f32();
    // Higher levels: slightly more rare drops
    if level > 5 {
        if r > 0.94 { 3 } else if r > 0.84 { 2 } else if r > 0.68 { 1 } else { 0 }
    } else {
        if r > 0.96 { 3 } else if r > 0.88 { 2 } else if r > 0.72 { 1 } else { 0 }
    }
}

/// Attempt to trigger Telegram WebApp haptic feedback
fn haptic_impact() {
    let _ = js_sys::eval(
        "try { Telegram.WebApp.HapticFeedback.impactOccurred('light'); } catch(e) {}"
    );
}

// ── Game component ────────────────────────────────────────────────────────────

#[component]
pub fn WoodyCatch() -> Element {
    // ── State signals ────────────────────────────────────────────────────────
    let mut score     = use_signal(|| 0u32);
    let mut lives     = use_signal(|| 3u32);
    let mut level     = use_signal(|| 1u32);
    let mut combo     = use_signal(|| 0u32);
    let mut playing   = use_signal(|| false);
    let mut game_over = use_signal(|| false);
    let mut lane      = use_signal(|| 1u32);  // 0-3
    let mut buds      = use_signal(|| Vec::<Bud>::new());
    let mut next_id   = use_signal(|| 0u32);
    let high_score    = use_signal(get_high_score);

    // Timing: track spawn countdown in ticks (each tick ~16ms)
    let mut spawn_ticks = use_signal(|| 0u32);

    // ── Game loop via use_future ─────────────────────────────────────────────
    {
        let mut score     = score.clone();
        let mut lives     = lives.clone();
        let mut level     = level.clone();
        let mut combo     = combo.clone();
        let mut playing   = playing.clone();
        let mut game_over = game_over.clone();
        let mut lane      = lane.clone();
        let mut buds      = buds.clone();
        let mut next_id   = next_id.clone();
        let mut high_score = high_score.clone();
        let mut spawn_ticks = spawn_ticks.clone();

        use_future(move || async move {
            loop {
                TimeoutFuture::new(16).await;  // ~60fps

                if !*playing.read() || *game_over.read() {
                    continue;
                }

                let cur_level  = *level.read();
                let cur_lane   = *lane.read() as u8;
                let cur_combo  = *combo.read();

                // ── Spawn logic ──────────────────────────────────────────────
                // spawn interval: max(900 - level*50, 280) ms → in ticks (÷16)
                let spawn_interval_ms = (900u32.saturating_sub(cur_level * 50)).max(280);
                let spawn_interval_ticks = spawn_interval_ms / 16;

                *spawn_ticks.write() += 1;
                if *spawn_ticks.read() >= spawn_interval_ticks {
                    *spawn_ticks.write() = 0;
                    let id = *next_id.read();
                    *next_id.write() = id + 1;
                    let speed = 0.8 + cur_level as f32 * 0.12 + rand_f32() * 0.2;
                    buds.write().push(Bud {
                        id,
                        lane: rand_lane(),
                        y: -5.0,
                        speed,
                        type_idx: rand_type(cur_level),
                        rot: rand_f32() * 360.0,
                    });

                    // Double-spawn at level > 3
                    if cur_level > 3 && rand_f32() > 0.6 {
                        let id2 = *next_id.read();
                        *next_id.write() = id2 + 1;
                        buds.write().push(Bud {
                            id: id2,
                            lane: rand_lane(),
                            y: -5.0,
                            speed: 0.8 + cur_level as f32 * 0.12,
                            type_idx: 0,
                            rot: 0.0,
                        });
                    }
                }

                // ── Move & collide ───────────────────────────────────────────
                let mut caught_pts: Option<(u32, &'static str)> = None;
                let mut missed = false;

                let updated: Vec<Bud> = buds.read().iter().filter_map(|b| {
                    let ny = b.y + b.speed;

                    // Catch zone: y in [72, 88] and same lane
                    if ny >= 72.0 && ny <= 88.0 && b.lane == cur_lane {
                        let bt = &BUD_TYPES[b.type_idx as usize];
                        let mult = (1.0 + (cur_combo / 5) as f32 * 0.5).min(3.0);
                        let earned = (bt.pts as f32 * mult) as u32;
                        caught_pts = Some((earned, bt.color));
                        return None; // remove bud
                    }

                    // Past bottom edge
                    if ny > 102.0 {
                        missed = true;
                        return None;
                    }

                    Some(Bud {
                        y: ny,
                        rot: b.rot + b.speed * 3.0,
                        ..b.clone()
                    })
                }).collect();

                *buds.write() = updated;

                if let Some((earned, _color)) = caught_pts {
                    haptic_impact();
                    *combo.write() += 1;
                    let new_score = *score.read() + earned;
                    *score.write() = new_score;
                    *level.write() = new_score / 100 + 1;
                    if new_score > *high_score.read() {
                        set_high_score(new_score);
                        *high_score.write() = new_score;
                    }
                }

                if missed {
                    *combo.write() = 0;
                    let new_lives = lives.read().saturating_sub(1);
                    *lives.write() = new_lives;
                    if new_lives == 0 {
                        *game_over.write() = true;
                        *playing.write() = false;
                    }
                }
            }
        });
    }

    // ── Keyboard handler ─────────────────────────────────────────────────────
    {
        let mut lane = lane.clone();
        let mut playing = playing.clone();
        let mut game_over = game_over.clone();
        use_effect(move || {
            let closure = Closure::<dyn FnMut(web_sys::KeyboardEvent)>::new(move |e: web_sys::KeyboardEvent| {
                if !*playing.read() || *game_over.read() { return; }
                let key = e.key();
                if key == "ArrowLeft" || key == "a" || key == "A" {
                    let cur = *lane.read();
                    if cur > 0 { *lane.write() = cur - 1; }
                }
                if key == "ArrowRight" || key == "d" || key == "D" {
                    let cur = *lane.read();
                    if cur < 3 { *lane.write() = cur + 1; }
                }
            });
            if let Some(win) = window() {
                let _ = win.add_event_listener_with_callback(
                    "keydown",
                    closure.as_ref().unchecked_ref(),
                );
            }
            closure.forget(); // Leak intentionally — runs for app lifetime
        });
    }

    // ── Start / Restart helper (Copy via use_callback so it can be reused) ───
    let start_game = use_callback(move |_: dioxus::prelude::Event<MouseData>| {
        let mut score       = score;
        let mut lives       = lives;
        let mut level       = level;
        let mut combo       = combo;
        let mut playing     = playing;
        let mut game_over   = game_over;
        let mut lane        = lane;
        let mut buds        = buds;
        let mut spawn_ticks = spawn_ticks;
        *score.write()       = 0;
        *lives.write()       = 3;
        *level.write()       = 1;
        *combo.write()       = 0;
        *playing.write()     = true;
        *game_over.write()   = false;
        *lane.write()        = 1;
        *spawn_ticks.write() = 0;
        buds.write().clear();
    });

    // ── Computed values for render ───────────────────────────────────────────
    let cur_score  = *score.read();
    let cur_lives  = *lives.read();
    let cur_level  = *level.read();
    let cur_combo  = *combo.read();
    let cur_lane   = *lane.read();
    let cur_hs     = *high_score.read();
    let is_playing = *playing.read();
    let is_over    = *game_over.read();
    let mult       = (1.0 + (cur_combo / 5) as f32 * 0.5).min(3.0);
    let buds_list  = buds.read().clone();

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

                // ── Lane columns ────────────────────────────────────────────
                for i in 0u32..4u32 {
                    {
                        let mut lane_sig = lane.clone();
                        let start_game_click = start_game;
                        let is_active_lane = cur_lane == i;
                        let border_style = if i < 3 { "1px solid rgba(255,255,255,0.03)" } else { "none" };
                        let bg_style = if is_active_lane {
                            "linear-gradient(180deg, transparent 0%, rgba(34,197,94,0.08) 60%, rgba(34,197,94,0.2) 80%, rgba(34,197,94,0.08) 100%)"
                        } else {
                            "transparent"
                        };
                        rsx! {
                            div {
                                key: "{i}",
                                style: "
                                    position: absolute; left: {i * 25}%; width: 25%; height: 100%;
                                    cursor: pointer; border-right: {border_style};
                                    background: {bg_style}; transition: background 0.15s;
                                ",
                                onclick: move |evt| {
                                    if is_playing && !is_over {
                                        *lane_sig.write() = i;
                                    } else {
                                        start_game_click.call(evt);
                                    }
                                },
                                // Tree decoration
                                div { style: "
                                    position: absolute; top: 28px; left: 50%;
                                    transform: translateX(-50%);
                                    font-size: 36px;
                                    filter: drop-shadow(0 4px 8px rgba(0,0,0,0.4));
                                ", "🌲" }
                            }
                        }
                    }
                }

                // ── Falling buds ─────────────────────────────────────────────
                for bud in buds_list.iter() {
                    {
                        let bt = &BUD_TYPES[bud.type_idx as usize];
                        let left_pct = (bud.lane as f32 + 0.5) * 25.0;
                        let top_pct  = bud.y;
                        let rot      = bud.rot;
                        let glow     = bt.glow;
                        let emoji    = bt.emoji;
                        rsx! {
                            div {
                                key: "{bud.id}",
                                style: "
                                    position: absolute;
                                    left: {left_pct}%;
                                    top: {top_pct}%;
                                    transform: translate(-50%, -50%) rotate({rot}deg);
                                    font-size: 28px;
                                    filter: drop-shadow(0 0 12px {glow});
                                    pointer-events: none;
                                    transition: transform 0.03s linear;
                                ",
                                "{emoji}"
                            }
                        }
                    }
                }

                // ── Catch zone line ──────────────────────────────────────────
                div { style: "
                    position: absolute; bottom: 68px; left: 0; right: 0; height: 2px;
                    background: linear-gradient(90deg, transparent, rgba(34,197,94,0.3), transparent);
                " }

                // ── Player (basket + bird) ───────────────────────────────────
                div { style: "
                    position: absolute; bottom: 28px;
                    left: {(cur_lane as f32 + 0.5) * 25.0}%;
                    transform: translateX(-50%);
                    transition: left 0.08s ease-out; z-index: 5;
                ",
                    div { style: "font-size: 44px; filter: drop-shadow(0 4px 12px rgba(0,0,0,0.5));", "🧺" }
                    div { style: "
                        position: absolute; top: -24px; left: 50%;
                        transform: translateX(-50%); font-size: 32px;
                    ", "🐦" }
                }

                // ── Mobile touch buttons ─────────────────────────────────────
                if is_playing && !is_over {
                    div { style: "
                        position: absolute; bottom: 8px; left: 0; right: 0;
                        display: flex; justify-content: space-between;
                        padding: 0 8px; z-index: 15; pointer-events: none;
                    ",
                        button {
                            style: "
                                font-size: 22px; background: rgba(255,255,255,0.08);
                                border: 2px solid rgba(255,255,255,0.15);
                                border-radius: 8px; padding: 6px 18px; cursor: pointer;
                                pointer-events: all; color: white;
                            ",
                            onclick: {
                                let mut lane = lane.clone();
                                move |_| {
                                    let cur = *lane.read();
                                    if cur > 0 { *lane.write() = cur - 1; }
                                }
                            },
                            "◀"
                        }
                        button {
                            style: "
                                font-size: 22px; background: rgba(255,255,255,0.08);
                                border: 2px solid rgba(255,255,255,0.15);
                                border-radius: 8px; padding: 6px 18px; cursor: pointer;
                                pointer-events: all; color: white;
                            ",
                            onclick: {
                                let mut lane = lane.clone();
                                move |_| {
                                    let cur = *lane.read();
                                    if cur < 3 { *lane.write() = cur + 1; }
                                }
                            },
                            "▶"
                        }
                    }
                }

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
        }
    }
}
