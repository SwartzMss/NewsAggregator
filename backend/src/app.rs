use std::{sync::Arc, time::Duration};

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
    config::{AppConfig, FetcherConfig, FrontendPublicConfig, HttpClientConfig},
    fetcher, repo,
    util::translator::{TranslationEngine, TranslatorCredentialsUpdate, TranslatorProvider},
};

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub config: FrontendPublicConfig,
    pub admin: auth::AdminManager,
    pub http_client: HttpClientConfig,
    pub fetcher_config: FetcherConfig,
    pub translator: Arc<TranslationEngine>,
}

pub async fn build_router(config: &AppConfig) -> anyhow::Result<Router> {
    let pool = PgPoolOptions::new()
        .max_connections(config.db.max_connections)
        .acquire_timeout(Duration::from_secs(5))
        .connect(&config.db.url)
        .await?;

    repo::migrations::ensure_schema(&pool).await?;
    repo::maintenance::cleanup_orphan_content(&pool).await?;

    let translator = Arc::new(TranslationEngine::new(
        &config.http_client,
        &config.translator,
        &config.ai,
    )?);

    let stored_baidu_app_id =
        repo::settings::get_setting(&pool, "translation.baidu_app_id").await?;
    let stored_baidu_secret =
        repo::settings::get_setting(&pool, "translation.baidu_secret_key").await?;
    let stored_deepseek_key =
        repo::settings::get_setting(&pool, "translation.deepseek_api_key").await?;
    let stored_translate_descriptions =
        repo::settings::get_setting(&pool, "translation.translate_descriptions").await?;
    let translate_flag = stored_translate_descriptions.as_ref().and_then(|value| {
        match value.trim().to_ascii_lowercase().as_str() {
            "true" | "1" | "yes" | "on" => Some(true),
            "false" | "0" | "no" | "off" => Some(false),
            _ => None,
        }
    });

    translator.update_credentials(TranslatorCredentialsUpdate {
        baidu_app_id: stored_baidu_app_id,
        baidu_secret_key: stored_baidu_secret,
        deepseek_api_key: stored_deepseek_key,
        translate_descriptions: translate_flag,
        ..Default::default()
    })?;

    if let Some(saved_provider) = repo::settings::get_setting(&pool, "translation.provider").await?
    {
        match saved_provider.parse::<TranslatorProvider>() {
            Ok(provider) => {
                if let Err(err) = translator.set_provider(provider) {
                    tracing::warn!(
                        provider = provider.as_str(),
                        error = %err,
                        "translator provider from settings not available, using default"
                    );
                }
            }
            Err(err) => {
                tracing::warn!(
                    saved = saved_provider,
                    error = %err,
                    "invalid translator provider stored in settings"
                );
            }
        }
    }

    fetcher::spawn(
        pool.clone(),
        config.fetcher.clone(),
        config.http_client.clone(),
        Arc::clone(&translator),
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
        fetcher_config: config.fetcher.clone(),
        translator,
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
        .route(
            "/settings/translation",
            get(api::settings::get_translation_settings)
                .post(api::settings::update_translation_settings),
        )
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
