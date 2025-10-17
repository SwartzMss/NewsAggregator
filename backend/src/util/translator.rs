use std::sync::{Arc, RwLock};

use anyhow::{anyhow, Result};
use tokio::runtime::Handle;
use std::time::Instant;
use tracing::{info, warn};

use crate::config::HttpClientConfig;

use super::{
    baidu::BaiduTranslator,
    deepseek::{DeepseekClient, TranslationResult},
    ollama::OllamaClient,
};

const VERIFICATION_SAMPLE_TEXT: &str = "NewsAggregator ping";
const PROVIDER_PRIORITY: [TranslatorProvider; 3] = [
    TranslatorProvider::Deepseek,
    TranslatorProvider::Baidu,
    TranslatorProvider::Ollama,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TranslatorProvider {
    Deepseek,
    Baidu,
    Ollama,
}

fn clear_verification(state: &mut TranslationState, provider: TranslatorProvider) {
    match provider {
        TranslatorProvider::Baidu => {
            state.baidu_verified = false;
            state.baidu_error = None;
        }
        TranslatorProvider::Deepseek => {
            state.deepseek_verified = false;
            state.deepseek_error = None;
        }
        TranslatorProvider::Ollama => {
            state.ollama_verified = false;
            state.ollama_error = None;
        }
    }
}

fn provider_available(state: &TranslationState, provider: TranslatorProvider) -> bool {
    match provider {
        TranslatorProvider::Baidu => state.baidu_client.is_some() && state.baidu_verified,
        TranslatorProvider::Deepseek => state.deepseek_client.is_some() && state.deepseek_verified,
        TranslatorProvider::Ollama => state.ollama_client.is_some() && state.ollama_verified,
    }
}

fn ensure_provider_consistency(state: &mut TranslationState) {
    if provider_available(state, state.provider) {
        return;
    }

    if let Some(fallback) = PROVIDER_PRIORITY
        .into_iter()
        .find(|provider| provider_available(state, *provider))
    {
        state.provider = fallback;
    }
}

async fn verify_provider_credentials(
    state: Arc<RwLock<TranslationState>>,
    verify_baidu: bool,
    verify_deepseek: bool,
    verify_ollama: bool,
) {
    if !verify_baidu && !verify_deepseek && !verify_ollama {
        return;
    }

    let (baidu_client, deepseek_client, ollama_client) = {
        let mut guard = state
            .write()
            .expect("translator state poisoned before verification");

        let baidu = if verify_baidu {
            let client = guard.baidu_client.clone();
            clear_verification(&mut guard, TranslatorProvider::Baidu);
            client
        } else {
            None
        };

        let deepseek = if verify_deepseek {
            let client = guard.deepseek_client.clone();
            clear_verification(&mut guard, TranslatorProvider::Deepseek);
            client
        } else {
            None
        };

        let ollama = if verify_ollama {
            let client = guard.ollama_client.clone();
            clear_verification(&mut guard, TranslatorProvider::Ollama);
            client
        } else {
            None
        };

        (baidu, deepseek, ollama)
    };

    if verify_baidu {
        if let Some(client) = baidu_client {
            let started = Instant::now();
            info!(phase = "start", provider = "baidu", "verifying translator credentials");
            let result = client
                .translate(VERIFICATION_SAMPLE_TEXT, "auto", "zh")
                .await;

            let mut guard = state
                .write()
                .expect("translator state poisoned while updating baidu verification");
            match result {
                Ok(_) => {
                    guard.baidu_verified = true;
                    guard.baidu_error = None;
                    info!(
                        phase = "end",
                        provider = "baidu",
                        elapsed_ms = started.elapsed().as_millis() as u64,
                        "verification completed"
                    );
                }
                Err(err) => {
                    guard.baidu_verified = false;
                    guard.baidu_error = Some(truncate_error(err));
                    warn!(
                        error = guard.baidu_error.as_deref().unwrap_or_default(),
                        provider = "baidu",
                        elapsed_ms = started.elapsed().as_millis() as u64,
                        "translator credential verification failed"
                    );
                }
            }
        } else if let Ok(mut guard) = state.write() {
            clear_verification(&mut guard, TranslatorProvider::Baidu);
        }
    }

    if verify_deepseek {
        if let Some(client) = deepseek_client {
            let started = Instant::now();
            info!(phase = "start", provider = "deepseek", "verifying translator credentials");
            let result = client.translate_news(VERIFICATION_SAMPLE_TEXT, None).await;

            let mut guard = state
                .write()
                .expect("translator state poisoned while updating deepseek verification");
            match result {
                Ok(_) => {
                    guard.deepseek_verified = true;
                    guard.deepseek_error = None;
                    info!(
                        phase = "end",
                        provider = "deepseek",
                        elapsed_ms = started.elapsed().as_millis() as u64,
                        "verification completed"
                    );
                }
                Err(err) => {
                    guard.deepseek_verified = false;
                    guard.deepseek_error = Some(truncate_error(err));
                    warn!(
                        error = guard.deepseek_error.as_deref().unwrap_or_default(),
                        provider = "deepseek",
                        elapsed_ms = started.elapsed().as_millis() as u64,
                        "translator credential verification failed"
                    );
                }
            }
        } else if let Ok(mut guard) = state.write() {
            clear_verification(&mut guard, TranslatorProvider::Deepseek);
        }
    }

    if verify_ollama {
        if let Some(client) = ollama_client {
            let started = Instant::now();
            info!(phase = "start", provider = "ollama", "verifying translator connectivity");
            let result = client.translate_news(VERIFICATION_SAMPLE_TEXT, None).await;

            let mut guard = state
                .write()
                .expect("translator state poisoned while updating ollama verification");
            match result {
                Ok(_) => {
                    guard.ollama_verified = true;
                    guard.ollama_error = None;
                    info!(
                        phase = "end",
                        provider = "ollama",
                        elapsed_ms = started.elapsed().as_millis() as u64,
                        "verification completed"
                    );
                }
                Err(err) => {
                    guard.ollama_verified = false;
                    guard.ollama_error = Some(truncate_error(err));
                    warn!(
                        error = guard.ollama_error.as_deref().unwrap_or_default(),
                        provider = "ollama",
                        elapsed_ms = started.elapsed().as_millis() as u64,
                        "translator verification failed"
                    );
                }
            }
        } else if let Ok(mut guard) = state.write() {
            clear_verification(&mut guard, TranslatorProvider::Ollama);
        }
    }

    if verify_baidu || verify_deepseek || verify_ollama {
        if let Ok(mut guard) = state.write() {
            ensure_provider_consistency(&mut guard);
        }
    }
}

fn truncate_error<E: std::fmt::Display>(err: E) -> String {
    let mut message = err.to_string();
    const MAX_LEN: usize = 200;
    if message.len() > MAX_LEN {
        message.truncate(MAX_LEN);
    }
    message
}

impl TranslatorProvider {
    pub fn as_str(&self) -> &'static str {
        match self {
            TranslatorProvider::Deepseek => "deepseek",
            TranslatorProvider::Baidu => "baidu",
            TranslatorProvider::Ollama => "ollama",
        }
    }
}

