use axum::{extract::State, response::IntoResponse, Json};
use axum::response::sse::Sse;
use serde::Deserialize;

use crate::{app::AppState, ops::events as ops_events, repo::events as repo_events};

#[derive(Deserialize)]
pub struct ListQuery {
    level: Option<String>,
    code: Option<String>,
    source: Option<String>,
    #[serde(default)]
    from: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(default)]
    to: Option<chrono::DateTime<chrono::Utc>>,
    since_id: Option<i64>,
    limit: Option<i64>,
}

pub async fn list_alerts(State(state): State<AppState>, axum::extract::Query(q): axum::extract::Query<ListQuery>) -> impl IntoResponse {
    let params = repo_events::ListParams {
        level: q.level,
        code: q.code,
        source: q.source,
        from: q.from,
        to: q.to,
        since_id: q.since_id,
        limit: q.limit,
    };
    match repo_events::list_events(&state.pool, &params).await {
        Ok(items) => Json(items).into_response(),
        Err(err) => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response(),
    }
}

pub async fn stream_alerts(State(state): State<AppState>) -> Sse<impl futures::Stream<Item = Result<axum::response::sse::Event, std::convert::Infallible>>> {
    ops_events::sse_response(&state.events)
}
