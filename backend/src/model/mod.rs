use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
pub struct ArticleOut {
    pub id: i64,
    pub title: String,
    pub url: String,
    pub description: Option<String>,
    pub language: Option<String>,
    pub source_domain: String,
    pub published_at: String,
    pub click_count: i64,
}

#[derive(Debug, Serialize)]
pub struct FeedOut {
    pub id: i64,
    pub url: String,
    pub title: Option<String>,
    pub site_url: Option<String>,
    pub source_domain: String,
    pub enabled: bool,
    pub fetch_interval_seconds: i32,
    pub filter_condition: Option<String>,
    pub last_fetch_at: Option<String>,
    pub last_fetch_status: Option<i32>,
    pub fail_count: i32,
}

#[derive(Debug, Serialize)]
pub struct PageResp<T> {
    pub page: u32,
    pub page_size: u32,
    pub total_hint: u64,
    pub items: Vec<T>,
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct ArticleListQuery {
    pub from: Option<String>,
    pub to: Option<String>,
    pub page: u32,
    pub page_size: u32,
    pub keyword: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AdminLoginPayload {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct AdminLogoutPayload {
    pub token: String,
}

#[derive(Debug, Serialize)]
pub struct AdminLoginResponse {
    pub token: String,
    pub expires_in: u64,
}

#[derive(Debug, Deserialize)]
pub struct FeedUpsertPayload {
    pub id: Option<i64>,
    pub url: String,
    pub source_domain: String,
    pub enabled: Option<bool>,
    pub fetch_interval_seconds: Option<i32>,
    pub title: Option<String>,
    pub site_url: Option<String>,
    pub filter_condition: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct FeedTestPayload {
    pub url: String,
}

#[derive(Debug, Serialize)]
pub struct FeedTestResult {
    pub status: u16,
    pub title: Option<String>,
    pub site_url: Option<String>,
    pub entry_count: usize,
}

#[derive(Debug, Serialize)]
pub struct TranslationSettingsOut {
    pub provider: String,
    pub translation_enabled: bool,
    pub deepseek_configured: bool,
    pub ollama_configured: bool,
    pub deepseek_api_key_masked: Option<String>,
    pub deepseek_error: Option<String>,
    pub ollama_error: Option<String>,
    pub ollama_base_url: Option<String>,
    pub ollama_model: Option<String>,
    pub translate_descriptions: bool,
}

#[derive(Debug, Deserialize)]
pub struct TranslationSettingsUpdate {
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub translation_enabled: Option<bool>,
    #[serde(default)]
    pub deepseek_api_key: Option<String>,
    #[serde(default)]
    pub ollama_base_url: Option<String>,
    #[serde(default)]
    pub ollama_model: Option<String>,
    #[serde(default)]
    pub translate_descriptions: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct AiDedupSettingsOut {
    pub enabled: bool,
    pub provider: Option<String>, // deepseek | ollama when enabled
    pub deepseek_configured: bool,
    pub ollama_configured: bool,
    pub threshold: f32,
    pub max_checks: usize,
}

#[derive(Debug, Deserialize)]
pub struct AiDedupSettingsUpdate {
    pub enabled: Option<bool>,
    pub provider: Option<String>,
}

impl Default for ArticleListQuery {
    fn default() -> Self {
        Self {
            from: None,
            to: None,
            page: 1,
            page_size: 20,
            keyword: None,
        }
    }
}
