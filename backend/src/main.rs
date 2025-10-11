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
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    setup_tracing();

    let config = config::AppConfig::from_env().context("failed to load configuration")?;
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

fn setup_tracing() {
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(env_filter).init();
}
