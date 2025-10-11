use axum::{
    extract::{Query, State},
    Json,
};

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
