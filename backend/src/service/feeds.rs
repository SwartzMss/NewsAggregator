use std::time::Duration;

use feed_rs::parser;
use reqwest::Client;

use crate::{
    error::{AppError, AppResult},
    model::{FeedOut, FeedTestPayload, FeedTestResult, FeedUpsertPayload},
    repo,
};

pub async fn list(pool: &sqlx::PgPool) -> AppResult<Vec<FeedOut>> {
    let rows = repo::feeds::list_feeds(pool).await?;
    Ok(rows.into_iter().map(feed_row_to_out).collect())
}

pub async fn upsert(pool: &sqlx::PgPool, payload: FeedUpsertPayload) -> AppResult<FeedOut> {
    let FeedUpsertPayload {
        id,
        url,
        source_domain,
        enabled,
        fetch_interval_seconds,
        title,
        site_url,
    } = payload;

    if url.is_empty() {
        return Err(AppError::BadRequest("url is required".into()));
    }

    if source_domain.is_empty() {
        return Err(AppError::BadRequest("source_domain is required".into()));
    }

    let record = repo::feeds::FeedUpsertRecord {
        url,
        title,
        site_url,
        source_domain,
        enabled,
        fetch_interval_seconds,
    };

    let row = repo::feeds::upsert_feed(pool, record).await?;

    tracing::info!(
        feed_id = row.id,
        url = row.url,
        enabled = row.enabled,
        "feed saved"
    );

    if let Some(expected) = id {
        if row.id != expected {
            return Err(AppError::BadRequest(format!(
                "payload id {expected} does not match stored feed"
            )));
        }
    }

    Ok(feed_row_to_out(row))
}

pub async fn delete(pool: &sqlx::PgPool, id: i64) -> AppResult<()> {
    let affected = repo::feeds::delete_feed(pool, id).await?;
    if affected == 0 {
        return Err(AppError::BadRequest(format!("feed {id} not found")));
    }
    tracing::info!(feed_id = id, "feed deleted");
    Ok(())
}

pub async fn test(payload: FeedTestPayload) -> AppResult<FeedTestResult> {
    let url = payload.url.trim();
    if url.is_empty() {
        return Err(AppError::BadRequest("url is required".into()));
    }

    let client = Client::builder()
        .user_agent("NewsAggregatorTester/0.1")
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|err| AppError::Internal(err.into()))?;

    let response = client
        .get(url)
        .send()
        .await
        .map_err(|err| AppError::BadRequest(format!("请求订阅源失败: {err}")))?;

    let status = response.status();
    if !status.is_success() {
        return Err(AppError::BadRequest(format!(
            "订阅源返回状态码 {}",
            status.as_u16()
        )));
    }

    let bytes = response
        .bytes()
        .await
        .map_err(|err| AppError::BadRequest(format!("读取订阅源失败: {err}")))?;

    let parsed = parser::parse(&bytes[..])
        .map_err(|err| AppError::BadRequest(format!("解析订阅源失败: {err}")))?;

    let title = parsed
        .title
        .as_ref()
        .and_then(|text| text.content.clone())
        .filter(|s| !s.trim().is_empty());

    let site_url = parsed.links.first().map(|link| link.href.to_string());

    Ok(FeedTestResult {
        status: status.as_u16(),
        title,
        site_url,
        entry_count: parsed.entries.len(),
    })
}

fn feed_row_to_out(row: repo::feeds::FeedRow) -> FeedOut {
    FeedOut {
        id: row.id,
        url: row.url,
        title: row.title,
        site_url: row.site_url,
        source_domain: row.source_domain,
        enabled: row.enabled,
        fetch_interval_seconds: row.fetch_interval_seconds,
        last_fetch_at: row.last_fetch_at.map(|dt| dt.to_rfc3339()),
        last_fetch_status: row.last_fetch_status.map(|s| s as i32),
        fail_count: row.fail_count,
    }
}
