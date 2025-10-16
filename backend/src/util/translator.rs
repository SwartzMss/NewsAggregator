use std::sync::{Arc, RwLock};

use anyhow::{anyhow, Result};
use tokio::runtime::Handle;
use tracing::{debug, warn};

use crate::config::{AiConfig, HttpClientConfig, TranslatorConfig};

use super::{
    baidu::BaiduTranslator,
    deepseek::{DeepseekClient, TranslationResult},
};

const VERIFICATION_SAMPLE_TEXT: &str = "NewsAggregator ping";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TranslatorProvider {
    Deepseek,
    Baidu,
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
    }
}

fn provider_available(state: &TranslationState, provider: TranslatorProvider) -> bool {
    match provider {
        TranslatorProvider::Baidu => state.baidu_client.is_some() && state.baidu_verified,
        TranslatorProvider::Deepseek => state.deepseek_client.is_some() && state.deepseek_verified,
    }
}

fn ensure_provider_consistency(state: &mut TranslationState) {
    if provider_available(state, state.provider) {
        return;
    }

    if let Some(fallback) = [TranslatorProvider::Deepseek, TranslatorProvider::Baidu]
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
) {
    if !verify_baidu && !verify_deepseek {
        return;
    }

    let (baidu_client, deepseek_client) = {
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

        (baidu, deepseek)
    };

    if verify_baidu {
        if let Some(client) = baidu_client {
            debug!("verifying baidu translator credentials");
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
                }
                Err(err) => {
                    guard.baidu_verified = false;
                    guard.baidu_error = Some(truncate_error(err));
                    warn!(
                        error = guard.baidu_error.as_deref().unwrap_or_default(),
                        "baidu translator credential verification failed"
                    );
                }
            }
        } else if let Ok(mut guard) = state.write() {
            clear_verification(&mut guard, TranslatorProvider::Baidu);
        }
    }

    if verify_deepseek {
        if let Some(client) = deepseek_client {
            debug!("verifying deepseek translator credentials");
            let result = client.translate_news(VERIFICATION_SAMPLE_TEXT, None).await;

            let mut guard = state
                .write()
                .expect("translator state poisoned while updating deepseek verification");
            match result {
                Ok(_) => {
                    guard.deepseek_verified = true;
                    guard.deepseek_error = None;
                }
                Err(err) => {
                    guard.deepseek_verified = false;
                    guard.deepseek_error = Some(truncate_error(err));
                    warn!(
                        error = guard.deepseek_error.as_deref().unwrap_or_default(),
                        "deepseek translator credential verification failed"
                    );
                }
            }
        } else if let Ok(mut guard) = state.write() {
            clear_verification(&mut guard, TranslatorProvider::Deepseek);
        }
    }

    if verify_baidu || verify_deepseek {
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
        }
    }
}

impl std::str::FromStr for TranslatorProvider {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "deepseek" => Ok(TranslatorProvider::Deepseek),
            "baidu" => Ok(TranslatorProvider::Baidu),
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
}

#[derive(Debug, Clone)]
struct DeepseekBaseConfig {
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
}

#[derive(Debug, Clone)]
pub struct TranslatorSnapshot {
    pub provider: TranslatorProvider,
    pub baidu_configured: bool,
    pub deepseek_configured: bool,
    pub baidu_app_id_masked: Option<String>,
    pub baidu_secret_key_masked: Option<String>,
    pub deepseek_api_key_masked: Option<String>,
    pub baidu_error: Option<String>,
    pub deepseek_error: Option<String>,
}

