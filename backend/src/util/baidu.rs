use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, Context, Result};
use reqwest::Client;
use serde::Deserialize;

use crate::config::{BaiduTranslatorConfig, HttpClientConfig};

const BAIDU_API_URL: &str = "https://fanyi-api.baidu.com/api/trans/vip/translate";

#[derive(Debug)]
pub struct BaiduTranslator {
    client: Client,
    app_id: String,
    secret_key: String,
}

#[derive(Debug, thiserror::Error)]
pub enum BaiduError {
    #[error("baidu translator quota exceeded")]
    QuotaExceeded,
    #[error("baidu translator api error {code}: {message}")]
    Api { code: String, message: String },
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl BaiduTranslator {
    #[allow(dead_code)]
    pub fn new(
        config: &BaiduTranslatorConfig,
        http_config: &HttpClientConfig,
    ) -> Result<Option<Self>> {
        let app_id = config
            .app_id
            .as_ref()
            .map(|v| v.trim())
            .filter(|v| !v.is_empty())
            .map(|v| v.to_string());
        let secret_key = config
            .secret_key
            .as_ref()
            .map(|v| v.trim())
            .filter(|v| !v.is_empty())
            .map(|v| v.to_string());

        let (app_id, secret_key) = match (app_id, secret_key) {
            (Some(app_id), Some(secret_key)) => (app_id, secret_key),
            _ => return Ok(None),
        };

        Ok(Some(Self::from_credentials(
            &app_id,
            &secret_key,
            http_config,
        )?))
    }

    pub fn from_credentials(
        app_id: &str,
        secret_key: &str,
        http_config: &HttpClientConfig,
    ) -> Result<Self> {
        let client = http_config
            .apply(Client::builder().user_agent("NewsAggregatorTranslator/0.1"))
            .context("failed to apply proxy settings for baidu translator")?
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .context("failed to build baidu translator client")?;

        Ok(Self {
            client,
            app_id: app_id.to_string(),
            secret_key: secret_key.to_string(),
        })
    }

    pub async fn translate(
        &self,
        text: &str,
        from_lang: &str,
        to_lang: &str,
    ) -> Result<String, BaiduError> {
        if text.trim().is_empty() {
            return Ok(String::new());
        }

        let salt = Self::generate_salt();
        let sign = Self::generate_signature(&self.app_id, text, &salt, &self.secret_key);

        let response = self
            .client
            .get(BAIDU_API_URL)
            .query(&[
                ("q", text),
                ("from", from_lang),
                ("to", to_lang),
                ("appid", self.app_id.as_str()),
                ("salt", salt.as_str()),
                ("sign", sign.as_str()),
            ])
            .send()
            .await
            .context("baidu translation request failed")
            .map_err(BaiduError::Other)?;

        let status = response.status();
        let body = response
            .text()
            .await
            .context("failed to read baidu translation response")
            .map_err(BaiduError::Other)?;

        if !status.is_success() {
            return Err(BaiduError::Other(anyhow!(
                "baidu translation http status {}: {}",
                status,
                body
            )));
        }

        let payload: BaiduResponse = serde_json::from_str(&body)
            .context("failed to parse baidu translation response")
            .map_err(BaiduError::Other)?;

        if let Some(code) = payload.error_code {
            let message = payload
                .error_msg
                .unwrap_or_else(|| "unknown error".to_string());
            return Err(match code.as_str() {
                "54004" => BaiduError::QuotaExceeded,
                _ => BaiduError::Api { code, message },
            });
        }

        let mut combined = String::new();
        if let Some(results) = payload.trans_result {
            for item in results {
                if !combined.is_empty() {
                    combined.push('\n');
                }
                combined.push_str(&item.dst);
            }
        }

        if combined.is_empty() {
            return Err(BaiduError::Other(anyhow!("empty translation result")));
        }

        Ok(combined)
    }

    fn generate_salt() -> String {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis().to_string())
            .unwrap_or_else(|_| "0".to_string())
    }

    fn generate_signature(app_id: &str, query: &str, salt: &str, secret_key: &str) -> String {
        let raw = format!("{}{}{}{}", app_id, query, salt, secret_key);
        let digest = md5::compute(raw.as_bytes());
        format!("{:x}", digest)
    }
}

#[derive(Debug, Deserialize)]
struct BaiduResponse {
    error_code: Option<String>,
    error_msg: Option<String>,
    trans_result: Option<Vec<BaiduTransResult>>,
}

#[derive(Debug, Deserialize)]
struct BaiduTransResult {
    dst: String,
}
