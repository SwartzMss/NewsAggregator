use anyhow::Context;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    pub bind: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DbConfig {
    pub url: String,
    pub max_connections: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FetcherConfig {
    pub interval_secs: u64,
    pub batch_size: u32,
    pub concurrency: u32,
    pub request_timeout_secs: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LoggingConfig {
    pub file: String,
    pub level: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub db: DbConfig,
    pub fetcher: FetcherConfig,
    pub logging: LoggingConfig,
}

impl AppConfig {
    pub fn from_env() -> anyhow::Result<Self> {
        let bind = std::env::var("SERVER_BIND").unwrap_or_else(|_| "127.0.0.1:8080".to_string());
        let url = std::env::var("DATABASE_URL").context("DATABASE_URL must be set")?;
        let max_connections = parse_env("DB_MAX_CONNECTIONS", 5)?;
        let interval_secs = parse_env("FETCH_INTERVAL_SECS", 300)?;
        let batch_size = parse_env("FETCH_BATCH_SIZE", 8)?;
        let concurrency = parse_env("FETCH_CONCURRENCY", 4)?;
        let request_timeout_secs = parse_env("FETCH_TIMEOUT_SECS", 15)?;
        let log_file =
            std::env::var("LOG_FILE_PATH").unwrap_or_else(|_| "logs/backend.log".to_string());
        let log_level = std::env::var("LOG_LEVEL").ok();

        Ok(Self {
            server: ServerConfig { bind },
            db: DbConfig {
                url,
                max_connections,
            },
            fetcher: FetcherConfig {
                interval_secs,
                batch_size,
                concurrency,
                request_timeout_secs,
            },
            logging: LoggingConfig {
                file: log_file,
                level: log_level,
            },
        })
    }
}

fn parse_env<T>(key: &str, default: T) -> anyhow::Result<T>
where
    T: std::str::FromStr,
    T::Err: std::error::Error + Send + Sync + 'static,
{
    match std::env::var(key) {
        Ok(v) => Ok(v
            .parse::<T>()
            .with_context(|| format!("{key} must be a valid value"))?),
        Err(_) => Ok(default),
    }
}
