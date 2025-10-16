use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use reqwest::{header, Client};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::config::HttpClientConfig;

use super::deepseek::{
    build_translation_input, parse_translation, TranslationResult, TRANSLATION_PROMPT,
};

pub struct OllamaClient {
    http: Client,
    base_url: String,
    model: String,
}

impl OllamaClient {
    pub fn new(
        base_url: &str,
        model: &str,
        timeout_secs: u64,
        http_config: &HttpClientConfig,
    ) -> Result<Self> {
        let timeout = Duration::from_secs(timeout_secs.max(1));
        let mut builder = http_config
            .apply(Client::builder())
            .context("failed to apply proxy settings for ollama client")?;
        if let Ok(parsed) = Url::parse(base_url) {
            let disable_proxy = parsed
                .host()
                .map(|host| match host {
                    url::Host::Domain(domain) => domain.eq_ignore_ascii_case("localhost"),
                    url::Host::Ipv4(addr) => addr.is_loopback(),
                    url::Host::Ipv6(addr) => addr.is_loopback(),
                })
                .unwrap_or(false);
            if disable_proxy {
                builder = builder.no_proxy();
            }
        }
        let http = builder
            .timeout(timeout)
            .build()
            .context("failed to build ollama http client")?;

        Ok(Self {
            http,
            base_url: base_url.trim_end_matches('/').to_string(),
            model: model.to_string(),
        })
    }

    pub async fn translate_news(
        &self,
        title: &str,
        description: Option<&str>,
    ) -> Result<TranslationResult> {
        if self.base_url.is_empty() {
            return Err(anyhow!("ollama base url not configured"));
        }

        let url = format!("{}/api/chat", self.base_url);
        let payload = ChatRequest {
            model: self.model.clone(),
            messages: vec![
                ChatMessage {
                    role: "system",
                    content: TRANSLATION_PROMPT.to_string(),
                },
                ChatMessage {
                    role: "user",
                    content: build_translation_input(title, description),
                },
            ],
            stream: false,
        };

        let response = self
            .http
            .post(&url)
            .header(header::CONTENT_TYPE, "application/json")
            .json(&payload)
            .send()
            .await
            .context("ollama translation request failed")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow!(
                "ollama translation returned non-success status {}: {}",
                status,
                body
            ));
        }

        let text = response
            .text()
            .await
            .context("failed to read ollama translation response")?;

        let content = extract_content(&text).unwrap_or_else(|| text.clone());

        parse_translation(&content)
            .context("failed to parse ollama translation payload: ensure模型提示输出 JSON")
    }
}

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    stream: bool,
}

#[derive(Serialize)]
struct ChatMessage {
    role: &'static str,
    content: String,
}

#[derive(Deserialize)]
struct ChatResponse {
    message: Option<ChatResponseMessage>,
    messages: Option<Vec<ChatResponseMessage>>,
    response: Option<String>,
}

#[derive(Deserialize)]
struct ChatResponseMessage {
    #[serde(default)]
    content: String,
}

fn extract_content(raw: &str) -> Option<String> {
    if let Ok(parsed) = serde_json::from_str::<ChatResponse>(raw) {
        if let Some(message) = parsed.message {
            if !message.content.trim().is_empty() {
                return Some(message.content);
            }
        }
        if let Some(messages) = parsed.messages {
            let combined = messages
                .iter()
                .map(|msg| msg.content.trim())
                .filter(|content| !content.is_empty())
                .collect::<Vec<_>>()
                .join("\n");
            if !combined.trim().is_empty() {
                return Some(combined);
            }
        }
        if let Some(response) = parsed.response {
            if !response.trim().is_empty() {
                return Some(response);
            }
        }
    }
    None
}
