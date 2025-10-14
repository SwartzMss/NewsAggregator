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

    let url = url.trim().to_string();
    if url.is_empty() {
        return Err(AppError::BadRequest("url is required".into()));
    }

    let source_domain_input = source_domain.trim();
    let (source_domain, derived_source_domain) = if source_domain_input.is_empty() {
        let inferred = crate::util::url_norm::infer_source_domain(&url)
            .ok_or_else(|| AppError::BadRequest("无法从 URL 推断来源域名".into()))?;
        (inferred, true)
    } else {
        (source_domain_input.to_ascii_lowercase(), false)
    };

    if source_domain.is_empty() {
        return Err(AppError::BadRequest("source_domain is required".into()));
    }

    let record = repo::feeds::FeedUpsertRecord {
        url: url.clone(),
        title,
        site_url,
        source_domain: source_domain.clone(),
        enabled,
        fetch_interval_seconds,
    };

    let row = repo::feeds::upsert_feed(pool, record).await?;

    tracing::info!(
        feed_id = row.id,
        url = row.url,
        enabled = row.enabled,
        source_domain = %source_domain,
        derived_source_domain,
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
    let mut lock_conn = pool.acquire().await?;
    repo::feeds::acquire_processing_lock(&mut lock_conn, id).await?;

    let result: AppResult<()> = async {
        let mut tx = pool.begin().await?;

        let disabled = repo::feeds::disable_feed(&mut tx, id).await?;
        if disabled == 0 {
            tx.rollback().await?;
            return Err(AppError::BadRequest(format!("feed {id} not found")));
        }

        repo::article_sources::delete_by_feed(&mut tx, id).await?;
        repo::articles::delete_by_feed(&mut tx, id).await?;
        repo::feeds::delete_feed(&mut tx, id).await?;

        tx.commit().await?;
        Ok(())
    }
    .await;

    let release_result = repo::feeds::release_processing_lock(&mut lock_conn, id).await;
    drop(lock_conn);

    match (result, release_result) {
        (Ok(()), Ok(())) => {
            tracing::info!(feed_id = id, "feed and associated content deleted");
            Ok(())
        }
        (Err(err), Ok(())) => Err(err),
        (Ok(()), Err(release_err)) => Err(AppError::from(release_err)),
        (Err(err), Err(release_err)) => {
            tracing::error!(
                error = ?release_err,
                feed_id = id,
                "failed to release feed lock after error"
            );
            Err(err)
        }
    }
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
        .map(|text| text.content.clone())
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
