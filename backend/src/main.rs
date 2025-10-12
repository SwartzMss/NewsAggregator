mod api;
mod app;
mod config;
mod error;
mod fetcher;
mod model;
mod repo;
mod service;
mod util;

use anyhow::Context;
use std::{net::SocketAddr, path::Path, sync::OnceLock};
use tokio::net::TcpListener;
use tracing_appender::rolling;
use tracing_subscriber::{fmt::layer as fmt_layer, prelude::*, EnvFilter, Registry};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = config::AppConfig::from_env().context("failed to load configuration")?;
    setup_tracing(&config)?;
    let addr: SocketAddr = config
        .server
        .bind
        .parse()
        .context("invalid SERVER_BIND address")?;

    tracing::info!(%addr, "starting server");

    let app = app::build_router(&config).await?;
    let listener = TcpListener::bind(addr).await?;

    axum::serve(listener, app).await.context("server failed")?;

    Ok(())
}

fn setup_tracing(config: &config::AppConfig) -> anyhow::Result<()> {
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        let level = config
            .logging
            .level
            .clone()
            .unwrap_or_else(|| "info".to_string());
        EnvFilter::new(level)
    });

    let log_path = Path::new(&config.logging.file);
    if let Some(parent) = log_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let file_name = log_path
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(|| anyhow::anyhow!("invalid log file path"))?;
    let directory = log_path
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| Path::new(".").to_path_buf());

    let file_appender = rolling::never(directory, file_name);
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    static FILE_GUARD: OnceLock<tracing_appender::non_blocking::WorkerGuard> = OnceLock::new();
    let _ = FILE_GUARD.set(guard);

    let stdout_layer = fmt_layer()
        .with_writer(std::io::stdout)
        .with_file(true)
        .with_line_number(true);
    let file_layer = fmt_layer()
        .with_writer(non_blocking)
        .with_ansi(false)
        .with_file(true)
        .with_line_number(true);

    Registry::default()
        .with(env_filter)
        .with(stdout_layer)
        .with(file_layer)
        .try_init()
        .context("failed to init tracing subscriber")?;

    Ok(())
}
