use std::sync::{Arc, RwLock};

use anyhow::{anyhow, Result};
use tracing::warn;

use crate::config::{AiConfig, HttpClientConfig, TranslatorConfig};

use super::{
    baidu::BaiduTranslator,
    deepseek::{DeepseekClient, TranslationResult},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TranslatorProvider {
    Deepseek,
    Baidu,
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
    deepseek_api_key: Option<String>,
    deepseek_client: Option<Arc<DeepseekClient>>,
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
            deepseek_api_key: ai_config
                .deepseek
                .api_key
                .as_ref()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty()),
            deepseek_client: None,
        };

        let base_deepseek = DeepseekBaseConfig {
            base_url: ai_config.deepseek.base_url.clone(),
            model: ai_config.deepseek.model.clone(),
            timeout_secs: ai_config.deepseek.timeout_secs,
        };

        // attempt to build clients
        state.baidu_client = build_baidu_client(http_client, &state)?;
        state.deepseek_client = build_deepseek_client(http_client, &base_deepseek, &state)?;

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

        Ok(Self {
            state: Arc::new(RwLock::new(state)),
            http_config: http_client.clone(),
            base_deepseek,
        })
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

        let available = match provider {
            TranslatorProvider::Baidu => guard.baidu_client.is_some(),
            TranslatorProvider::Deepseek => guard.deepseek_client.is_some(),
        };

        if !available {
            return Err(anyhow!("translator provider {:?} unavailable", provider));
        }

        guard.provider = provider;
        Ok(())
    }

    pub fn available_providers(&self) -> Vec<TranslatorProvider> {
        vec![TranslatorProvider::Deepseek, TranslatorProvider::Baidu]
    }

    #[allow(dead_code)]
    pub fn is_baidu_available(&self) -> bool {
        self.state
            .read()
            .map(|state| state.baidu_client.is_some())
            .unwrap_or(false)
    }

    #[allow(dead_code)]
    pub fn is_deepseek_available(&self) -> bool {
        self.state
            .read()
            .map(|state| state.deepseek_client.is_some())
            .unwrap_or(false)
    }

    pub fn deepseek_client(&self) -> Option<Arc<DeepseekClient>> {
        self.state
            .read()
            .ok()
            .and_then(|state| state.deepseek_client.as_ref().map(Arc::clone))
    }

    pub fn snapshot(&self) -> TranslatorSnapshot {
        let state = self.state.read().expect("translator state poisoned");
        TranslatorSnapshot {
            provider: state.provider,
            baidu_configured: state.baidu_client.is_some(),
            deepseek_configured: state.deepseek_client.is_some(),
            baidu_app_id_masked: state.baidu_app_id.as_ref().map(|value| mask_secret(value)),
            baidu_secret_key_masked: state
                .baidu_secret_key
                .as_ref()
                .map(|value| mask_secret(value)),
            deepseek_api_key_masked: state
                .deepseek_api_key
                .as_ref()
                .map(|value| mask_secret(value)),
        }
    }

    pub fn update_credentials(&self, update: TranslatorCredentialsUpdate) -> Result<()> {
        let mut state = self
            .state
            .write()
            .map_err(|_| anyhow!("failed to acquire translator state lock"))?;

        if let Some(app_id) = update.baidu_app_id {
            let trimmed = app_id.trim().to_string();
            state.baidu_app_id = if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            };
        }

        if let Some(secret) = update.baidu_secret_key {
            let trimmed = secret.trim().to_string();
            state.baidu_secret_key = if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            };
        }

        if let Some(api_key) = update.deepseek_api_key {
            let trimmed = api_key.trim().to_string();
            state.deepseek_api_key = if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            };
        }

        state.baidu_client = build_baidu_client(&self.http_config, &state)?;
        state.deepseek_client =
            build_deepseek_client(&self.http_config, &self.base_deepseek, &state)?;

        if let Some(provider) = update.provider {
            let available = match provider {
                TranslatorProvider::Baidu => state.baidu_client.is_some(),
                TranslatorProvider::Deepseek => state.deepseek_client.is_some(),
            };
            if !available {
                return Err(anyhow!(
                    "translator provider {:?} unavailable after update",
                    provider
                ));
            }
            state.provider = provider;
        } else if state.provider == TranslatorProvider::Baidu && state.baidu_client.is_none() {
            if state.deepseek_client.is_some() {
                state.provider = TranslatorProvider::Deepseek;
            }
        } else if state.provider == TranslatorProvider::Deepseek && state.deepseek_client.is_none()
        {
            if state.baidu_client.is_some() {
                state.provider = TranslatorProvider::Baidu;
            }
        }

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
            if state.baidu_client.is_none() && state.deepseek_client.is_none() {
                return Ok(None);
            }
            match state.provider {
                TranslatorProvider::Baidu => {
                    vec![TranslatorProvider::Baidu, TranslatorProvider::Deepseek]
                }
                TranslatorProvider::Deepseek => {
                    vec![TranslatorProvider::Deepseek, TranslatorProvider::Baidu]
                }
            }
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
                let client = self
                    .state
                    .read()
                    .map_err(|_| TranslationError::Other(anyhow!("translator lock poisoned")))?
                    .baidu_client
                    .clone()
                    .ok_or(TranslationError::NotConfigured)?;

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
                let client = self
                    .state
                    .read()
                    .map_err(|_| TranslationError::Other(anyhow!("translator lock poisoned")))?
                    .deepseek_client
                    .clone()
                    .ok_or(TranslationError::NotConfigured)?;
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