impl std::str::FromStr for TranslatorProvider {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "deepseek" => Ok(TranslatorProvider::Deepseek),
            "baidu" => Ok(TranslatorProvider::Baidu),
            "ollama" => Ok(TranslatorProvider::Ollama),
            other => Err(anyhow!("unsupported translator provider: {other}")),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum TranslationError {
    #[error("translator not configured")]
    NotConfigured,
    #[error("translator quota exceeded")]
    QuotaExceeded,
    #[error("translator api error {code}: {message}")]
    Api { code: String, message: String },
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl TranslationError {
    fn into_anyhow(self) -> anyhow::Error {
        match self {
            TranslationError::NotConfigured => anyhow!("translator not configured"),
            TranslationError::QuotaExceeded => anyhow!("translator quota exceeded"),
            TranslationError::Api { code, message } => {
                anyhow!("translator api error {code}: {message}")
            }
            TranslationError::Other(err) => err,
        }
    }
}

#[derive(Clone)]
pub struct TranslationEngine {
    state: Arc<RwLock<TranslationState>>,
    http_config: HttpClientConfig,
    base_deepseek: DeepseekBaseConfig,
    base_ollama: Arc<RwLock<OllamaBaseConfig>>,
}

struct TranslationState {
    provider: TranslatorProvider,
    baidu_app_id: Option<String>,
    baidu_secret_key: Option<String>,
    baidu_client: Option<Arc<BaiduTranslator>>,
    baidu_verified: bool,
    baidu_error: Option<String>,
    deepseek_api_key: Option<String>,
    deepseek_client: Option<Arc<DeepseekClient>>,
    deepseek_verified: bool,
    deepseek_error: Option<String>,
    ollama_client: Option<Arc<OllamaClient>>,
    ollama_verified: bool,
    ollama_error: Option<String>,
    translate_descriptions: bool,
}

#[derive(Debug, Clone)]
struct DeepseekBaseConfig {
    base_url: String,
    model: String,
    timeout_secs: u64,
}

#[derive(Debug, Clone)]
struct OllamaBaseConfig {
    base_url: String,
    model: String,
    timeout_secs: u64,
}

#[derive(Debug, Default)]
pub struct TranslatorCredentialsUpdate {
    pub provider: Option<TranslatorProvider>,
    pub baidu_app_id: Option<String>,
    pub baidu_secret_key: Option<String>,
    pub deepseek_api_key: Option<String>,
    pub ollama_base_url: Option<String>,
    pub ollama_model: Option<String>,
    pub translate_descriptions: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct TranslatorSnapshot {
    pub provider: TranslatorProvider,
    pub baidu_configured: bool,
    pub deepseek_configured: bool,
    pub ollama_configured: bool,
    pub baidu_app_id_masked: Option<String>,
    pub baidu_secret_key_masked: Option<String>,
    pub deepseek_api_key_masked: Option<String>,
    pub baidu_error: Option<String>,
    pub deepseek_error: Option<String>,
    pub ollama_error: Option<String>,
    pub ollama_base_url: Option<String>,
    pub ollama_model: Option<String>,
    pub translate_descriptions: bool,
}

impl TranslationEngine {
    pub fn new(
        http_client: &HttpClientConfig,
    ) -> Result<Self> {
        let mut state = TranslationState {
            provider: TranslatorProvider::Deepseek, // 默认提供商，但不会被使用直到从数据库加载
            baidu_app_id: None, // 不从配置文件读取，仅从数据库读取
            baidu_secret_key: None, // 不从配置文件读取，仅从数据库读取
            baidu_client: None,
            baidu_verified: false,
            baidu_error: None,
            deepseek_api_key: None, // 不从配置文件读取，仅从数据库读取
            deepseek_client: None,
            deepseek_verified: false,
            deepseek_error: None,
            ollama_client: None,
            ollama_verified: false,
            ollama_error: None,
            translate_descriptions: false,
        };

        let base_deepseek = DeepseekBaseConfig {
            base_url: "https://api.deepseek.com".to_string(),
            model: "deepseek-chat".to_string(),
            timeout_secs: 30,
        };

        // 不再从配置文件/环境变量读取 Ollama，默认留空，待数据库（管理后台）写入后启用
        let base_ollama = Arc::new(RwLock::new(OllamaBaseConfig {
            base_url: String::new(),
            model: String::new(),
            timeout_secs: 30,
        }));

        // attempt to build clients
        state.baidu_client = build_baidu_client(http_client, &state)?;
        state.deepseek_client = build_deepseek_client(http_client, &base_deepseek, &state)?;
        // 初始不构建 Ollama 客户端，待 settings 注入后再构建
        state.ollama_client = None;
        clear_verification(&mut state, TranslatorProvider::Baidu);
        clear_verification(&mut state, TranslatorProvider::Deepseek);
        clear_verification(&mut state, TranslatorProvider::Ollama);

        ensure_provider_consistency(&mut state);

        let verify_baidu = state.baidu_client.is_some();
        let verify_deepseek = state.deepseek_client.is_some();
        let verify_ollama = state.ollama_client.is_some();

        

        let state_lock = Arc::new(RwLock::new(state));

        let engine = Self {
            state: Arc::clone(&state_lock),
            http_config: http_client.clone(),
            base_deepseek,
            base_ollama,
        };

        engine.spawn_verification_tasks(verify_baidu, verify_deepseek, verify_ollama);

        Ok(engine)
    }

    #[allow(dead_code)]
    pub fn current_provider(&self) -> TranslatorProvider {
        self.state
            .read()
            .map(|state| state.provider)
            .unwrap_or(TranslatorProvider::Deepseek)
    }

    pub fn set_provider(&self, provider: TranslatorProvider) -> Result<()> {
        let mut guard = self
            .state
            .write()
            .map_err(|_| anyhow!("failed to acquire translator state lock"))?;

        let available = provider_available(&guard, provider);
        let has_client = match provider {
            TranslatorProvider::Baidu => guard.baidu_client.is_some(),
            TranslatorProvider::Deepseek => guard.deepseek_client.is_some(),
            TranslatorProvider::Ollama => guard.ollama_client.is_some(),
        };

        if !has_client {
            return Err(anyhow!("translator provider {:?} unavailable", provider));
        }

        guard.provider = provider;
        drop(guard);

        if !available {
            match provider {
                TranslatorProvider::Baidu => self.spawn_verification_tasks(true, false, false),
                TranslatorProvider::Deepseek => self.spawn_verification_tasks(false, true, false),
                TranslatorProvider::Ollama => self.spawn_verification_tasks(false, false, true),
            }
        }

        Ok(())
    }

    pub fn available_providers(&self) -> Vec<TranslatorProvider> {
        self.state
            .read()
            .map(|state| {
                PROVIDER_PRIORITY
                    .into_iter()
                    .filter(|provider| provider_available(&state, *provider))
                    .collect()
            })
            .unwrap_or_default()
    }

    #[allow(dead_code)]
    pub fn is_baidu_available(&self) -> bool {
        self.state
            .read()
            .map(|state| provider_available(&state, TranslatorProvider::Baidu))
            .unwrap_or(false)
    }

    #[allow(dead_code)]
    pub fn is_deepseek_available(&self) -> bool {
        self.state
            .read()
            .map(|state| provider_available(&state, TranslatorProvider::Deepseek))
            .unwrap_or(false)
    }

    pub fn deepseek_client(&self) -> Option<Arc<DeepseekClient>> {
        self.state
            .read()
            .ok()
            .and_then(|state| state.deepseek_client.as_ref().map(Arc::clone))
    }

    pub fn ollama_client(&self) -> Option<Arc<OllamaClient>> {
        self.state
            .read()
            .ok()
            .and_then(|state| state.ollama_client.as_ref().map(Arc::clone))
    }

    fn spawn_verification_tasks(
        &self,
        verify_baidu: bool,
        verify_deepseek: bool,
        verify_ollama: bool,
    ) {
        if !verify_baidu && !verify_deepseek && !verify_ollama {
            return;
        }

        let state = Arc::clone(&self.state);
        match Handle::try_current() {
            Ok(handle) => {
                handle.spawn(async move {
                    verify_provider_credentials(
                        state,
                        verify_baidu,
                        verify_deepseek,
                        verify_ollama,
                    )
                    .await;
                });
            }
            Err(error) => {
                warn!(
                    error = %error,
                    "unable to spawn translator credential verification task"
                );
                if let Ok(mut guard) = state.write() {
                    if verify_baidu && guard.baidu_client.is_some() {
                        guard.baidu_verified = false;
                        guard.baidu_error = Some("无法执行凭据验证任务".to_string());
                    }
                    if verify_deepseek && guard.deepseek_client.is_some() {
                        guard.deepseek_verified = false;
                        guard.deepseek_error = Some("无法执行凭据验证任务".to_string());
                    }
                    if verify_ollama && guard.ollama_client.is_some() {
                        guard.ollama_verified = false;
                        guard.ollama_error = Some("无法执行凭据验证任务".to_string());
                    }
                }
            }
        }
    }

    pub fn snapshot(&self) -> TranslatorSnapshot {
        let state = self.state.read().expect("translator state poisoned");
        let base_ollama = self
            .base_ollama
            .read()
            .expect("ollama base config poisoned during snapshot");
        let ollama_base_url = if base_ollama.base_url.trim().is_empty() {
            None
        } else {
            Some(base_ollama.base_url.clone())
        };
        let ollama_model = if base_ollama.model.trim().is_empty() {
            None
        } else {
            Some(base_ollama.model.clone())
        };

        TranslatorSnapshot {
            provider: state.provider,
            baidu_configured: state.baidu_client.is_some() && state.baidu_verified,
            deepseek_configured: state.deepseek_client.is_some() && state.deepseek_verified,
            ollama_configured: state.ollama_client.is_some() && state.ollama_verified,
            baidu_app_id_masked: state.baidu_app_id.as_ref().map(|value| mask_secret(value)),
            baidu_secret_key_masked: state
                .baidu_secret_key
                .as_ref()
                .map(|value| mask_secret(value)),
            deepseek_api_key_masked: state
                .deepseek_api_key
                .as_ref()
                .map(|value| mask_secret(value)),
            baidu_error: state.baidu_error.clone(),
            deepseek_error: state.deepseek_error.clone(),
            ollama_error: state.ollama_error.clone(),
            ollama_base_url,
            ollama_model,
            translate_descriptions: state.translate_descriptions,
        }
    }

    pub fn translate_descriptions(&self) -> bool {
        self.state
            .read()
            .map(|state| state.translate_descriptions)
            .unwrap_or(false)
    }

    pub fn update_credentials(&self, update: TranslatorCredentialsUpdate) -> Result<()> {
        let mut state = self
            .state
            .write()
            .map_err(|_| anyhow!("failed to acquire translator state lock"))?;

        let mut baidu_changed = false;
        let mut deepseek_changed = false;
        let mut ollama_changed = false;

        if let Some(app_id) = update.baidu_app_id {
            let trimmed = app_id.trim().to_string();
            let new_value = if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            };
            if state.baidu_app_id != new_value {
                baidu_changed = true;
            }
            state.baidu_app_id = new_value;
        }

        if let Some(secret) = update.baidu_secret_key {
            let trimmed = secret.trim().to_string();
            let new_value = if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            };
            if state.baidu_secret_key != new_value {
                baidu_changed = true;
            }
            state.baidu_secret_key = new_value;
        }

        if let Some(api_key) = update.deepseek_api_key {
            let trimmed = api_key.trim().to_string();
            let new_value = if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            };
            if state.deepseek_api_key != new_value {
                deepseek_changed = true;
            }
            state.deepseek_api_key = new_value;
        }

        if baidu_changed {
            clear_verification(&mut state, TranslatorProvider::Baidu);
        }
        if deepseek_changed {
            clear_verification(&mut state, TranslatorProvider::Deepseek);
        }

        if update.ollama_base_url.is_some() || update.ollama_model.is_some() {
            let mut base_guard = self
                .base_ollama
                .write()
                .map_err(|_| anyhow!("failed to acquire ollama base config lock"))?;
            let mut changed = false;
            if let Some(base_url) = update.ollama_base_url {
                let trimmed = base_url.trim().to_string();
                if base_guard.base_url != trimmed {
                    base_guard.base_url = trimmed;
                    changed = true;
                }
            }
            if let Some(model) = update.ollama_model {
                let trimmed = model.trim().to_string();
                if base_guard.model != trimmed {
                    base_guard.model = trimmed;
                    changed = true;
                }
            }
            if changed {
                let snapshot = base_guard.clone();
                drop(base_guard);
                state.ollama_client = build_ollama_client(&self.http_config, &snapshot)?;
                clear_verification(&mut state, TranslatorProvider::Ollama);
                ollama_changed = true;
            } else {
                drop(base_guard);
            }
        }

        state.baidu_client = build_baidu_client(&self.http_config, &state)?;
        state.deepseek_client =
            build_deepseek_client(&self.http_config, &self.base_deepseek, &state)?;
        if state.ollama_client.is_none() {
            let base_guard = self
                .base_ollama
                .read()
                .map_err(|_| anyhow!("failed to read ollama base config"))?;
            state.ollama_client = build_ollama_client(&self.http_config, &base_guard)?;
        }

        if let Some(flag) = update.translate_descriptions {
            state.translate_descriptions = flag;
        }

        if let Some(provider) = update.provider {
            if !provider_available(&state, provider) {
                return Err(anyhow!(
                    "translator provider {:?} unavailable after update",
                    provider
                ));
            }
            state.provider = provider;
        } else if !provider_available(&state, state.provider) {
            if let Some(fallback) = PROVIDER_PRIORITY
                .into_iter()
                .find(|candidate| provider_available(&state, *candidate))
            {
                state.provider = fallback;
            }
        }

        drop(state);
        self.spawn_verification_tasks(baidu_changed, deepseek_changed, ollama_changed);

        Ok(())
    }

