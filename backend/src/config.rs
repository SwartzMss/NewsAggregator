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

// Baidu translator support removed


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
            deployment: DeploymentConfig::default(),
            admin: AdminConfig::default(),
        }
    }
}

impl AppConfig {
    /// 从默认的配置文件搜索路径加载配置（不读取任何环境变量）。
    pub fn load() -> anyhow::Result<Self> {
        if let Some(path) = locate_default_config() {
            Self::load_from_file(&path)
        } else {
            Ok(AppConfig::default())
        }
    }

    /// 从指定的文件路径显式加载配置。
    // 删除未用到的 API（可从 Git 历史恢复）

    fn load_from_file(path: &Path) -> anyhow::Result<Self> {
        let contents = fs::read_to_string(path)
            .with_context(|| format!("failed to read config file {:?}", path))?;
        let config: AppConfig = serde_yaml::from_str(&contents)
            .with_context(|| format!("failed to parse config file {:?}", path))?;
        Ok(config)
    }

    pub fn frontend_public_config(&self) -> FrontendPublicConfig {
    // 依次收集可能的外部可访问 API 基础地址候选，然后选取第一个有效的。
    let mut candidates: Vec<String> = Vec::new();


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

// 规范化代理地址：空字符串或全空白会被转换为 None。
// 删除未使用的辅助函数（可从 Git 历史恢复）



// 查找默认配置文件路径，按顺序返回第一个存在的路径。
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

// 将主机或地址补全为带协议的形式，未指定协议时默认使用 http。
fn format_host_base(host: &str) -> String {
    let trimmed = host.trim();
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        trimmed.to_string()
    } else {
        format!("http://{trimmed}")
    }
}

// 确保基础 URL 末尾包含 /api 后缀，避免前端使用时自行拼接。
fn ensure_api_suffix(base: &str) -> String {
    let normalized = base.trim_end_matches('/');
    if normalized.ends_with("/api") {
        normalized.to_string()
    } else {
        format!("{normalized}/api")
    }
}
