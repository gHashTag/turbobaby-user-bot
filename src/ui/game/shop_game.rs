// Woody Shop v0.4 — single-screen mini-app game prototype.
//
// v0.1 shop core: customers spawn, order, get served, leave dirty tables.
// v0.2 farm: plant/water/harvest bushes.
// v0.3 upgrades: speed, tables, spawn rate.
// v0.4 DJ zone, grill zone, random events.
// Pure emoji/CSS visuals inside the existing Dioxus/WASM Telegram Mini App.

use dioxus::prelude::*;
use gloo_timers::future::TimeoutFuture;

// ── Domain types ───────────────────────────────────────────────────────────────

const MAX_TABLES: usize = 6;
const INITIAL_TABLES: usize = 3;
const WALK_MS: u32 = 350;
const PREPARE_MS: u32 = 1800;
const EAT_MS: u32 = 2200;
const CLEAN_MS: u32 = 700;
const SERVE_REWARD: u32 = 10;
const FARM_SLOTS: usize = 4;
const PLANT_COST: u32 = 15;
const HARVEST_REWARD: u32 = 45;
const DJ_PARTY_MS: u32 = 8000;
const GRILL_COOK_MS: u32 = 2500;
const EVENT_INTERVAL_MS: u32 = 25000;

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
    Seated,                     // customer waiting for order to be taken
    Preparing { order: Order }, // product is being made
    Ready { order: Order },     // product ready to serve
    Eating,                     // customer eats after being served
    Dirty,                      // table needs cleaning
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FarmStage {
    Empty,
    Planted,
    Watered,
    Grown,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WoodyAction {
    Idle,
    WalkingTo(usize),
    PreparingAt(usize, Order),
    Serving(usize, Order),
    Cleaning(usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActiveZone {
    Shop,
    Farm,
    Party,
    Grill,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Upgrades {
    pub table_count_level: u32,
    pub speed_level: u32,
    pub spawn_level: u32,
}

impl Upgrades {
    pub fn new() -> Self {
        Self {
            table_count_level: 1,
            speed_level: 1,
            spawn_level: 1,
        }
    }

    pub fn table_count(&self) -> usize {
        (INITIAL_TABLES + (self.table_count_level as usize).saturating_sub(1)).min(MAX_TABLES)
    }

    pub fn prepare_ms(&self) -> u32 {
        (PREPARE_MS as f32 * 0.85f32.powi((self.speed_level as i32).saturating_sub(1))) as u32
    }

    pub fn eat_ms(&self) -> u32 {
        (EAT_MS as f32 * 0.9f32.powi((self.speed_level as i32).saturating_sub(1))) as u32
    }

    pub fn spawn_interval_ms(&self) -> u32 {
        (6000f32 * 0.88f32.powi((self.spawn_level as i32).saturating_sub(1))) as u32
    }

    pub fn cost_table_count(level: u32) -> u32 {
        60 * level
    }
    pub fn cost_speed(level: u32) -> u32 {
        80 * level
    }
    pub fn cost_spawn(level: u32) -> u32 {
        70 * level
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ShopState {
    pub coins: u32,
    pub served: u32,
    pub harvested: u32,
    pub woody_table: usize,
    pub woody_action: WoodyAction,
    pub tables: [TableState; MAX_TABLES],
    pub farm: [FarmStage; FARM_SLOTS],
    pub farm_watering: [bool; FARM_SLOTS], // visual flag while watering
    pub upgrades: Upgrades,
    pub active_zone: ActiveZone,
    pub party_active: bool,
    pub party_timer_ms: u32,
    pub party_multiplier: f32,
    pub grill_stock: u32, // cooked food ready
    pub grill_cooking: bool,
    pub event_text: Option<String>,
}

impl ShopState {
    pub fn new() -> Self {
        Self {
            coins: 0,
            served: 0,
            harvested: 0,
            woody_table: 1,
            woody_action: WoodyAction::Idle,
            tables: [TableState::Empty; MAX_TABLES],
            farm: [FarmStage::Empty; FARM_SLOTS],
            farm_watering: [false; FARM_SLOTS],
            upgrades: Upgrades::new(),
            active_zone: ActiveZone::Shop,
            party_active: false,
            party_timer_ms: 0,
            party_multiplier: 1.0,
            grill_stock: 0,
            grill_cooking: false,
            event_text: None,
        }
    }

    pub fn table_count(&self) -> usize {
        self.upgrades.table_count()
    }

    fn first_empty_table(&self) -> Option<usize> {
        self.tables[..self.table_count()]
            .iter()
            .position(|s| *s == TableState::Empty)
    }

    fn is_busy(&self) -> bool {
        !matches!(self.woody_action, WoodyAction::Idle)
    }

    pub fn reward_per_serve(&self) -> u32 {
        let base = SERVE_REWARD;
        let party_bonus = if self.party_active { 5 } else { 0 };
        (base + party_bonus).max(1)
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

fn rand_event() -> &'static str {
    match rand_u32() % 5 {
        0 => "🎉 Rush hour! More customers coming!",
        1 => "💰 Big tip! +20 coins",
        2 => "🌿 Herb delivery! All farm plots watered",
        3 => "🎵 DJ energy up! Party lasts longer",
        4 => "🍔 Grill demand! Free food stock",
        _ => "🎉 Event!",
    }
}

fn haptic_light() {
    let _ =
        js_sys::eval("try { Telegram.WebApp.HapticFeedback.impactOccurred('light'); } catch(e) {}");
}

fn haptic_success() {
    let _ = js_sys::eval(
        "try { Telegram.WebApp.HapticFeedback.notificationOccurred('success'); } catch(e) {}",
    );
}

// ── Game component ─────────────────────────────────────────────────────────────

#[component]
pub fn WoodyShop() -> Element {
    let mut state = use_signal(ShopState::new);
    let logs = use_signal(|| Vec::<String>::new());

    // Customer spawn loop (depends on upgrades).
    {
        let mut state = state;
        let mut logs = logs;
        use_future(move || async move {
            loop {
                let delay = state.with(|s| s.upgrades.spawn_interval_ms());
                TimeoutFuture::new(delay).await;
                state.with_mut(|s| {
                    if let Some(idx) = s.first_empty_table() {
                        s.tables[idx] = TableState::Seated;
                    }
                });
                logs.with_mut(|l| {
                    if l.len() > 6 {
                        l.remove(0);
                    }
                    l.push("New customer arrived".into());
                });
            }
        });
    }

    // Farm growth loop: watered plots grow over time.
    {
        let mut state = state;
        use_future(move || async move {
            loop {
                TimeoutFuture::new(4000).await;
                state.with_mut(|s| {
                    for stage in s.farm.iter_mut() {
                        *stage = match *stage {
                            FarmStage::Watered => FarmStage::Grown,
                            FarmStage::Planted => FarmStage::Watered,
                            other => other,
                        };
                    }
                });
            }
        });
    }

    // Party timer loop.
    {
        let mut state = state;
        use_future(move || async move {
            loop {
                TimeoutFuture::new(200).await;
                state.with_mut(|s| {
                    if s.party_active {
                        if s.party_timer_ms > 200 {
                            s.party_timer_ms -= 200;
                        } else {
                            s.party_active = false;
                            s.party_timer_ms = 0;
                            s.party_multiplier = 1.0;
                        }
                    }
                });
            }
        });
    }

    // Random event loop.
    {
        let mut state = state;
        let mut logs = logs;
        use_future(move || async move {
            loop {
                TimeoutFuture::new(EVENT_INTERVAL_MS).await;
                let event = rand_event();
                state.with_mut(|s| {
                    match rand_u32() % 5 {
                        0 => {
                            // rush hour: fill empty tables
                            for i in 0..s.table_count() {
                                if s.tables[i] == TableState::Empty {
                                    s.tables[i] = TableState::Seated;
                                }
                            }
                        }
                        1 => {
                            s.coins += 20;
                        }
                        2 => {
                            for stage in s.farm.iter_mut() {
                                if *stage == FarmStage::Planted || *stage == FarmStage::Empty {
                                    *stage = FarmStage::Watered;
                                }
                            }
                        }
                        3 => {
                            if s.party_active {
                                s.party_timer_ms += 3000;
                            }
                        }
                        _ => {
                            s.grill_stock += 2;
                        }
                    }
                    s.event_text = Some(event.to_string());
                });
                logs.with_mut(|l| {
                    if l.len() > 6 {
                        l.remove(0);
                    }
                    l.push(event.to_string());
                });
                // Clear event banner after 4s.
                let mut state = state;
                spawn(async move {
                    TimeoutFuture::new(4000).await;
                    state.with_mut(|s| s.event_text = None);
                });
            }
        });
    }

    // Derived snapshots.
    let coins = state.read().coins;
    let served = state.read().served;
    let harvested = state.read().harvested;
    let woody_table = state.read().woody_table;
    let tables = state.read().tables.clone();
    let farm = state.read().farm.clone();
    let farm_watering = state.read().farm_watering.clone();
    let upgrades = state.read().upgrades.clone();
    let active_zone = state.read().active_zone;
    let party_active = state.read().party_active;
    let party_timer_ms = state.read().party_timer_ms;
    let grill_stock = state.read().grill_stock;
    let event_text = state.read().event_text.clone();
    let reward = state.read().reward_per_serve();

    // Helpers
    let mut buy_upgrade = move |kind: &'static str| {
        state.with_mut(|s| {
            let (cost, can_buy) = match kind {
                "tables" => {
                    let cost = Upgrades::cost_table_count(s.upgrades.table_count_level);
                    (
                        cost,
                        s.coins >= cost
                            && s.upgrades.table_count_level
                                < (MAX_TABLES - INITIAL_TABLES + 1) as u32,
                    )
                }
                "speed" => {
                    let cost = Upgrades::cost_speed(s.upgrades.speed_level);
                    (cost, s.coins >= cost && s.upgrades.speed_level < 5)
                }
                "spawn" => {
                    let cost = Upgrades::cost_spawn(s.upgrades.spawn_level);
                    (cost, s.coins >= cost && s.upgrades.spawn_level < 5)
                }
                _ => (0, false),
            };
            if can_buy {
                s.coins -= cost;
                match kind {
                    "tables" => s.upgrades.table_count_level += 1,
                    "speed" => s.upgrades.speed_level += 1,
                    "spawn" => s.upgrades.spawn_level += 1,
                    _ => {}
                }
            }
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
                    border-radius: 12px; padding: 10px 14px; margin-bottom: 12px;
                ",
                div {
                    style: "display: flex; align-items: center; gap: 6px;",
                    span { style: "font-size: 22px;", "🪙" }
                    span { style: "font-size: 20px; font-weight: 800; color: #ffe600;", "{coins}" }
                }
                div {
                    style: "font-size: 12px; color: #888; text-align: right;",
                    div { span { "Served: " }
                        span { style: "color: #39ff14; font-weight: 700;", "{served}" }
                    }
                    div { span { "Harvest: " }
                        span { style: "color: #b388ff; font-weight: 700;", "{harvested}" }
                    }
                }
            }

            // Zone tabs
            ZoneTabs { active_zone, state }

            // Event banner
            if let Some(text) = event_text {
                div {
                    style: "
                        margin-bottom: 10px; padding: 10px 14px; border-radius: 10px;
                        background: rgba(255,230,0,0.15); border: 2px solid #ffe600;
                        color: #ffe600; font-size: 13px; font-weight: 700; text-align: center;
                        animation: game-pulse 1s infinite;
                    ",
                    "{text}"
                }
            }

            // Main zone content
            div {
                style: "
                    flex: 1; background: #1a1a2e; border: 3px solid #2a2a4a;
                    border-radius: 16px; overflow: hidden; padding: 14px;
                    box-shadow: inset 0 0 40px rgba(0,0,0,0.5);
                    display: flex; flex-direction: column;
                ",
                match active_zone {
                    ActiveZone::Shop => rsx! {
                        ShopZone {
                            state,
                            logs,
                            coins,
                            served,
                            woody_table,
                            tables,
                            reward,
                            upgrades,
                        }
                    },
                    ActiveZone::Farm => rsx! {
                        FarmZone { state, logs, farm, farm_watering, coins }
                    },
                    ActiveZone::Party => rsx! {
                        PartyZone { state, logs, party_active, party_timer_ms }
                    },
                    ActiveZone::Grill => rsx! {
                        GrillZone { state, logs, grill_stock, grill_cooking: state.read().grill_cooking }
                    },
                }
            }

            // Upgrades panel (always visible below)
            div {
                style: "
                    margin-top: 10px; background: rgba(0,0,0,0.35);
                    border: 2px solid #2a2a4a; border-radius: 12px; padding: 10px;
                ",
                div { style: "font-size: 13px; font-weight: 700; color: #888; margin-bottom: 8px;", "🆙 UPGRADES" }
                div {
                    style: "display: flex; gap: 6px;",
                    UpgradeButton {
                        label: "🪑 Tables",
                        level: upgrades.table_count_level,
                        cost: Upgrades::cost_table_count(upgrades.table_count_level),
                        maxed: upgrades.table_count_level >= (MAX_TABLES - INITIAL_TABLES + 1) as u32,
                        coins,
                        on_click: move |_| buy_upgrade("tables"),
                    }
                    UpgradeButton {
                        label: "⚡ Speed",
                        level: upgrades.speed_level,
                        cost: Upgrades::cost_speed(upgrades.speed_level),
                        maxed: upgrades.speed_level >= 5,
                        coins,
                        on_click: move |_| buy_upgrade("speed"),
                    }
                    UpgradeButton {
                        label: "🚪 Flow",
                        level: upgrades.spawn_level,
                        cost: Upgrades::cost_spawn(upgrades.spawn_level),
                        maxed: upgrades.spawn_level >= 5,
                        coins,
                        on_click: move |_| buy_upgrade("spawn"),
                    }
                }
            }

            // Mini log
            div {
                style: "
                    margin-top: 10px; min-height: 56px;
                    font-size: 12px; color: #888; line-height: 1.45;
                    display: flex; flex-direction: column; gap: 2px;
                ",
                for entry in logs.read().iter().rev().enumerate().take(3) {
                    div { key: "{entry.0}", "• {entry.1}" }
                }
            }
        }
    }
}

// ── Zone tabs ──────────────────────────────────────────────────────────────────

#[component]
fn ZoneTabs(active_zone: ActiveZone, state: Signal<ShopState>) -> Element {
    let make_tab = |zone: ActiveZone, emoji: &str, label: &str| {
        let is_active = active_zone == zone;
        let bg = if is_active { "#39ff14" } else { "#2a2a4a" };
        let fg = if is_active { "#000" } else { "#888" };
        let mut state = state;
        let z = zone;
        rsx! {
            button {
                style: "
                    flex: 1; padding: 10px 4px; border-radius: 10px; border: none;
                    font-size: 12px; font-weight: 700; cursor: pointer;
                    background: {bg}; color: {fg}; transition: all 0.15s;
                ",
                onclick: move |_| state.with_mut(|s| s.active_zone = z),
                "{emoji} {label}"
            }
        }
    };

    rsx! {
        div {
            style: "display: flex; gap: 6px; margin-bottom: 12px;",
            {make_tab(ActiveZone::Shop, "🛒", "Shop")}
            {make_tab(ActiveZone::Farm, "🌱", "Farm")}
            {make_tab(ActiveZone::Party, "🎧", "DJ")}
            {make_tab(ActiveZone::Grill, "🍖", "Grill")}
        }
    }
}

// ── Shop zone ──────────────────────────────────────────────────────────────────

#[component]
fn ShopZone(
    state: Signal<ShopState>,
    logs: Signal<Vec<String>>,
    coins: u32,
    served: u32,
    woody_table: usize,
    tables: [TableState; MAX_TABLES],
    reward: u32,
    upgrades: Upgrades,
) -> Element {
    let on_table_click = move |idx: usize| {
        if state.read().is_busy() {
            return;
        }
        let mut state = state;
        let mut logs = logs;
        let current_table = state.read().woody_table;
        if current_table == idx {
            perform_shop_action(&mut state, idx, &mut logs, reward, &upgrades);
            return;
        }
        state.with_mut(|s| s.woody_action = WoodyAction::WalkingTo(idx));
        spawn(async move {
            TimeoutFuture::new(WALK_MS).await;
            state.with_mut(|s| {
                s.woody_table = idx;
                s.woody_action = WoodyAction::Idle;
            });
            logs.with_mut(|l| {
                if l.len() > 6 {
                    l.remove(0);
                }
                l.push(format!("Woody moved to table {}", idx + 1));
            });
        });
    };

    rsx! {
        div {
            style: "
                flex: 1; display: flex; flex-direction: column; justify-content: flex-end; gap: 14px;
                position: relative;
            ",
            div {
                style: "
                    position: absolute; top: 0; left: 50%; transform: translateX(-50%);
                    font-size: 13px; font-weight: 700; letter-spacing: 2px;
                    color: #39ff14; text-shadow: 0 0 10px rgba(57,255,20,0.4);
                ",
                "WOODY SHOP"
            }
            for (idx, table) in tables.iter().enumerate() {
                if idx < state.read().table_count() {
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
                style: "display: flex; gap: 6px; margin-top: 6px;",
                ActionButton {
                    label: "👋 Order",
                    active: matches!(tables[woody_table], TableState::Seated),
                    color: "#39ff14",
                    on_click: {
                        let mut state = state;
                        let mut logs = logs;
                        move |_| {
                            if state.read().is_busy() { return; }
                            perform_shop_action(&mut state, woody_table, &mut logs, reward, &upgrades);
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
                            perform_shop_action(&mut state, woody_table, &mut logs, reward, &upgrades);
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
                            perform_shop_action(&mut state, woody_table, &mut logs, reward, &upgrades);
                        }
                    },
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
    let customer_emoji = if matches!(table, TableState::Empty) {
        ""
    } else {
        "🧑‍🦱"
    };
    let order_bubble = match table {
        TableState::Seated => Some(rand_order().emoji()),
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
                        font-size: 22px; animation: game-pulse 1.2s infinite;
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
        TableState::Seated => "Customer waiting",
        TableState::Preparing { .. } => "Preparing...",
        TableState::Ready { .. } => "Order ready",
        TableState::Eating => "Customer eating",
        TableState::Dirty => "Dirty table",
    }
}

fn table_color(table: TableState) -> &'static str {
    match table {
        TableState::Empty => "#666",
        TableState::Seated => "#39ff14",
        TableState::Preparing { .. } => "#ff9d00",
        TableState::Ready { .. } => "#00e5ff",
        TableState::Eating => "#ffe600",
        TableState::Dirty => "#ff4757",
    }
}

fn border_color(woody_here: bool) -> &'static str {
    if woody_here {
        "#39ff14"
    } else {
        "#2a2a4a"
    }
}

// ── Farm zone ─────────────────────────────────────────────────────────────────

#[component]
fn FarmZone(
    state: Signal<ShopState>,
    logs: Signal<Vec<String>>,
    farm: [FarmStage; FARM_SLOTS],
    farm_watering: [bool; FARM_SLOTS],
    coins: u32,
) -> Element {
    rsx! {
        div {
            style: "flex: 1; display: flex; flex-direction: column; gap: 10px;",
            div {
                style: "
                    font-size: 13px; font-weight: 700; letter-spacing: 2px;
                    color: #39ff14; text-align: center;
                ",
                "🌱 FARM"
            }
            div {
                style: "display: grid; grid-template-columns: 1fr 1fr; gap: 10px;",
                for (idx, stage) in farm.iter().enumerate() {
                    FarmPlot {
                        key: "{idx}",
                        idx,
                        stage: *stage,
                        watering: farm_watering[idx],
                        state,
                        logs,
                        coins,
                    }
                }
            }
        }
    }
}

#[component]
fn FarmPlot(
    idx: usize,
    stage: FarmStage,
    watering: bool,
    state: Signal<ShopState>,
    logs: Signal<Vec<String>>,
    coins: u32,
) -> Element {
    let (emoji, label, can_plant, can_water, can_harvest) = match stage {
        FarmStage::Empty => ("🟫", "Empty soil", true, false, false),
        FarmStage::Planted => ("🌱", "Seedling", false, true, false),
        FarmStage::Watered => ("🌿", "Growing", false, false, false),
        FarmStage::Grown => ("🌳", "Ready!", false, false, true),
    };

    let plant_emoji = if watering { "💧" } else { emoji };

    rsx! {
        div {
            style: "
                background: rgba(255,255,255,0.04); border: 2px solid #2a2a4a;
                border-radius: 14px; padding: 12px; text-align: center;
                display: flex; flex-direction: column; align-items: center; gap: 6px;
            ",
            div { style: "font-size: 42px;", "{plant_emoji}" }
            div { style: "font-size: 12px; color: #888;", "{label}" }
            div {
                style: "display: flex; gap: 4px; width: 100%; margin-top: 4px;",
                ActionButton {
                    label: "🌱 Plant",
                    active: can_plant && coins >= PLANT_COST,
                    color: "#39ff14",
                    on_click: {
                        let mut state = state;
                        let mut logs = logs;
                        move |_| {
                            state.with_mut(|s| {
                                if s.farm[idx] == FarmStage::Empty && s.coins >= PLANT_COST {
                                    s.coins -= PLANT_COST;
                                    s.farm[idx] = FarmStage::Planted;
                                }
                            });
                            logs.with_mut(|l| {
                                if l.len() > 6 { l.remove(0); }
                                l.push(format!("Planted seed -{}", PLANT_COST));
                            });
                        }
                    },
                }
                ActionButton {
                    label: "💧 Water",
                    active: can_water,
                    color: "#00e5ff",
                    on_click: {
                        let mut state = state;
                        let mut logs = logs;
                        move |_| {
                            state.with_mut(|s| {
                                if s.farm[idx] == FarmStage::Planted {
                                    s.farm_watering[idx] = true;
                                }
                            });
                            logs.with_mut(|l| {
                                if l.len() > 6 { l.remove(0); }
                                l.push("Watering...".into());
                            });
                            let mut state = state;
                            spawn(async move {
                                TimeoutFuture::new(1200).await;
                                state.with_mut(|s| {
                                    s.farm_watering[idx] = false;
                                    s.farm[idx] = FarmStage::Watered;
                                });
                                haptic_light();
                            });
                        }
                    },
                }
                ActionButton {
                    label: "✂️ Harvest",
                    active: can_harvest,
                    color: "#ffe600",
                    on_click: {
                        let mut state = state;
                        let mut logs = logs;
                        move |_| {
                            state.with_mut(|s| {
                                if s.farm[idx] == FarmStage::Grown {
                                    s.farm[idx] = FarmStage::Empty;
                                    s.coins += HARVEST_REWARD;
                                    s.harvested += 1;
                                }
                            });
                            logs.with_mut(|l| {
                                if l.len() > 6 { l.remove(0); }
                                l.push(format!("Harvest! +{} 🪙", HARVEST_REWARD));
                            });
                            haptic_success();
                        }
                    },
                }
            }
        }
    }
}

// ── Party zone ────────────────────────────────────────────────────────────────

#[component]
fn PartyZone(
    state: Signal<ShopState>,
    logs: Signal<Vec<String>>,
    party_active: bool,
    party_timer_ms: u32,
) -> Element {
    let cost: u32 = 40;
    let seconds = party_timer_ms / 1000;
    let emoji = if party_active { "🎉" } else { "🎧" };
    let status_text = if party_active {
        "Party ON — tips +5 🪙 per serve"
    } else {
        "Start party to boost shop income"
    };
    let button_label = if party_active {
        "🔥 Party ON"
    } else {
        "🚀 Start Party"
    };

    rsx! {
        div {
            style: "flex: 1; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 16px;",
            div {
                style: "
                    font-size: 13px; font-weight: 700; letter-spacing: 2px;
                    color: #d946ef; text-align: center;
                ",
                "🎧 DJ ZONE"
            }
            div {
                style: "font-size: 72px; filter: drop-shadow(0 0 20px rgba(217,70,239,0.5)); animation: game-bounce 0.6s infinite alternate;",
                "{emoji}"
            }
            div {
                style: "font-size: 14px; color: #888; text-align: center;",
                "{status_text}"
            }
            if party_active {
                div {
                    style: "font-size: 24px; font-weight: 800; color: #d946ef;",
                    "⏳ {seconds}s"
                }
            }
            ActionButton {
                label: button_label,
                active: !party_active && state.read().coins >= cost,
                color: "#d946ef",
                on_click: {
                    let mut state = state;
                    let mut logs = logs;
                    move |_| {
                        state.with_mut(|s| {
                            if !s.party_active && s.coins >= cost {
                                s.coins -= cost;
                                s.party_active = true;
                                s.party_timer_ms = DJ_PARTY_MS;
                            }
                        });
                        logs.with_mut(|l| {
                            if l.len() > 6 { l.remove(0); }
                            l.push(format!("Party started! -{}", cost));
                        });
                        haptic_success();
                    }
                },
            }
        }
    }
}

// ── Grill zone ────────────────────────────────────────────────────────────────

#[component]
fn GrillZone(
    state: Signal<ShopState>,
    logs: Signal<Vec<String>>,
    grill_stock: u32,
    grill_cooking: bool,
) -> Element {
    let cost: u32 = 10;
    let emoji = if grill_cooking { "🔥" } else { "🍖" };
    let button_label = if grill_cooking {
        "🔥 Cooking..."
    } else {
        "🍳 Cook"
    };

    rsx! {
        div {
            style: "flex: 1; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 16px;",
            div {
                style: "
                    font-size: 13px; font-weight: 700; letter-spacing: 2px;
                    color: #ff9d00; text-align: center;
                ",
                "🍖 VERANDA GRILL"
            }
            div {
                style: "font-size: 72px; filter: drop-shadow(0 0 20px rgba(255,157,0,0.4));",
                "{emoji}"
            }
            div {
                style: "font-size: 16px; font-weight: 700; color: #ff9d00;",
                "Stock: {grill_stock}"
            }
            div {
                style: "font-size: 13px; color: #888; text-align: center;",
                "Cook food. Serves hungry customers instantly when in shop."
            }
            div {
                style: "display: flex; gap: 6px;",
                ActionButton {
                    label: button_label,
                    active: !grill_cooking && state.read().coins >= cost,
                    color: "#ff9d00",
                    on_click: {
                        let mut state = state;
                        let mut logs = logs;
                        move |_| {
                            state.with_mut(|s| {
                                if !s.grill_cooking && s.coins >= cost {
                                    s.coins -= cost;
                                    s.grill_cooking = true;
                                }
                            });
                            logs.with_mut(|l| {
                                if l.len() > 6 { l.remove(0); }
                                l.push(format!("Cooking started -{}", cost));
                            });
                            let mut state = state;
                            spawn(async move {
                                TimeoutFuture::new(GRILL_COOK_MS).await;
                                state.with_mut(|s| {
                                    s.grill_cooking = false;
                                    s.grill_stock += 1;
                                });
                                haptic_success();
                            });
                        }
                    },
                }
            }
        }
    }
}

// ── Upgrade button component ───────────────────────────────────────────────────

#[component]
fn UpgradeButton(
    label: &'static str,
    level: u32,
    cost: u32,
    maxed: bool,
    coins: u32,
    on_click: EventHandler<()>,
) -> Element {
    let bg = if maxed {
        "#1a3a1a"
    } else if coins >= cost {
        "#39ff14"
    } else {
        "#2a2a4a"
    };
    let fg = if maxed || coins >= cost {
        "#000"
    } else {
        "#666"
    };
    let opacity = if maxed { "0.7" } else { "1" };

    rsx! {
        button {
            style: "
                flex: 1; padding: 10px 4px; border-radius: 8px; border: none;
                font-size: 11px; font-weight: 700; cursor: pointer;
                background: {bg}; color: {fg}; opacity: {opacity};
                transition: all 0.15s;
            ",
            disabled: maxed || coins < cost,
            onclick: move |_| on_click.call(()),
            div { "{label}" }
            div { style: "font-size: 10px; opacity: 0.8;",
                if maxed { "MAX" } else { "Lv{level} • {cost}🪙" }
            }
        }
    }
}

// ── Action button component ──────────────────────────────────────────────────

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
                flex: 1; padding: 10px 4px; border-radius: 8px; border: none;
                font-size: 12px; font-weight: 700; cursor: {cursor_style(active)};
                background: {bg_style(active, color)}; color: {text_style(active)};
                box-shadow: {shadow_style(active)}; opacity: {opacity_style(active)};
                transition: all 0.15s;
            ",
            disabled: !active,
            onclick: move |_| on_click.call(()),
            "{label}"
        }
    }
}

fn cursor_style(active: bool) -> &'static str {
    if active {
        "pointer"
    } else {
        "default"
    }
}
fn bg_style(active: bool, color: &'static str) -> &'static str {
    if active {
        color
    } else {
        "#2a2a4a"
    }
}
fn text_style(active: bool) -> &'static str {
    if active {
        "#000"
    } else {
        "#666"
    }
}
fn shadow_style(active: bool) -> &'static str {
    if active {
        "0 3px 0 rgba(0,0,0,0.4)"
    } else {
        "none"
    }
}
fn opacity_style(active: bool) -> &'static str {
    if active {
        "1"
    } else {
        "0.55"
    }
}

// ── Core shop action logic ─────────────────────────────────────────────────────

fn perform_shop_action(
    state: &mut Signal<ShopState>,
    idx: usize,
    logs: &mut Signal<Vec<String>>,
    reward: u32,
    upgrades: &Upgrades,
) {
    let action: Option<(WoodyAction, String)> = state.with_mut(|s| {
        let table = s.tables.get_mut(idx)?;
        match *table {
            TableState::Seated => {
                let order = rand_order();
                *table = TableState::Preparing { order };
                Some((
                    WoodyAction::PreparingAt(idx, order),
                    format!("Taking order at table {}", idx + 1),
                ))
            }
            TableState::Ready { order } => {
                *table = TableState::Eating;
                Some((
                    WoodyAction::Serving(idx, order),
                    format!("Serving at table {}", idx + 1),
                ))
            }
            TableState::Dirty => {
                *table = TableState::Empty;
                Some((
                    WoodyAction::Cleaning(idx),
                    format!("Cleaning table {}", idx + 1),
                ))
            }
            _ => None,
        }
    });

    let Some((action, msg)) = action else { return };
    state.with_mut(|s| s.woody_action = action.clone());
    haptic_light();
    logs.with_mut(|l| {
        if l.len() > 6 {
            l.remove(0);
        }
        l.push(msg);
    });

    let prepare_ms = upgrades.prepare_ms();
    let eat_ms = upgrades.eat_ms();
    let mut state = *state;
    let mut logs = *logs;
    spawn(async move {
        match action {
            WoodyAction::PreparingAt(idx, order) => {
                TimeoutFuture::new(prepare_ms).await;
                state.with_mut(|s| {
                    s.tables[idx] = TableState::Ready { order };
                    s.woody_action = WoodyAction::Idle;
                });
                haptic_success();
                logs.with_mut(|l| {
                    if l.len() > 6 {
                        l.remove(0);
                    }
                    l.push(format!("{} ready at table {}", order.emoji(), idx + 1));
                });
            }
            WoodyAction::Serving(idx, _order) => {
                TimeoutFuture::new(eat_ms).await;
                state.with_mut(|s| {
                    s.tables[idx] = TableState::Dirty;
                    s.coins += reward;
                    s.served += 1;
                    s.woody_action = WoodyAction::Idle;
                });
                haptic_success();
                logs.with_mut(|l| {
                    if l.len() > 6 {
                        l.remove(0);
                    }
                    l.push(format!("Customer left +{} 🪙", reward));
                });
            }
            WoodyAction::Cleaning(_idx) => {
                TimeoutFuture::new(CLEAN_MS).await;
                state.with_mut(|s| {
                    s.woody_action = WoodyAction::Idle;
                });
                haptic_light();
                logs.with_mut(|l| {
                    if l.len() > 6 {
                        l.remove(0);
                    }
                    l.push("Table cleaned".into());
                });
            }
            _ => {}
        }
    });
}