impl TranslationEngine {
    pub fn new(
        http_client: &HttpClientConfig,
        translator_config: &TranslatorConfig,
        ai_config: &AiConfig,
    ) -> Result<Self> {
        let mut state = TranslationState {
            provider: translator_config
                .provider
                .parse::<TranslatorProvider>()
                .unwrap_or(TranslatorProvider::Deepseek),
            baidu_app_id: translator_config
                .baidu
                .app_id
                .as_ref()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty()),
            baidu_secret_key: translator_config
                .baidu
                .secret_key
                .as_ref()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty()),
            baidu_client: None,
            baidu_verified: false,
            baidu_error: None,
            deepseek_api_key: ai_config
                .deepseek
                .api_key
                .as_ref()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty()),
            deepseek_client: None,
            deepseek_verified: false,
            deepseek_error: None,
        };

        let base_deepseek = DeepseekBaseConfig {
            base_url: ai_config.deepseek.base_url.clone(),
            model: ai_config.deepseek.model.clone(),
            timeout_secs: ai_config.deepseek.timeout_secs,
        };

        // attempt to build clients
        state.baidu_client = build_baidu_client(http_client, &state)?;
        state.deepseek_client = build_deepseek_client(http_client, &base_deepseek, &state)?;
        clear_verification(&mut state, TranslatorProvider::Baidu);
        clear_verification(&mut state, TranslatorProvider::Deepseek);

        if state.provider == TranslatorProvider::Baidu && state.baidu_client.is_none() {
            if state.deepseek_client.is_some() {
                state.provider = TranslatorProvider::Deepseek;
            }
        } else if state.provider == TranslatorProvider::Deepseek && state.deepseek_client.is_none()
        {
            if state.baidu_client.is_some() {
                state.provider = TranslatorProvider::Baidu;
            }
        }

        let engine = Self {
            state: Arc::new(RwLock::new(state)),
            http_config: http_client.clone(),
            base_deepseek,
        };

        engine.spawn_verification_tasks(true, true);

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
        };

        if !has_client {
            return Err(anyhow!("translator provider {:?} unavailable", provider));
        }

        guard.provider = provider;
        drop(guard);

        if !available {
            match provider {
                TranslatorProvider::Baidu => self.spawn_verification_tasks(true, false),
                TranslatorProvider::Deepseek => self.spawn_verification_tasks(false, true),
            }
        }

        Ok(())
    }

    pub fn available_providers(&self) -> Vec<TranslatorProvider> {
        self.state
            .read()
            .map(|state| {
                [TranslatorProvider::Deepseek, TranslatorProvider::Baidu]
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

    fn spawn_verification_tasks(&self, verify_baidu: bool, verify_deepseek: bool) {
        if !verify_baidu && !verify_deepseek {
            return;
        }

        let state = Arc::clone(&self.state);
        match Handle::try_current() {
            Ok(handle) => {
                handle.spawn(async move {
                    verify_provider_credentials(state, verify_baidu, verify_deepseek).await;
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
                }
            }
        }
    }

    pub fn snapshot(&self) -> TranslatorSnapshot {
        let state = self.state.read().expect("translator state poisoned");
        TranslatorSnapshot {
            provider: state.provider,
            baidu_configured: state.baidu_client.is_some() && state.baidu_verified,
            deepseek_configured: state.deepseek_client.is_some() && state.deepseek_verified,
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
        }
    }

    pub fn update_credentials(&self, update: TranslatorCredentialsUpdate) -> Result<()> {
        let mut state = self
            .state
            .write()
            .map_err(|_| anyhow!("failed to acquire translator state lock"))?;

        let mut baidu_changed = false;
        let mut deepseek_changed = false;

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

        state.baidu_client = build_baidu_client(&self.http_config, &state)?;
        state.deepseek_client =
            build_deepseek_client(&self.http_config, &self.base_deepseek, &state)?;

        if let Some(provider) = update.provider {
            if !provider_available(&state, provider) {
                return Err(anyhow!(
                    "translator provider {:?} unavailable after update",
                    provider
                ));
            }
            state.provider = provider;
        } else if !provider_available(&state, state.provider) {
            if let Some(fallback) = [TranslatorProvider::Deepseek, TranslatorProvider::Baidu]
                .into_iter()
                .find(|candidate| provider_available(&state, *candidate))
            {
                state.provider = fallback;
            }
        }

        drop(state);
        self.spawn_verification_tasks(baidu_changed, deepseek_changed);

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
            let fallback = match state.provider {
                TranslatorProvider::Baidu => TranslatorProvider::Deepseek,
                TranslatorProvider::Deepseek => TranslatorProvider::Baidu,
            };
            if provider_available(&state, fallback) && !order.contains(&fallback) {
                order.push(fallback);
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
