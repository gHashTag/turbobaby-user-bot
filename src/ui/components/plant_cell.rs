use crate::trios::garden::{GrowthStage, Plant};
use crate::trios::i18n::{t, T_GARDEN_HARVEST, T_GARDEN_PLANT};
use crate::ui::lang;
use dioxus::prelude::*;

#[component]
pub fn PlantCell(plant: Plant, on_harvest: EventHandler<String>) -> Element {
    let lang = lang::current_lang();
    let img_idx = (plant.water_count % 14) + 1;
    let img_src = format!("/assets/game/{}.png", img_idx);
    let emoji = plant.current_stage.emoji();
    let name = plant.current_stage.name();
    let is_ready = plant.is_completed;
    let pid = plant.id.clone();
    let color = match plant.current_stage {
        GrowthStage::Seed => "#8b5a2b",
        GrowthStage::Sprout | GrowthStage::FirstLeaf => "#39ff14",
        GrowthStage::YoungBush | GrowthStage::VegStart | GrowthStage::BigVeg => "#00e5ff",
        GrowthStage::PreFlower | GrowthStage::SmallBuds | GrowthStage::BigBuds => "#ff6b35",
        GrowthStage::Trimming | GrowthStage::Curing => "#a78bfa",
        GrowthStage::Lab | GrowthStage::Delivery => "#60a5fa",
        GrowthStage::Final => "#ffd700",
    };
    let class = if is_ready {
        "plant-cell harvestable"
    } else {
        "plant-cell"
    };

    rsx! {
        div { class: "{class}",
            img { src: "{img_src}", class: "plant-sprite", alt: "plant", loading: "lazy" }
            span { class: "stage-label", style: "color: {color};", "{emoji} {name}" }
            span { class: "ticks", "Water: {plant.water_count}" }
            if is_ready {
                button {
                    class: "btn btn-green harvest-btn",
                    onclick: move |_| on_harvest.call(pid.clone()),
                    "{t(lang, T_GARDEN_HARVEST)}"
                }
            }
        }
    }
}

#[component]
pub fn EmptyPlotCell(on_plant: EventHandler<usize>) -> Element {
    let lang = lang::current_lang();
    rsx! {
        div { class: "plant-cell empty",
            div { class: "empty-plot-icon", "🕳️" }
            button {
                class: "btn btn-green plant-btn",
                onclick: move |_| on_plant.call(0),
                "{t(lang, T_GARDEN_PLANT)}"
            }
        }
    }
}