    pub async fn translate(
        &self,
        title: &str,
        description: Option<&str>,
    ) -> Result<Option<TranslationResult>> {
        let order = {
            let state = self
                .state
                .read()
                .map_err(|_| anyhow!("translator lock poisoned"))?;
            let mut order = Vec::new();
            if provider_available(&state, state.provider) {
                order.push(state.provider);
            }
            for candidate in PROVIDER_PRIORITY {
                if candidate != state.provider && provider_available(&state, candidate) {
                    order.push(candidate);
                }
            }
            if order.is_empty() {
                return Ok(None);
            }
            order
        };

        let mut last_error: Option<anyhow::Error> = None;

        for provider in order {
            match self.try_provider(provider, title, description).await {
                Ok(result) => return Ok(Some(result)),
                Err(TranslationError::NotConfigured) => continue,
                Err(err @ TranslationError::QuotaExceeded) => {
                    warn!(
                        provider = provider.as_str(),
                        error = %err,
                        "translator quota exceeded, trying fallback"
                    );
                    last_error = Some(err.into_anyhow());
                    continue;
                }
                Err(err) => {
                    warn!(
                        provider = provider.as_str(),
                        error = %err,
                        "translator failed"
                    );
                    last_error = Some(err.into_anyhow());
                    continue;
                }
            }
        }

        if let Some(err) = last_error {
            Err(err)
        } else {
            Ok(None)
        }
    }

