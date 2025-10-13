use anyhow::{anyhow, Context};
use serde::Deserialize;
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct ServerConfig {
    pub bind: String,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind: "127.0.0.1:8080".to_string(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct DbConfig {
    pub url: String,
    pub max_connections: u32,
}

impl Default for DbConfig {
    fn default() -> Self {
        Self {
            url: String::new(),
            max_connections: 5,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct FetcherConfig {
    pub interval_secs: u64,
    pub batch_size: u32,
    pub concurrency: u32,
    pub request_timeout_secs: u64,
}

impl Default for FetcherConfig {
    fn default() -> Self {
        Self {
            interval_secs: 300,
            batch_size: 8,
            concurrency: 4,
            request_timeout_secs: 15,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct LoggingConfig {
    pub file: String,
    pub level: Option<String>,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            file: "logs/backend.log".to_string(),
            level: Some("info".to_string()),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct DeepseekConfig {
    pub api_key: Option<String>,
    pub base_url: String,
    pub model: String,
    pub timeout_secs: u64,
}

impl Default for DeepseekConfig {
    fn default() -> Self {
        Self {
            api_key: None,
            base_url: "https://api.deepseek.com".to_string(),
            model: "deepseek-chat".to_string(),
            timeout_secs: 30,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct QdrantConfig {
    pub enabled: bool,
    pub uri: String,
    pub api_key: Option<String>,
    pub collection: String,
    pub vector_size: u64,
}

impl Default for QdrantConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            uri: "http://localhost:6334".to_string(),
            api_key: None,
            collection: "news_articles".to_string(),
            vector_size: 384,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct AiConfig {
    pub deepseek: DeepseekConfig,
    pub qdrant: QdrantConfig,
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            deepseek: DeepseekConfig::default(),
            qdrant: QdrantConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub db: DbConfig,
    pub fetcher: FetcherConfig,
    pub logging: LoggingConfig,
    pub ai: AiConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig::default(),
            db: DbConfig::default(),
            fetcher: FetcherConfig::default(),
            logging: LoggingConfig::default(),
            ai: AiConfig::default(),
        }
    }
}

impl AppConfig {
    pub fn from_env() -> anyhow::Result<Self> {
        let explicit_path = std::env::var("CONFIG_FILE").ok();
        let config = if let Some(path) = explicit_path {
            let path = PathBuf::from(path);
            if !path.exists() {
                return Err(anyhow!("config file {:?} not found", path));
            }
            Self::load_from_file(&path)?
        } else {
            let path = locate_default_config();
            if let Some(path) = path {
                Self::load_from_file(&path)?
            } else {
                AppConfig::default()
            }
        };

        Self::apply_env_overrides(config)
    }

    fn load_from_file(path: &Path) -> anyhow::Result<Self> {
        let contents = fs::read_to_string(path)
            .with_context(|| format!("failed to read config file {:?}", path))?;
        let config: AppConfig = serde_yaml::from_str(&contents)
            .with_context(|| format!("failed to parse config file {:?}", path))?;
        Ok(config)
    }

    fn apply_env_overrides(mut config: AppConfig) -> anyhow::Result<AppConfig> {
        if let Ok(bind) = std::env::var("SERVER_BIND") {
            config.server.bind = bind;
        }

        if let Ok(url) = std::env::var("DATABASE_URL") {
            config.db.url = url;
        }

        if let Some(max_conn) = parse_optional_env("DB_MAX_CONNECTIONS")? {
            config.db.max_connections = max_conn;
        }

        if let Some(interval) = parse_optional_env("FETCH_INTERVAL_SECS")? {
            config.fetcher.interval_secs = interval;
        }

        if let Some(batch) = parse_optional_env("FETCH_BATCH_SIZE")? {
            config.fetcher.batch_size = batch;
        }

        if let Some(concurrency) = parse_optional_env("FETCH_CONCURRENCY")? {
            config.fetcher.concurrency = concurrency;
        }

        if let Some(timeout) = parse_optional_env("FETCH_TIMEOUT_SECS")? {
            config.fetcher.request_timeout_secs = timeout;
        }

        if let Ok(log_file) = std::env::var("LOG_FILE_PATH") {
            config.logging.file = log_file;
        }

        if let Ok(log_level) = std::env::var("LOG_LEVEL") {
            config.logging.level = Some(log_level);
        }

        if let Ok(api_key) = std::env::var("DEEPSEEK_API_KEY") {
            config.ai.deepseek.api_key = Some(api_key);
        }

        if let Ok(base_url) = std::env::var("DEEPSEEK_BASE_URL") {
            config.ai.deepseek.base_url = base_url;
        }

        if let Ok(model) = std::env::var("DEEPSEEK_MODEL") {
            config.ai.deepseek.model = model;
        }

        if let Some(timeout) = parse_optional_env("DEEPSEEK_TIMEOUT_SECS")? {
            config.ai.deepseek.timeout_secs = timeout;
        }

        if let Some(enabled) = parse_optional_env("QDRANT_ENABLED")? {
            config.ai.qdrant.enabled = enabled;
        }

        if let Ok(uri) = std::env::var("QDRANT_URI") {
            config.ai.qdrant.uri = uri;
        }

        if let Ok(api_key) = std::env::var("QDRANT_API_KEY") {
            config.ai.qdrant.api_key = Some(api_key);
        }

        if let Ok(collection) = std::env::var("QDRANT_COLLECTION") {
            config.ai.qdrant.collection = collection;
        }

        if let Some(vector_size) = parse_optional_env("QDRANT_VECTOR_SIZE")? {
            config.ai.qdrant.vector_size = vector_size;
        }

        if config.db.url.trim().is_empty() {
            return Err(anyhow!(
                "database url missing; set DATABASE_URL env var or db.url in config file"
            ));
        }

        Ok(config)
    }
}

fn parse_optional_env<T>(key: &str) -> anyhow::Result<Option<T>>
where
    T: std::str::FromStr,
    T::Err: std::error::Error + Send + Sync + 'static,
{
    match std::env::var(key) {
        Ok(v) => Ok(Some(
            v.parse::<T>()
                .with_context(|| format!("{key} must be a valid value"))?,
        )),
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(err) => Err(err.into()),
    }
}

fn locate_default_config() -> Option<PathBuf> {
    let candidates = [
        PathBuf::from("config/config.yaml"),
        PathBuf::from("../config/config.yaml"),
    ];

    for path in candidates {
        if path.exists() {
            return Some(path);
        }
    }

    None
}
