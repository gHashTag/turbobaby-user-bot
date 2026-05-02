use dioxus::prelude::*;
use js_sys::eval;

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct QuestLocation {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub coordinates: (f64, f64),
    pub reward: &'static str,
    pub is_completed: bool,
}

pub static QUEST_LOCATIONS: &[QuestLocation] = &[
    QuestLocation { id: "moscow-park", name: "Moscow Park", description: "Find the secret garden in the park", coordinates: (55.7558, 37.6173), reward: "Sativa Seeds x5", is_completed: false },
    QuestLocation { id: "red-square", name: "Red Square", description: "Locate the hidden dispensary", coordinates: (55.7558, 37.6206), reward: "Indica Seeds x3", is_completed: false },
    QuestLocation { id: "gorky-park", name: "Gorky Park", description: "Discover the legendary strain", coordinates: (55.7901, 37.6812), reward: "Hybrid Seeds x10", is_completed: false },
];

#[component]
pub fn Quest(id: String) -> Element {
    let mut scanned = use_signal(|| false);
    let mut last_scan_result = use_signal(|| String::new());

    let scan_qr = move |_| {
        scanned.set(true);
        spawn(async move {
            let _ = eval(
                r#"if (window.Telegram?.WebApp?.showScanQrPopup) {
                    window.Telegram.WebApp.showScanQrPopup({text: 'Scan location QR'});
                }"#
            );
            last_scan_result.set("Scan initiated! Check Telegram app...".to_string());
            gloo_timers::callback::Timeout::new(2000, move || {
                scanned.set(false);
            })
            .forget();
        });
    };

    rsx! {
        div { class: "page quest-page",
            div { class: "quest-header",
                h1 { class: "quest-title", "Quest Mode" }
                p { class: "quest-description", "Scan QR codes at real-world locations to earn rewards!" }
            }
            if !last_scan_result.read().is_empty() {
                div { class: "scan-result",
                    p { "{last_scan_result.read()}" }
                    button {
                        class: "btn btn-secondary",
                        onclick: move |_| last_scan_result.set(String::new()),
                        "Dismiss"
                    }
                }
            }
            div { class: "quest-locations",
                for location in QUEST_LOCATIONS.iter() {
                    QuestLocationCard {
                        key: "{location.id}",
                        location: location.clone(),
                    }
                }
            }
            button {
                class: "btn btn-cyan scan-btn",
                disabled: *scanned.read(),
                onclick: scan_qr,
                "Scan QR Code"
            }
        }
    }
}

#[derive(Props, Clone, PartialEq)]
pub struct QuestLocationCardProps {
    location: QuestLocation,
}

#[component]
fn QuestLocationCard(props: QuestLocationCardProps) -> Element {
    let loc = props.location.clone();

    rsx! {
        div { class: "quest-location-card",
            h3 { class: "location-name", "{loc.name}" }
            p { class: "location-description", "{loc.description}" }
            div { class: "location-coordinates",
                span { "Coordinates: {loc.coordinates.0}, {loc.coordinates.1}" }
            }
            div { class: "location-reward",
                span { "Reward: {loc.reward}" }
            }
            if loc.is_completed {
                div { class: "completed-badge", "Completed" }
            } else {
                div { class: "pending-badge", "Pending" }
            }
        }
    }
}
