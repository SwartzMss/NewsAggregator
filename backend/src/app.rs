use std::time::Duration;

use axum::{
    routing::{delete, get},
    Router,
};
use sqlx::{postgres::PgPoolOptions, PgPool};

use crate::{api, config::AppConfig, fetcher, repo};

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
}

pub async fn build_router(config: &AppConfig) -> anyhow::Result<Router> {
    let pool = PgPoolOptions::new()
        .max_connections(config.db.max_connections)
        .acquire_timeout(Duration::from_secs(5))
        .connect(&config.db.url)
        .await?;

    repo::migrations::ensure_schema(&pool).await?;

    fetcher::spawn(pool.clone(), config.fetcher.clone())?;

    let state = AppState { pool };

    let router = Router::new()
        .route("/healthz", get(api::health::health_check))
        .route(
            "/feeds",
            get(api::feeds::list_feeds).post(api::feeds::upsert_feed),
        )
        .route("/feeds/:id", delete(api::feeds::delete_feed))
        .route("/articles", get(api::articles::list_articles))
        .with_state(state);

    Ok(router)
}
