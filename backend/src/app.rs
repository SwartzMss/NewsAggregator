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
    ops::events::EventsHub,
};
use crate::repo::events as repo_events;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub config: FrontendPublicConfig,
    pub admin: auth::AdminManager,
    pub http_client: HttpClientConfig,
    pub fetcher_config: FetcherConfig,
    pub translator: Arc<TranslationEngine>,
    pub events: EventsHub,
}

pub async fn build_router(config: &AppConfig) -> anyhow::Result<Router> {
    let pool = PgPoolOptions::new()
        .max_connections(config.db.max_connections)
        .acquire_timeout(Duration::from_secs(5))
        .connect(&config.db.url)
        .await?;

    repo::migrations::ensure_schema(&pool).await?;
    repo::maintenance::cleanup_orphan_content(&pool).await?;

    // Emit a simple system startup event (no source_domain)
    let _ = repo_events::upsert_event(
        &pool,
        &repo_events::NewEvent { level: "info".to_string(), code: "SYSTEM_STARTED".to_string(), source_domain: None },
        0,
    ).await;

    // Normalize translation-related settings at startup:
    // - Force default provider to 'ollama'
    // - Remove deprecated Baidu settings keys if present
    if let Err(err) = async {
        // Upsert provider to 'ollama' if missing or different
        let current = repo::settings::get_setting(&pool, "translation.provider").await?;
        if current.as_deref() != Some("ollama") {
            repo::settings::upsert_setting(&pool, "translation.provider", "ollama").await?;
            tracing::info!(old = current.as_deref().unwrap_or("<none>"), new = "ollama", "normalized translation.provider");
        }
        // Clean deprecated keys (safe no-op if absent)
        let _ = repo::settings::delete_setting(&pool, "translation.baidu_app_id").await;
        let _ = repo::settings::delete_setting(&pool, "translation.baidu_secret_key").await;
        Ok::<(), anyhow::Error>(())
    }
    .await {
        tracing::warn!(error = %err, "failed to normalize translation settings at startup");
    }

    let translator = Arc::new(TranslationEngine::new(
        &config.http_client,
    )?);

    let stored_deepseek_key =
        repo::settings::get_setting(&pool, "translation.deepseek_api_key").await?;
    let stored_ollama_base_url =
        repo::settings::get_setting(&pool, "translation.ollama_base_url").await?;
    let stored_ollama_model =
        repo::settings::get_setting(&pool, "translation.ollama_model").await?;
    let stored_translation_enabled =
        repo::settings::get_setting(&pool, "translation.enabled").await?;

    translator.update_credentials(TranslatorCredentialsUpdate {
        deepseek_api_key: stored_deepseek_key,
        ollama_base_url: stored_ollama_base_url,
        ollama_model: stored_ollama_model,
        translation_enabled: stored_translation_enabled.as_ref().and_then(|v| {
            match v.trim().to_ascii_lowercase().as_str() {
                "true" | "1" | "yes" | "on" => Some(true),
                "false" | "0" | "no" | "off" => Some(false),
                _ => None,
            }
        }),
        ..Default::default()
    })?;

    if let Some(saved_provider) = repo::settings::get_setting(&pool, "translation.provider").await?
    {
        tracing::info!("loaded translator provider from database: {}", saved_provider);
        
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
    } else {
        tracing::info!("no translator provider configured, translation disabled");
    }

    // init events hub early so background tasks can broadcast
    let events_hub = EventsHub::new(256);

    fetcher::spawn(
        pool.clone(),
        config.fetcher.clone(),
        config.http_client.clone(),
        Arc::clone(&translator),
        events_hub.clone(),
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
        events: events_hub,
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
        .route("/alerts", get(api::alerts::list_alerts).delete(api::alerts::delete_alerts))
        .route("/alerts/stream", get(api::alerts::stream_alerts))
        .route(
            "/settings/translation",
            get(api::settings::get_translation_settings)
                .post(api::settings::update_translation_settings),
        )
        .route(
            "/settings/models",
            get(api::settings::get_model_settings)
                .post(api::settings::update_model_settings),
        )
        .route(
            "/settings/models/test",
            post(api::settings::test_model_connectivity),
        )
        .route(
            "/settings/ai_dedup",
            get(api::settings::get_ai_dedup_settings)
                .post(api::settings::update_ai_dedup_settings),
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