    async fn try_provider(
        &self,
        provider: TranslatorProvider,
        title: &str,
        description: Option<&str>,
    ) -> Result<TranslationResult, TranslationError> {
        match provider {
            TranslatorProvider::Baidu => {
                let (client, verified) = {
                    let state = self.state.read().map_err(|_| {
                        TranslationError::Other(anyhow!("translator lock poisoned"))
                    })?;
                    (state.baidu_client.clone(), state.baidu_verified)
                };

                let client = client.ok_or(TranslationError::NotConfigured)?;

                if !verified {
                    return Err(TranslationError::NotConfigured);
                }

                let translated_title = client
                    .translate(title, "auto", "zh")
                    .await
                    .map_err(map_baidu_error)?;
                let translated_description = match description {
                    Some(text) if !text.trim().is_empty() => Some(
                        client
                            .translate(text, "auto", "zh")
                            .await
                            .map_err(map_baidu_error)?,
                    ),
                    _ => None,
                };

                let desc_in_len = description.map(|s| s.len()).unwrap_or(0);
                let desc_out_len = translated_description.as_ref().map(|s| s.len()).unwrap_or(0);
                info!(
                    provider = %TranslatorProvider::Baidu.as_str(),
                    title_len = translated_title.len(),
                    desc_in_len,
                    desc_out_len,
                    "translation success"
                );

                Ok(TranslationResult {
                    title: translated_title,
                    description: translated_description,
                })
            }
            TranslatorProvider::Deepseek => {
                let (client, verified) = {
                    let state = self.state.read().map_err(|_| {
                        TranslationError::Other(anyhow!("translator lock poisoned"))
                    })?;
                    (state.deepseek_client.clone(), state.deepseek_verified)
                };

                let client = client.ok_or(TranslationError::NotConfigured)?;

                if !verified {
                    return Err(TranslationError::NotConfigured);
                }
                client
                    .translate_news(title, description)
                    .await
                    .map(|result| {
                        let desc_in_len = description.map(|s| s.len()).unwrap_or(0);
                        let desc_out_len = result.description.as_ref().map(|s| s.len()).unwrap_or(0);
                        info!(
                            provider = %TranslatorProvider::Deepseek.as_str(),
                            title_len = result.title.len(),
                            desc_in_len,
                            desc_out_len,
                            "translation success"
                        );
                        result
                    })
                    .map_err(TranslationError::Other)
            }
            TranslatorProvider::Ollama => {
                let (client, verified) = {
                    let state = self.state.read().map_err(|_| {
                        TranslationError::Other(anyhow!("translator lock poisoned"))
                    })?;
                    (state.ollama_client.clone(), state.ollama_verified)
                };

                let client = client.ok_or(TranslationError::NotConfigured)?;

                if !verified {
                    return Err(TranslationError::NotConfigured);
                }

                client
                    .translate_news(title, description)
                    .await
                    .map(|result| {
                        let desc_in_len = description.map(|s| s.len()).unwrap_or(0);
                        let desc_out_len = result.description.as_ref().map(|s| s.len()).unwrap_or(0);
                        info!(
                            provider = %TranslatorProvider::Ollama.as_str(),
                            title_len = result.title.len(),
                            desc_in_len,
                            desc_out_len,
                            "translation success"
                        );
                        result
                    })
                    .map_err(TranslationError::Other)
            }
        }
    }
}

