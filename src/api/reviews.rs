use axum::{
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::Json,
    routing::{get, post},
    Router,
};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect, Set,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::api::auth::{check_not_blocked, check_owner};
use crate::db::entities::{order, strain_review};
use crate::db::orders::OrderItem;
use crate::AppState;

#[derive(Deserialize)]
pub(crate) struct CreateReviewRequest {
    order_id: String,
    strain_id: String,
    rating: i32,
    #[serde(default)]
    comment: String,
}

#[derive(Serialize)]
struct ReviewResponse {
    id: String,
    telegram_id: i64,
    strain_id: String,
    order_id: String,
    rating: i32,
    comment: String,
    created_at: String,
}

#[derive(Serialize)]
struct ReviewsListResponse {
    reviews: Vec<ReviewResponse>,
    average_rating: Option<f64>,
}

#[derive(Deserialize)]
struct ListReviewsQuery {
    strain_id: String,
}

pub(crate) fn routes() -> Router<AppState> {
    Router::new()
        .route("/reviews", get(list_reviews))
        .route("/reviews", post(create_review))
}

async fn list_reviews(
    Query(query): Query<ListReviewsQuery>,
    State(state): State<AppState>,
) -> Result<Json<Value>, StatusCode> {
    let rows = strain_review::Entity::find()
        .filter(strain_review::Column::StrainId.eq(query.strain_id))
        .filter(strain_review::Column::Approved.eq(true))
        .order_by_desc(strain_review::Column::CreatedAt)
        .limit(50)
        .all(&state.db.orm)
        .await
        .map_err(|e| {
            tracing::error!("list_reviews DB error: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let average = if rows.is_empty() {
        None
    } else {
        let sum: i64 = rows.iter().map(|r| r.rating as i64).sum();
        Some(sum as f64 / rows.len() as f64)
    };

    let reviews = rows
        .into_iter()
        .map(|r| ReviewResponse {
            id: r.id,
            telegram_id: r.telegram_id,
            strain_id: r.strain_id,
            order_id: r.order_id,
            rating: r.rating,
            comment: r.comment,
            created_at: r.created_at.to_rfc3339(),
        })
        .collect();

    Ok(Json(json!(ReviewsListResponse {
        reviews,
        average_rating: average,
    })))
}

async fn create_review(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(req): Json<CreateReviewRequest>,
) -> Result<Json<Value>, StatusCode> {
    if req.rating < 1 || req.rating > 5 {
        return Err(StatusCode::BAD_REQUEST);
    }
    if req.comment.len() > 1000 {
        return Err(StatusCode::BAD_REQUEST);
    }

    let order_id = req.order_id.clone();
    let strain_id = req.strain_id.clone();

    let order = order::Entity::find_by_id(order_id.clone())
        .one(&state.db.orm)
        .await
        .map_err(|e| {
            tracing::error!("create_review order lookup error: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or(StatusCode::NOT_FOUND)?;

    let telegram_id = order.telegram_id.ok_or(StatusCode::BAD_REQUEST)?;

    // Owner-auth: the review must come from the order's real Telegram user.
    check_owner(&headers, &state, telegram_id)?;
    check_not_blocked(&state, telegram_id).await?;

    // Only delivered/completed orders can be reviewed.
    let deliverable = matches!(order.status.as_str(), "delivered" | "completed");
    if !deliverable {
        return Err(StatusCode::UNPROCESSABLE_ENTITY);
    }

    let items: Vec<OrderItem> = serde_json::from_value(order.items.clone())
        .map_err(|e| {
            tracing::warn!("create_review: cannot parse order items: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let strain_bought = items
        .iter()
        .any(|item| item.strain_id.as_deref() == Some(&strain_id));
    if !strain_bought {
        return Err(StatusCode::FORBIDDEN);
    }

    // One review per (order, strain) pair.
    let existing = strain_review::Entity::find()
        .filter(strain_review::Column::OrderId.eq(&order_id))
        .filter(strain_review::Column::StrainId.eq(strain_id.clone()))
        .one(&state.db.orm)
        .await
        .map_err(|e| {
            tracing::error!("create_review duplicate check error: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    if existing.is_some() {
        return Err(StatusCode::CONFLICT);
    }

    let clean_comment = crate::util::html_escape(&req.comment.trim()).trim().to_string();

    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Local::now().fixed_offset();
    let active = strain_review::ActiveModel {
        id: Set(id.clone()),
        telegram_id: Set(telegram_id),
        strain_id: Set(strain_id.clone()),
        order_id: Set(order_id.clone()),
        rating: Set(req.rating),
        comment: Set(clean_comment),
        created_at: Set(now.into()),
        approved: Set(true),
    };

    active.insert(&state.db.orm).await.map_err(|e| {
        tracing::error!("create_review insert error: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(json!({
        "id": id,
        "telegram_id": telegram_id,
        "strain_id": strain_id,
        "order_id": order_id,
        "rating": req.rating,
        "comment": req.comment,
    })))
}
