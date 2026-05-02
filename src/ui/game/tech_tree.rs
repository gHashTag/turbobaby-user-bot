use dioxus::prelude::*;

#[derive(Clone, PartialEq, Debug)]
pub struct TechNode {
    pub id: &'static str,
    pub name: &'static str,
    pub cost: u32,
    pub requires: &'static [&'static str],
    pub unlocked: bool,
}

pub static TECH_TREE: &[TechNode] = &[
    TechNode { id: "basic_grow", name: "Basic Growing", cost: 10, requires: &[], unlocked: true },
    TechNode { id: "soil_boost", name: "Soil Boost", cost: 25, requires: &["basic_grow"], unlocked: false },
    TechNode { id: "led_lights", name: "LED Lights", cost: 75, requires: &["basic_grow"], unlocked: false },
    TechNode { id: "auto_water", name: "Auto Watering", cost: 50, requires: &["soil_boost"], unlocked: false },
    TechNode { id: "master_grower", name: "Master Grower", cost: 200, requires: &["auto_water", "led_lights"], unlocked: false },
];

#[derive(Props, PartialEq, Clone)]
pub struct TechNodeCardProps {
    tech: TechNode,
    unlocked_ids: Vec<&'static str>,
    progress_points: u32,
}

#[component]
fn TechNodeCard(props: TechNodeCardProps) -> Element {
    let _can_unlock = props.tech.requires.iter().all(|r| props.unlocked_ids.contains(r));
    let can_afford = props.progress_points >= props.tech.cost;

    rsx! {
        div { class: "tech-node-card",
            h3 { "{props.tech.name}" }
            span { "Cost: {props.tech.cost}" }
            if !props.tech.requires.is_empty() {
                span { "Requires: {props.tech.requires.join(\", \")}" }
            }
            if props.tech.unlocked {
                span { "Unlocked" }
            } else if can_afford {
                button { "Unlock" }
            } else {
                span { "Locked" }
            }
        }
    }
}

#[component]
pub fn TechTree() -> Element {
    let progress_points = use_signal(|| 0u32);
    let unlocked_ids: Vec<&str> = TECH_TREE.iter().filter(|t| t.unlocked).map(|t| t.id).collect();

    rsx! {
        div { class: "page tech-tree-page",
            h1 { "Tech Tree" }
            span { "Points: {progress_points}" }
            div { class: "tech-grid",
                for tech in TECH_TREE.iter() {
                    TechNodeCard {
                        key: "{tech.id}",
                        tech: tech.clone(),
                        unlocked_ids: unlocked_ids.clone(),
                        progress_points: *progress_points.read(),
                    }
                }
            }
        }
    }
}