fn build_baidu_client(
    http_config: &HttpClientConfig,
    state: &TranslationState,
) -> Result<Option<Arc<BaiduTranslator>>> {
    let app_id = match state.baidu_app_id.as_ref() {
        Some(value) if !value.trim().is_empty() => value.trim(),
        _ => return Ok(None),
    };
    let secret = match state.baidu_secret_key.as_ref() {
        Some(value) if !value.trim().is_empty() => value.trim(),
        _ => return Ok(None),
    };

    Ok(Some(Arc::new(BaiduTranslator::from_credentials(
        app_id,
        secret,
        http_config,
    )?)))
}

fn build_deepseek_client(
    http_config: &HttpClientConfig,
    base_config: &DeepseekBaseConfig,
    state: &TranslationState,
) -> Result<Option<Arc<DeepseekClient>>> {
    let api_key = match state.deepseek_api_key.as_ref() {
        Some(value) if !value.trim().is_empty() => value.trim().to_string(),
        _ => return Ok(None),
    };

    let mut config = crate::config::DeepseekConfig::default();
    config.api_key = Some(api_key);
    config.base_url = base_config.base_url.clone();
    config.model = base_config.model.clone();
    config.timeout_secs = base_config.timeout_secs;

    Ok(Some(Arc::new(DeepseekClient::new(config, http_config)?)))
}

fn build_ollama_client(
    http_config: &HttpClientConfig,
    base_config: &OllamaBaseConfig,
) -> Result<Option<Arc<OllamaClient>>> {
    if base_config.base_url.trim().is_empty() || base_config.model.trim().is_empty() {
        return Ok(None);
    }

    Ok(Some(Arc::new(OllamaClient::new(
        &base_config.base_url,
        &base_config.model,
        base_config.timeout_secs,
        http_config,
    )?)))
}

fn map_baidu_error(err: crate::util::baidu::BaiduError) -> TranslationError {
    match err {
        crate::util::baidu::BaiduError::QuotaExceeded => TranslationError::QuotaExceeded,
        crate::util::baidu::BaiduError::Api { code, message } => {
            TranslationError::Api { code, message }
        }
        crate::util::baidu::BaiduError::Other(inner) => TranslationError::Other(inner),
    }
}

fn mask_secret(value: &str) -> String {
    if value.is_empty() {
        return "".to_string();
    }
    if value.len() <= 4 {
        return "••••".to_string();
    }
    let visible = &value[value.len() - 4..];
    format!("••••{}", visible)
}
