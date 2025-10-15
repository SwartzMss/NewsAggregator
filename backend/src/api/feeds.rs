use axum::{
    extract::{Path, State},
    Json,
};

use crate::{
    app::AppState,
    error::AppResult,
    model::{FeedOut, FeedTestPayload, FeedTestResult, FeedUpsertPayload},
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
    let feed = service::feeds::upsert(
        &state.pool,
        &state.http_client,
        &state.fetcher_config,
        &state.translator,
        payload,
    )
    .await?;
    Ok(Json(feed))
}

pub async fn delete_feed(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> AppResult<Json<serde_json::Value>> {
    service::feeds::delete(&state.pool, id).await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

pub async fn test_feed(
    State(state): State<AppState>,
    Json(payload): Json<FeedTestPayload>,
) -> AppResult<Json<FeedTestResult>> {
    let result = service::feeds::test(&state.http_client, payload).await?;
    Ok(Json(result))
}
