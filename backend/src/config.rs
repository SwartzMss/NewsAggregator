use anyhow::{anyhow, Context};
use serde::{Deserialize, Serialize};
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
    pub quick_retry_attempts: u32,
    pub quick_retry_delay_secs: u64,
}

impl Default for FetcherConfig {
    fn default() -> Self {
        Self {
            interval_secs: 300,
            batch_size: 8,
            concurrency: 4,
            request_timeout_secs: 15,
            quick_retry_attempts: 1,
            quick_retry_delay_secs: 10,
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
pub struct HttpClientConfig {
    pub http_proxy: Option<String>,
    pub https_proxy: Option<String>,
}

impl Default for HttpClientConfig {
    fn default() -> Self {
        let default_proxy = Some("http://172.20.160.1:7890".to_string());
        Self {
            http_proxy: default_proxy.clone(),
            https_proxy: default_proxy,
        }
    }
}

impl HttpClientConfig {
    pub fn apply(&self, builder: reqwest::ClientBuilder) -> anyhow::Result<reqwest::ClientBuilder> {
        let mut builder = builder;

        if let Some(ref proxy) = self.http_proxy {
            builder = builder.proxy(
                reqwest::Proxy::http(proxy)
                    .with_context(|| format!("failed to parse http proxy url: {proxy}"))?,
            );
        }

        if let Some(ref proxy) = self.https_proxy {
            builder = builder.proxy(
                reqwest::Proxy::https(proxy)
                    .with_context(|| format!("failed to parse https proxy url: {proxy}"))?,
            );
        }

        Ok(builder)
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
pub struct BaiduTranslatorConfig {
    pub app_id: Option<String>,
    pub secret_key: Option<String>,
}

impl Default for BaiduTranslatorConfig {
    fn default() -> Self {
        Self {
            app_id: None,
            secret_key: None,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct AiConfig {
    pub deepseek: DeepseekConfig,
    pub ollama: OllamaConfig,
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            deepseek: DeepseekConfig::default(),
            ollama: OllamaConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct OllamaConfig {
    pub base_url: String,
    pub model: String,
    pub timeout_secs: u64,
}

impl Default for OllamaConfig {
    fn default() -> Self {
        Self {
            base_url: "http://127.0.0.1:11434".to_string(),
            model: "qwen2.5:3b".to_string(),
            timeout_secs: 30,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct TranslatorConfig {
    pub provider: String,
    pub baidu: BaiduTranslatorConfig,
}

impl Default for TranslatorConfig {
    fn default() -> Self {
        Self {
            provider: "deepseek".to_string(),
            baidu: BaiduTranslatorConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct AdminConfig {
    pub username: String,
    pub password: String,
    pub session_ttl_secs: u64,
}

impl Default for AdminConfig {
    fn default() -> Self {
        Self {
            username: "admin".to_string(),
            password: "123456".to_string(),
            session_ttl_secs: 300,
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
    pub http_client: HttpClientConfig,
    pub ai: AiConfig,
    pub translator: TranslatorConfig,
    pub deployment: DeploymentConfig,
    pub admin: AdminConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig::default(),
            db: DbConfig::default(),
            fetcher: FetcherConfig::default(),
            logging: LoggingConfig::default(),
            http_client: HttpClientConfig::default(),
            ai: AiConfig::default(),
            translator: TranslatorConfig::default(),
            deployment: DeploymentConfig::default(),
            admin: AdminConfig::default(),
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

        if let Some(attempts) = parse_optional_env("FETCH_QUICK_RETRY_ATTEMPTS")? {
            config.fetcher.quick_retry_attempts = attempts;
        }

        if let Some(delay) = parse_optional_env("FETCH_QUICK_RETRY_DELAY_SECS")? {
            config.fetcher.quick_retry_delay_secs = delay;
        }

        if let Ok(proxy) = std::env::var("HTTP_PROXY") {
            config.http_client.http_proxy = Some(proxy);
        }

        if let Ok(proxy) = std::env::var("HTTPS_PROXY") {
            config.http_client.https_proxy = Some(proxy);
        }

        if let Ok(provider) = std::env::var("TRANSLATOR_PROVIDER") {
            if !provider.trim().is_empty() {
                config.translator.provider = provider;
            }
        }

        if let Ok(app_id) = std::env::var("BAIDU_APP_ID") {
            if !app_id.trim().is_empty() {
                config.translator.baidu.app_id = Some(app_id);
            }
        }

        if let Ok(secret) = std::env::var("BAIDU_SECRET_KEY") {
            if !secret.trim().is_empty() {
                config.translator.baidu.secret_key = Some(secret);
            }
        }

        if let Ok(log_file) = std::env::var("LOG_FILE_PATH") {
            config.logging.file = log_file;
        }

        if let Ok(log_level) = std::env::var("LOG_LEVEL") {
            config.logging.level = Some(log_level);
        }

        if let Ok(base_url) = std::env::var("OLLAMA_BASE_URL") {
            if !base_url.trim().is_empty() {
                config.ai.ollama.base_url = base_url;
            }
        }

        if let Ok(model) = std::env::var("OLLAMA_MODEL") {
            if !model.trim().is_empty() {
                config.ai.ollama.model = model;
            }
        }

        if let Some(timeout) = parse_optional_env("OLLAMA_TIMEOUT_SECS")? {
            config.ai.ollama.timeout_secs = timeout;
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

        if let Ok(admin_username) = std::env::var("ADMIN_USERNAME") {
            if !admin_username.trim().is_empty() {
                config.admin.username = admin_username;
            }
        }

        if let Ok(admin_password) = std::env::var("ADMIN_PASSWORD") {
            if !admin_password.trim().is_empty() {
                config.admin.password = admin_password;
            }
        }

        if let Some(ttl) = parse_optional_env::<u64>("ADMIN_SESSION_TTL_SECS")? {
            config.admin.session_ttl_secs = ttl.max(60);
        }

        if config.db.url.trim().is_empty() {
            return Err(anyhow!(
                "database url missing; set DATABASE_URL env var or db.url in config file"
            ));
        }

        config.http_client.http_proxy = normalize_proxy(config.http_client.http_proxy.take());
        config.http_client.https_proxy = normalize_proxy(config.http_client.https_proxy.take());
        config.translator.provider = normalize_provider(&config.translator.provider);
        config.translator.baidu.app_id = config
            .translator
            .baidu
            .app_id
            .take()
            .and_then(|v| normalize_optional_string(v));
        config.translator.baidu.secret_key = config
            .translator
            .baidu
            .secret_key
            .take()
            .and_then(|v| normalize_optional_string(v));

        Ok(config)
    }

    pub fn frontend_public_config(&self) -> FrontendPublicConfig {
        let mut candidates: Vec<String> = Vec::new();

        if let Ok(env_override) = std::env::var("PUBLIC_API_BASE_URL") {
            if !env_override.trim().is_empty() {
                candidates.push(env_override);
            }
        }

        if let Some(explicit) = self
            .deployment
            .public_api_base_url
            .as_ref()
            .map(|v| v.trim())
            .filter(|v| !v.is_empty())
        {
            candidates.push(explicit.to_string());
        }

        if let Some(domain) = self
            .deployment
            .domain
            .as_ref()
            .map(|v| v.trim())
            .filter(|v| !v.is_empty())
        {
            let scheme = if self.deployment.ssl_enabled() {
                "https://"
            } else {
                "http://"
            };
            let base = if domain.starts_with("http://") || domain.starts_with("https://") {
                domain.to_string()
            } else {
                format!("{scheme}{domain}")
            };
            candidates.push(base);
        }

        if let Some(bind_addr) = self
            .deployment
            .backend
            .bind_addr
            .as_ref()
            .map(|v| v.trim())
            .filter(|v| !v.is_empty())
        {
            candidates.push(format_host_base(bind_addr));
        } else if !self.server.bind.trim().is_empty() {
            candidates.push(format_host_base(&self.server.bind));
        }

        let base = candidates
            .into_iter()
            .map(|candidate| candidate.trim_end_matches('/').to_string())
            .find(|candidate| !candidate.is_empty())
            .unwrap_or_else(|| "http://127.0.0.1:8081".to_string());

        FrontendPublicConfig {
            api_base_url: ensure_api_suffix(&base),
        }
    }
}

fn normalize_proxy(value: Option<String>) -> Option<String> {
    value.and_then(|v| {
        let trimmed = v.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn normalize_provider(value: &str) -> String {
    let normalized = value.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        "deepseek".to_string()
    } else {
        normalized
    }
}

fn normalize_optional_string(value: String) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
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

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct DeploymentConfig {
    pub domain: Option<String>,
    pub public_api_base_url: Option<String>,
    pub backend: DeploymentBackendConfig,
    pub ssl: Option<SslConfig>,
}

impl DeploymentConfig {
    fn ssl_enabled(&self) -> bool {
        self.ssl
            .as_ref()
            .map(|ssl| !ssl.cert_path.trim().is_empty() && !ssl.key_path.trim().is_empty())
            .unwrap_or(false)
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct DeploymentBackendConfig {
    pub bind_addr: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct SslConfig {
    pub cert_path: String,
    pub key_path: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct FrontendPublicConfig {
    pub api_base_url: String,
}

fn format_host_base(host: &str) -> String {
    let trimmed = host.trim();
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        trimmed.to_string()
    } else {
        format!("http://{trimmed}")
    }
}

fn ensure_api_suffix(base: &str) -> String {
    let normalized = base.trim_end_matches('/');
    if normalized.ends_with("/api") {
        normalized.to_string()
    } else {
        format!("{normalized}/api")
    }
}
