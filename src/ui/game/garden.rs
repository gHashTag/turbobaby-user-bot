use dioxus::prelude::*;
use gloo_timers::future::TimeoutFuture;
use crate::ui::components::{PlantCell, EmptyPlotCell};

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum GrowthStage {
    Seed,
    Sprout,
    Veg,
    Flower,
    Harvest,
}

#[derive(Clone, PartialEq, Debug)]
pub struct Plant {
    pub id: u32,
    pub stage: GrowthStage,
    pub ticks: u32,
    pub asset_idx: u8,
}

impl Plant {
    pub fn new(id: u32) -> Self {
        Self { id, stage: GrowthStage::Seed, ticks: 0, asset_idx: ((id % 14) + 1) as u8 }
    }

    pub fn tick(&mut self) {
        self.ticks += 1;
        self.stage = match self.ticks {
            0..=5 => GrowthStage::Seed,
            6..=15 => GrowthStage::Sprout,
            16..=30 => GrowthStage::Veg,
            31..=50 => GrowthStage::Flower,
            _ => GrowthStage::Harvest,
        };
    }

    pub fn is_harvestable(&self) -> bool {
        matches!(self.stage, GrowthStage::Harvest)
    }
}

#[component]
fn PlantGridRow(plant: Plant, on_harvest: EventHandler<u32>) -> Element {
    let pid = plant.id;
    rsx! {
        PlantCell { key: "{pid}", plant, on_harvest: move |_| on_harvest.call(pid) }
    }
}

#[component]
pub fn Garden() -> Element {
    let plants = use_signal(|| Vec::new());
    let tick_count = use_signal(|| 0u32);

    let mut plants_clone = plants.clone();
    let mut tick_count_clone = tick_count.clone();
    use_future(move || async move {
        loop {
            TimeoutFuture::new(3000).await;
            *tick_count_clone.write() += 1;
            plants_clone.write().iter_mut().for_each(|p: &mut Plant| p.tick());
        }
    });

    let mut plants_for_seed = plants.clone();
    let plant_seed = move |_| {
        if plants_for_seed.read().len() < 9 {
            let next_id = plants_for_seed.read().iter().map(|p| p.id).max().unwrap_or(0) + 1;
            plants_for_seed.write().push(Plant::new(next_id));
        }
    };

    let mut plants_for_harvest = plants.clone();
    let harvest_plant = move |plant_id: u32| {
        plants_for_harvest.write().retain(|p| p.id != plant_id);
    };

    let plant_list = plants.read().clone();
    let empty_slots = 9 - plant_list.len();

    rsx! {
        div { class: "page garden-page",
            div { class: "garden-header",
                h1 { class: "garden-title", "Garden" }
                span { class: "tick-counter", "Day {tick_count}" }
            }
            div { class: "garden-grid",
                {plant_list.iter().map(|plant| {
                    let p = plant.clone();
                    rsx! {
                        PlantGridRow {
                            key: "{p.id}",
                            plant: p.clone(),
                            on_harvest: harvest_plant
                        }
                    }
                })}
                for slot in 0..empty_slots {
                    EmptyPlotCell { key: "{slot}", on_plant: plant_seed }
                }
            }
        }
    }
}
