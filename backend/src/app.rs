use std::time::Duration;

use axum::{
    middleware,
    routing::{delete, get, post},
    Router,
};
use sqlx::{postgres::PgPoolOptions, PgPool};
use tower::ServiceBuilder;
use tower_http::cors::{Any, CorsLayer};

use crate::{
    api, auth,
    config::{AppConfig, FrontendPublicConfig, HttpClientConfig},
    fetcher, repo,
};

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub config: FrontendPublicConfig,
    pub admin: auth::AdminManager,
    pub http_client: HttpClientConfig,
}

pub async fn build_router(config: &AppConfig) -> anyhow::Result<Router> {
    let pool = PgPoolOptions::new()
        .max_connections(config.db.max_connections)
        .acquire_timeout(Duration::from_secs(5))
        .connect(&config.db.url)
        .await?;

    repo::migrations::ensure_schema(&pool).await?;
    repo::maintenance::cleanup_orphan_content(&pool).await?;

    fetcher::spawn(
        pool.clone(),
        config.fetcher.clone(),
        config.http_client.clone(),
        config.ai.clone(),
    )?;

    let public_config = config.frontend_public_config();
    let admin_manager = auth::AdminManager::new(
        config.admin.username.clone(),
        config.admin.password.clone(),
        Duration::from_secs(std::cmp::max(60_u64, config.admin.session_ttl_secs)),
    );

    let state = AppState {
        pool,
        config: public_config,
        admin: admin_manager,
        http_client: config.http_client.clone(),
    };

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);
    let middleware = ServiceBuilder::new().layer(cors);

    let admin_api = Router::new()
        .route(
            "/feeds",
            get(api::feeds::list_feeds).post(api::feeds::upsert_feed),
        )
        .route("/feeds/test", post(api::feeds::test_feed))
        .route("/feeds/:id", delete(api::feeds::delete_feed))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            auth::require_admin,
        ))
        .with_state(state.clone());

    let router = Router::new()
        .route("/healthz", get(api::health::health_check))
        .route("/articles", get(api::articles::list_articles))
        .route("/articles/featured", get(api::articles::list_featured))
        .route("/articles/:id/click", post(api::articles::record_click))
        .route("/config/frontend", get(api::config::frontend_config))
        .route("/admin/login", post(api::admin::login))
        .route("/admin/logout", post(api::admin::logout))
        .nest("/admin/api", admin_api)
        .layer(middleware)
        .with_state(state);

    Ok(router)
}
