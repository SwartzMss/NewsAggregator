use axum::{
    extract::{Path, State},
    Json,
};

use crate::{
    app::AppState,
    error::AppResult,
    model::{FeedOut, FeedUpsertPayload},
    service,
};

pub async fn list_feeds(State(state): State<AppState>) -> AppResult<Json<Vec<FeedOut>>> {
    let feeds = service::feeds::list(&state.pool).await?;
    Ok(Json(feeds))
}

pub async fn upsert_feed(
    State(state): State<AppState>,
    Json(payload): Json<FeedUpsertPayload>,
) -> AppResult<Json<FeedOut>> {
    let feed = service::feeds::upsert(&state.pool, payload).await?;
    Ok(Json(feed))
}

pub async fn delete_feed(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> AppResult<Json<serde_json::Value>> {
    service::feeds::delete(&state.pool, id).await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}
