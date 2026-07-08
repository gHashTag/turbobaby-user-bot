// Woody Shop v0.1 — single-screen mini-app game prototype.
//
// Game loop: customers spawn at free tables, order a random product, Woody
// walks over and serves them, earns coins, then cleans the dirty table.
// Pure emoji/CSS visuals inside the existing Dioxus/WASM Telegram Mini App.

use dioxus::prelude::*;
use gloo_timers::future::TimeoutFuture;

// ── Domain types ───────────────────────────────────────────────────────────────

const TABLE_COUNT: usize = 3;
const SPAWN_INTERVAL_MS: u32 = 6000;
const SERVE_REWARD: u32 = 10;
const WALK_MS: u32 = 400;
const PREPARE_MS: u32 = 2000;
const EAT_MS: u32 = 2500;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Order {
    Weed,
    Coffee,
    Snack,
}

impl Order {
    pub fn emoji(self) -> &'static str {
        match self {
            Order::Weed => "🌿",
            Order::Coffee => "☕",
            Order::Snack => "🍪",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TableState {
    Empty,
    Seated { progress: u8 },          // customer waiting for order to be taken
    Preparing { order: Order },         // Woody accepted order, product is being made
    Ready { order: Order },             // product ready, waiting for Woody to serve
    Eating,                             // customer eats after being served
    Dirty,                              // table needs cleaning
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WoodyAction {
    Idle,
    WalkingTo(usize),
    TakingOrder(usize),
    PreparingAt(usize, Order),
    Serving(usize, Order),
    Cleaning(usize),
}

#[derive(Clone, Debug, PartialEq)]
pub struct ShopState {
    pub coins: u32,
    pub served: u32,
    pub woody_table: usize, // which table Woody is standing at (0..TABLE_COUNT)
    pub woody_action: WoodyAction,
    pub tables: [TableState; TABLE_COUNT],
}

impl ShopState {
    pub fn new() -> Self {
        Self {
            coins: 0,
            served: 0,
            woody_table: 1,
            woody_action: WoodyAction::Idle,
            tables: [TableState::Empty; TABLE_COUNT],
        }
    }

    fn first_empty_table(&self) -> Option<usize> {
        self.tables.iter().position(|s| *s == TableState::Empty)
    }

    fn is_busy(&self) -> bool {
        !matches!(self.woody_action, WoodyAction::Idle)
    }
}

// ── Utilities ─────────────────────────────────────────────────────────────────

fn rand_u32() -> u32 {
    let v: f64 = js_sys::Math::random();
    (v * (u32::MAX as f64)) as u32
}

fn rand_order() -> Order {
    match rand_u32() % 3 {
        0 => Order::Coffee,
        1 => Order::Snack,
        _ => Order::Weed,
    }
}

fn haptic_light() {
    let _ = js_sys::eval("try { Telegram.WebApp.HapticFeedback.impactOccurred('light'); } catch(e) {}");
}

fn haptic_success() {
    let _ = js_sys::eval("try { Telegram.WebApp.HapticFeedback.notificationOccurred('success'); } catch(e) {}");
}

// ── Game component ─────────────────────────────────────────────────────────────

#[component]
pub fn WoodyShop() -> Element {
    let state = use_signal(ShopState::new);
    let logs = use_signal(|| Vec::<String>::new());

    // Spawn customers loop.
    {
        let mut state = state;
        let mut logs = logs;
        use_future(move || async move {
            loop {
                TimeoutFuture::new(SPAWN_INTERVAL_MS).await;
                state.with_mut(|s| {
                    if let Some(idx) = s.first_empty_table() {
                        s.tables[idx] = TableState::Seated { progress: 0 };
                    }
                });
                logs.with_mut(|l| {
                    if l.len() > 5 { l.remove(0); }
                    l.push("New customer arrived".into());
                });
            }
        });
    }

    // Derived read-only values for render (snapshots).
    let coins = state.read().coins;
    let served = state.read().served;
    let woody_table = state.read().woody_table;
    let tables = state.read().tables.clone();

    // Click a table: Woody walks there, then the context-sensitive action fires.
    let on_table_click = move |idx: usize| {
        if state.read().is_busy() {
            return;
        }
        let mut state = state;
        let mut logs = logs;
        let current_table = state.read().woody_table;
        if current_table == idx {
            // Already there: perform immediate action.
            perform_action(&mut state, idx, &mut logs);
            return;
        }
        // Walk there first.
        state.with_mut(|s| s.woody_action = WoodyAction::WalkingTo(idx));
        spawn(async move {
            TimeoutFuture::new(WALK_MS).await;
            state.with_mut(|s| {
                s.woody_table = idx;
                s.woody_action = WoodyAction::Idle;
            });
            logs.with_mut(|l| {
                if l.len() > 5 { l.remove(0); }
                l.push(format!("Woody moved to table {}", idx + 1));
            });
        });
    };

    rsx! {
        div {
            style: "
                width: 100%; max-width: 420px; margin: 0 auto;
                min-height: 100vh; display: flex; flex-direction: column;
                background: linear-gradient(180deg, #16213e 0%, #0f0f1a 100%);
                color: #e8e8e8; font-family: system-ui, -apple-system, sans-serif;
                padding: 12px 12px 100px; box-sizing: border-box;
                touch-action: manipulation; user-select: none;
                -webkit-tap-highlight-color: transparent;
            ",

            // HUD
            div {
                style: "
                    display: flex; justify-content: space-between; align-items: center;
                    background: rgba(0,0,0,0.35); border: 2px solid #2a2a4a;
                    border-radius: 12px; padding: 10px 14px; margin-bottom: 14px;
                ",
                div {
                    style: "display: flex; align-items: center; gap: 6px;",
                    span { style: "font-size: 22px;", "🪙" }
                    span { style: "font-size: 20px; font-weight: 800; color: #ffe600;", "{coins}" }
                }
                div {
                    style: "font-size: 13px; color: #888;",
                    "Served: "
                    span { style: "color: #39ff14; font-weight: 700;", "{served}" }
                }
            }

            // Shop floor
            div {
                style: "
                    position: relative; flex: 1;
                    background: #1a1a2e; border: 3px solid #2a2a4a;
                    border-radius: 16px; overflow: hidden; padding: 16px;
                    display: flex; flex-direction: column; justify-content: flex-end; gap: 16px;
                    box-shadow: inset 0 0 40px rgba(0,0,0,0.5);
                ",

                // Decorative shop title
                div {
                    style: "
                        position: absolute; top: 10px; left: 50%; transform: translateX(-50%);
                        font-size: 13px; font-weight: 700; letter-spacing: 2px;
                        color: #39ff14; text-shadow: 0 0 10px rgba(57,255,20,0.4);
                    ",
                    "WOODY SHOP v0.1"
                }

                // Tables
                for (idx, table) in tables.iter().enumerate() {
                    TableRow {
                        key: "{idx}",
                        idx,
                        table: *table,
                        woody_here: woody_table == idx,
                        on_click: on_table_click.clone(),
                    }
                }
            }

            // Action bar
            div {
                style: "
                    display: flex; gap: 8px; margin-top: 12px;
                    background: rgba(0,0,0,0.35); border: 2px solid #2a2a4a;
                    border-radius: 12px; padding: 10px;
                ",
                ActionButton {
                    label: "👋 Take order",
                    active: matches!(tables[woody_table], TableState::Seated { .. }),
                    color: "#39ff14",
                    on_click: {
                        let mut state = state;
                        let mut logs = logs;
                        move |_| {
                            if state.read().is_busy() { return; }
                            perform_action(&mut state, woody_table, &mut logs);
                        }
                    },
                }
                ActionButton {
                    label: "🤲 Serve",
                    active: matches!(tables[woody_table], TableState::Ready { .. }),
                    color: "#00e5ff",
                    on_click: {
                        let mut state = state;
                        let mut logs = logs;
                        move |_| {
                            if state.read().is_busy() { return; }
                            perform_action(&mut state, woody_table, &mut logs);
                        }
                    },
                }
                ActionButton {
                    label: "🧽 Clean",
                    active: matches!(tables[woody_table], TableState::Dirty),
                    color: "#ff4757",
                    on_click: {
                        let mut state = state;
                        let mut logs = logs;
                        move |_| {
                            if state.read().is_busy() { return; }
                            perform_action(&mut state, woody_table, &mut logs);
                        }
                    },
                }
            }

            // Mini log
            div {
                style: "
                    margin-top: 10px; min-height: 60px;
                    font-size: 12px; color: #888; line-height: 1.45;
                    display: flex; flex-direction: column; gap: 2px;
                ",
                for (i, msg) in logs.read().iter().rev().enumerate().take(3) {
                    div { key: "{i}", "• {msg}" }
                }
            }
        }
    }
}

// ── Table row component ────────────────────────────────────────────────────────

#[component]
fn TableRow(
    idx: usize,
    table: TableState,
    woody_here: bool,
    on_click: EventHandler<usize>,
) -> Element {
    let customer_emoji = if matches!(table, TableState::Empty) { "" } else { "🧑‍🦱" };
    let order_bubble = match table {
        TableState::Seated { .. } => Some(rand_order().emoji()),
        TableState::Ready { order } => Some(order.emoji()),
        _ => None,
    };
    let table_emoji = match table {
        TableState::Dirty => "🍽️",
        _ => "🪑",
    };
    let busy_dot = match table {
        TableState::Preparing { .. } => Some("⚙️"),
        TableState::Eating => Some("😋"),
        _ => None,
    };

    rsx! {
        div {
            style: "
                display: flex; align-items: center; gap: 12px;
                background: rgba(255,255,255,0.04);
                border: 2px solid {border_color(woody_here)};
                border-radius: 14px; padding: 10px 14px;
                cursor: pointer; transition: border-color 0.2s;
            ",
            onclick: move |_| on_click.call(idx),

            div {
                style: "position: relative; font-size: 42px;",
                "{table_emoji}"
                if !customer_emoji.is_empty() {
                    div {
                        style: "
                            position: absolute; top: -10px; right: -10px;
                            font-size: 28px; filter: drop-shadow(0 2px 4px rgba(0,0,0,0.6));
                        ",
                        "{customer_emoji}"
                    }
                }
                if let Some(dot) = busy_dot {
                    div {
                        style: "
                            position: absolute; bottom: -6px; right: -6px;
                            font-size: 18px;
                        ",
                        "{dot}"
                    }
                }
            }

            div {
                style: "flex: 1; font-size: 14px; font-weight: 600; color: {table_color(table)};",
                "{table_label(table)}"
            }

            if let Some(bubble) = order_bubble {
                div {
                    style: "
                        width: 44px; height: 44px; border-radius: 50%;
                        background: #2a2a4a; border: 2px solid #ffe600;
                        display: flex; align-items: center; justify-content: center;
                        font-size: 22px; animation: pulse 1.2s infinite;
                    ",
                    "{bubble}"
                }
            }

            if woody_here {
                div {
                    style: "
                        font-size: 34px; filter: drop-shadow(0 3px 6px rgba(0,0,0,0.5));
                        transform: scaleX(-1);
                    ",
                    "🦉"
                }
            }
        }
    }
}

fn table_label(table: TableState) -> &'static str {
    match table {
        TableState::Empty => "Free table",
        TableState::Seated { .. } => "Customer waiting",
        TableState::Preparing { .. } => "Preparing...",
        TableState::Ready { .. } => "Order ready",
        TableState::Eating => "Customer eating",
        TableState::Dirty => "Dirty table",
    }
}

fn table_color(table: TableState) -> &'static str {
    match table {
        TableState::Empty => "#666",
        TableState::Seated { .. } => "#39ff14",
        TableState::Preparing { .. } => "#ff9d00",
        TableState::Ready { .. } => "#00e5ff",
        TableState::Eating => "#ffe600",
        TableState::Dirty => "#ff4757",
    }
}

// ── Action button component ──────────────────────────────────────────────────

fn border_color(woody_here: bool) -> &'static str {
    if woody_here { "#39ff14" } else { "#2a2a4a" }
}

fn cursor_style(active: bool) -> &'static str {
    if active { "pointer" } else { "default" }
}

