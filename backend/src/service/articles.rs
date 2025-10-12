use chrono::{DateTime, Utc};
use sqlx::PgPool;

use crate::{
    error::{AppError, AppResult},
    model::{ArticleListQuery, ArticleOut, PageResp},
    repo,
};

pub async fn list(pool: &PgPool, query: ArticleListQuery) -> AppResult<PageResp<ArticleOut>> {
    let ArticleListQuery {
        from,
        to,
        page,
        page_size,
    } = query;

    let page = if page == 0 { 1 } else { page };
    let page_size = page_size.clamp(1, 50);
    let offset = ((page - 1) * page_size) as i64;
    let limit = page_size as i64;

    let from = parse_optional_datetime(from.as_deref(), "from")?;
    let to = parse_optional_datetime(to.as_deref(), "to")?;

    let (rows, total) = repo::articles::list_articles(
        pool,
        repo::articles::ArticleListArgs {
            from,
            to,
            limit,
            offset,
        },
    )
    .await?;

    tracing::debug!(page, page_size, total, "articles list queried");

    let items = rows
        .into_iter()
        .map(|row| ArticleOut {
            id: row.id,
            title: row.title,
            url: row.url,
            description: row.description,
            language: row.language,
            source_domain: row.source_domain,
            published_at: row.published_at.to_rfc3339(),
            click_count: row.click_count,
        })
        .collect();

    Ok(PageResp {
        page,
        page_size,
        total_hint: total.max(0) as u64,
        items,
    })
}

fn parse_optional_datetime(value: Option<&str>, field: &str) -> AppResult<Option<DateTime<Utc>>> {
    match value {
        Some(raw) => {
            let parsed = DateTime::parse_from_rfc3339(raw)
                .map_err(|_| AppError::BadRequest(format!("invalid {field} timestamp")))?;
            Ok(Some(parsed.with_timezone(&Utc)))
        }
        None => Ok(None),
    }
}

pub async fn record_click(pool: &PgPool, id: i64) -> AppResult<()> {
    repo::articles::increment_click(pool, id).await?;
    Ok(())
}

pub async fn list_featured(pool: &PgPool, limit: i64) -> AppResult<Vec<ArticleOut>> {
    let rows = repo::articles::list_top_articles(pool, limit).await?;
    Ok(rows
        .into_iter()
        .map(|row| ArticleOut {
            id: row.id,
            title: row.title,
            url: row.url,
            description: row.description,
            language: row.language,
            source_domain: row.source_domain,
            published_at: row.published_at.to_rfc3339(),
            click_count: row.click_count,
        })
        .collect())
}
