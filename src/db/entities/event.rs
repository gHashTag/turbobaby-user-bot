//! SeaORM entity for `events` — calendar events with media gallery.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[allow(dead_code)] // Standard SeaORM entity pattern
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "events")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub title: String,
    pub title_en: Option<String>,
    pub description: Option<String>,
    pub description_en: Option<String>,
    pub starts_at: DateTimeWithTimeZone,
    pub ends_at: Option<DateTimeWithTimeZone>,
    pub location_text: Option<String>,
    pub image_url: Option<String>,
    pub video_url: Option<String>,
    pub max_seats: Option<i32>,
    pub price_baht: Option<f64>,
    pub price_stars: Option<i64>,
    pub is_public: bool,
    pub created_at: Option<DateTimeWithTimeZone>,
    pub updated_at: Option<DateTimeWithTimeZone>,
}

#[allow(dead_code)] // Standard SeaORM entity pattern
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::event_photo::Entity")]
    EventPhotos,
}

impl Related<super::event_photo::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::EventPhotos.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
