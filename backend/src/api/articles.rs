use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;

use crate::{
    app::AppState,
    error::AppResult,
    model::{ArticleListQuery, ArticleOut, PageResp},
    service,
};

pub async fn list_articles(
    State(state): State<AppState>,
    Query(query): Query<ArticleListQuery>,
) -> AppResult<Json<PageResp<ArticleOut>>> {
    let page = service::articles::list(&state.pool, query).await?;
    Ok(Json(page))
}

#[derive(Debug, Deserialize)]
pub struct FeaturedQuery {
    pub limit: Option<i64>,
}

pub async fn list_featured(
    State(state): State<AppState>,
    Query(query): Query<FeaturedQuery>,
) -> AppResult<Json<Vec<ArticleOut>>> {
    let limit = query.limit.unwrap_or(10).clamp(1, 100);
    let articles = service::articles::list_featured(&state.pool, limit).await?;
    Ok(Json(articles))
}

pub async fn record_click(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> AppResult<StatusCode> {
    service::articles::record_click(&state.pool, id).await?;
    Ok(StatusCode::NO_CONTENT)
}
