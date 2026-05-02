// Plant Cell Components for Garden Game Module
use dioxus::prelude::*;
use crate::ui::game::garden::{Plant, GrowthStage};

#[component]
pub fn PlantCell(plant: Plant, on_harvest: EventHandler<()>) -> Element {
    let img_src = format!("/assets/images/game/{}.png", plant.asset_idx);
    let stage_label = match plant.stage {
        GrowthStage::Seed => "🌱 Seed",
        GrowthStage::Sprout => "🌿 Sprout",
        GrowthStage::Veg => "🌲 Veg",
        GrowthStage::Flower => "🌸 Flower",
        GrowthStage::Harvest => "✂️ Ready!",
    };
    let class_name = if plant.is_harvestable() {
        "plant-cell harvestable"
    } else {
        "plant-cell"
    };

    rsx! {
        div { class: "{class_name}",
            img { src: "{img_src}", class: "plant-sprite", alt: "plant" }
            span { class: "stage-label", "{stage_label}" }
            span { class: "ticks", "Ticks: {plant.ticks}" }
            if plant.is_harvestable() {
                button {
                    class: "btn btn-green harvest-btn",
                    onclick: move |_| on_harvest.call(()),
                    "HARVEST"
                }
            }
        }
    }
}

#[component]
pub fn EmptyPlotCell(on_plant: EventHandler<usize>) -> Element {
    rsx! {
        div { class: "plant-cell empty",
            div { class: "empty-plot-icon", "🕳️" }
            button {
                class: "btn btn-green plant-btn",
                onclick: move |_| on_plant.call(0),
                "PLANT"
            }
        }
    }
}
