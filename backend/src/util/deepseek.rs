use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use reqwest::{header, Client};
use serde::{Deserialize, Serialize};

use crate::config::DeepseekConfig;

/// Summary of a candidate article used for de-duplication prompts.
#[derive(Debug, Clone)]
pub struct ArticleSnippet<'a> {
    pub title: &'a str,
    pub source: Option<&'a str>,
    pub url: Option<&'a str>,
    pub published_at: Option<&'a str>,
    pub summary: Option<&'a str>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DeepseekDecision {
    pub is_duplicate: bool,
    pub reason: Option<String>,
    pub confidence: Option<f32>,
    pub _raw: String,
}

pub struct DeepseekClient {
    http: Client,
    config: DeepseekConfig,
}

impl DeepseekClient {
    pub fn new(config: DeepseekConfig) -> Result<Self> {
        let timeout = Duration::from_secs(config.timeout_secs.max(1));
        let http = Client::builder()
            .timeout(timeout)
            .build()
            .context("failed to build deepseek http client")?;

        Ok(Self { http, config })
    }

    pub async fn judge_similarity(
        &self,
        a: &ArticleSnippet<'_>,
        b: &ArticleSnippet<'_>,
    ) -> Result<DeepseekDecision> {
        let api_key = self
            .config
            .api_key
            .as_deref()
            .ok_or_else(|| anyhow!("deepseek api key missing"))?;

        let base = self.config.base_url.trim_end_matches('/');
        let url = format!("{base}/v1/chat/completions");

        let prompt = build_prompt(a, b);

        let body = ChatCompletionRequest {
            model: &self.config.model,
            messages: vec![
                ChatMessage {
                    role: "system",
                    content: SYSTEM_PROMPT.to_string(),
                },
                ChatMessage {
                    role: "user",
                    content: prompt,
                },
            ],
            temperature: 0.1,
        };

        let response = self
            .http
            .post(&url)
            .header(header::AUTHORIZATION, format!("Bearer {api_key}"))
            .header(header::CONTENT_TYPE, "application/json")
            .json(&body)
            .send()
            .await
            .context("deepseek request failed")?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(anyhow!(
                "deepseek returned non-success status {}: {}",
                status,
                text
            ));
        }

        let payload: ChatCompletionResponse = response
            .json()
            .await
            .context("failed to parse deepseek response")?;

        let content = payload
            .choices
            .into_iter()
            .next()
            .and_then(|choice| choice.message.content)
            .ok_or_else(|| anyhow!("deepseek response missing message content"))?;

        let mut decision = parse_decision(&content).with_context(|| {
            format!("failed to parse deepseek decision from content: {content}")
        })?;

        decision._raw = content;
        Ok(decision)
    }
}

fn build_prompt(a: &ArticleSnippet<'_>, b: &ArticleSnippet<'_>) -> String {
    fn lines(snippet: &ArticleSnippet<'_>, label: &str) -> String {
        let mut parts = vec![format!("标题: {}", snippet.title)];
        if let Some(source) = snippet.source {
            parts.push(format!("来源: {source}"));
        }
        if let Some(url) = snippet.url {
            parts.push(format!("链接: {url}"));
        }
        if let Some(published_at) = snippet.published_at {
            parts.push(format!("发布时间: {published_at}"));
        }
        if let Some(summary) = snippet.summary {
            parts.push(format!("摘要: {summary}"));
        }
        format!("{label}\n{}\n", parts.join("\n"))
    }

    format!(
        "请比较以下两条新闻是否描述同一事件。若认为是同一新闻，请输出 JSON {{\"is_duplicate\": true, \"reason\": \"简要原因\", \"confidence\": 0-1之间的小数}}；如果不是，请输出对应的 false。除该 JSON 外不要包含额外文字。\n\n{}\n{}\n",
        lines(a, "新闻A"),
        lines(b, "新闻B")
    )
}

fn parse_decision(content: &str) -> Result<DeepseekDecision> {
    let cleaned = content.trim();
    let json_str = cleaned
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();

    #[derive(Deserialize)]
    struct DecisionPayload {
        is_duplicate: bool,
        #[serde(default)]
        reason: Option<String>,
        #[serde(default)]
        confidence: Option<f32>,
    }

    let payload: DecisionPayload =
        serde_json::from_str(json_str).or_else(|_| serde_json::from_str(cleaned))?;

    Ok(DeepseekDecision {
        is_duplicate: payload.is_duplicate,
        reason: payload.reason,
        confidence: payload.confidence,
        _raw: String::new(),
    })
}

#[derive(Serialize)]
struct ChatCompletionRequest<'a> {
    model: &'a str,
    messages: Vec<ChatMessage>,
    temperature: f32,
}

#[derive(Serialize)]
struct ChatMessage {
    role: &'static str,
    content: String,
}

#[derive(Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Deserialize)]
struct ChatChoice {
    message: ChatCompletionMessage,
}

#[derive(Deserialize)]
struct ChatCompletionMessage {
    content: Option<String>,
}

const SYSTEM_PROMPT: &str = "你是一名资深的新闻比对助手，需要判断两条新闻是否描述同一事件。输出必须是 JSON，字段 is_duplicate、reason、confidence。";
