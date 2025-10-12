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
}

impl Default for ArticleListQuery {
    fn default() -> Self {
        Self {
            from: None,
            to: None,
            page: 1,
            page_size: 20,
        }
    }
}
