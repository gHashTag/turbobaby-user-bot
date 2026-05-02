use dioxus::prelude::*;
use web_sys::window;
use js_sys::eval;

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct QuestLocation {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub required_level: u8,
    pub completed: bool,
}

pub static QUEST_LOCATIONS: &[QuestLocation] = &[
    QuestLocation { id: "forest_clearing", name: "Forest Clearing", description: "Hidden spot in old forest", required_level: 1, completed: false },
    QuestLocation { id: "urban_basement", name: "Urban Basement", description: "Underground grow room", required_level: 2, completed: false },
    QuestLocation { id: "mountain_peak", name: "Mountain Peak", description: "High altitude outdoor plot", required_level: 3, completed: false },
    QuestLocation { id: "coastal_warehouse", name: "Coastal Warehouse", description: "Industrial grow facility", required_level: 4, completed: false },
    QuestLocation { id: "desert_oasis", name: "Desert Oasis", description: "Secret garden in sand", required_level: 5, completed: false },
];

#[component]
pub fn Quest() -> Element {
    let scanned = use_signal(|| false);
    let current_level = use_signal(|| 1u8);

    let scan_qr = move |_| {
        let mut scanned = scanned.clone();
        spawn(async move {
            if let Some(_w) = window() {
                let _ = eval(r#"if (window.Telegram?.WebApp?.showScanQrPopup) { window.Telegram.WebApp.showScanQrPopup({text: 'Scan location QR'}); }"#);
                scanned.set(true);
            }
        });
    };

    rsx! {
        div { class: "page quest-page",
            div { class: "quest-header",
                h1 { "Quest" }
                span { class: "level-badge", "Level {current_level}" }
            }
            div { class: "quest-map",
                for location in QUEST_LOCATIONS.iter() {
                    QuestLocationCard {
                        key: "{location.id}",
                        location: location.clone(),
                        level: current_level,
                    }
                }
            }
            button {
                class: "btn btn-cyan scan-btn",
                onclick: scan_qr,
                "Scan QR"
            }
            if *scanned.read() {
                div { class: "scan-success", "QR Code scanned! Location unlocked." }
            }
        }
    }
}

#[component]
pub fn QuestLocationCard(location: QuestLocation, level: Signal<u8>) -> Element {
    let is_unlocked = location.required_level <= *level.read();
    let status_class = if location.completed { "completed" } else if is_unlocked { "available" } else { "locked" };

    rsx! {
        div { class: "quest-location-card {status_class}",
            div { class: "location-icon",
                if location.completed { "✅" }
                else if is_unlocked { "📍" }
                else { "🔒" }
            }
            div { class: "location-info",
                h3 { "{location.name}" }
                p { "{location.description}" }
                span { class: "required-level", "Required Level: {location.required_level}" }
            }
            if is_unlocked && !location.completed {
                button { class: "btn btn-sm btn-quest", "GO TO LOCATION" }
            }
        }
    }
}