fn bg_style(active: bool, color: &str) -> String {
    if active { color.to_string() } else { "#2a2a4a".to_string() }
}

fn text_style(active: bool) -> &'static str {
    if active { "#000" } else { "#666" }
}

fn shadow_style(active: bool) -> &'static str {
    if active { "0 4px 0 rgba(0,0,0,0.4)" } else { "none" }
}

fn opacity_style(active: bool) -> &'static str {
    if active { "1" } else { "0.55" }
}

#[component]
fn ActionButton(
    label: &'static str,
    active: bool,
    color: &'static str,
    on_click: EventHandler<()>,
) -> Element {
    rsx! {
        button {
            style: "
                flex: 1; padding: 12px 6px; border-radius: 10px; border: none;
                font-size: 13px; font-weight: 700;
                cursor: {cursor_style(active)};
                background: {bg_style(active, color)};
                color: {text_style(active)};
                box-shadow: {shadow_style(active)};
                opacity: {opacity_style(active)};
                transition: all 0.15s;
            ",
            disabled: !active,
            onclick: move |_| on_click.call(()),
            "{label}"
        }
    }
}

// ── Core game action logic ─────────────────────────────────────────────────────

fn perform_action(
    state: &mut Signal<ShopState>,
    idx: usize,
    logs: &mut Signal<Vec<String>>,
) {
    let action: Option<(WoodyAction, String)> = state.with_mut(|s| {
        let table = s.tables.get_mut(idx)?;
        match *table {
            TableState::Seated { .. } => {
                let order = rand_order();
                *table = TableState::Preparing { order };
                Some((WoodyAction::PreparingAt(idx, order), format!("Taking order at table {}", idx + 1)))
            }
            TableState::Ready { order } => {
                *table = TableState::Eating;
                Some((WoodyAction::Serving(idx, order), format!("Serving at table {}", idx + 1)))
            }
            TableState::Dirty => {
                *table = TableState::Empty;
                Some((WoodyAction::Cleaning(idx), format!("Cleaning table {}", idx + 1)))
            }
            _ => None,
        }
    });

    let Some((action, msg)) = action else { return };
    state.with_mut(|s| s.woody_action = action.clone());
    haptic_light();
    logs.with_mut(|l| {
        if l.len() > 5 { l.remove(0); }
        l.push(msg);
    });

    let mut state = *state;
    let mut logs = *logs;
    spawn(async move {
        match action {
            WoodyAction::PreparingAt(idx, order) => {
                TimeoutFuture::new(PREPARE_MS).await;
                state.with_mut(|s| {
                    s.tables[idx] = TableState::Ready { order };
                    s.woody_action = WoodyAction::Idle;
                });
                haptic_success();
                logs.with_mut(|l| {
                    if l.len() > 5 { l.remove(0); }
                    l.push(format!("{} ready at table {}", order.emoji(), idx + 1));
                });
            }
            WoodyAction::Serving(idx, _order) => {
                TimeoutFuture::new(EAT_MS).await;
                state.with_mut(|s| {
                    s.tables[idx] = TableState::Dirty;
                    s.coins += SERVE_REWARD;
                    s.served += 1;
                    s.woody_action = WoodyAction::Idle;
                });
                haptic_success();
                logs.with_mut(|l| {
                    if l.len() > 5 { l.remove(0); }
                    l.push(format!("Customer left +{} 🪙", SERVE_REWARD));
                });
            }
            WoodyAction::Cleaning(_idx) => {
                TimeoutFuture::new(800).await;
                state.with_mut(|s| {
                    s.woody_action = WoodyAction::Idle;
                });
                haptic_light();
                logs.with_mut(|l| {
                    if l.len() > 5 { l.remove(0); }
                    l.push("Table cleaned".into());
                });
            }
            _ => {}
        }
    });
}
