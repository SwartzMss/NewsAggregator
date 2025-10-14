use chrono::{DateTime, Utc};
use sqlx::{postgres::PgQueryResult, PgConnection, PgPool, Postgres, Transaction};

#[derive(Debug, sqlx::FromRow)]
pub struct FeedRow {
    pub id: i64,
    pub url: String,
    pub title: Option<String>,
    pub site_url: Option<String>,
    pub source_domain: String,
    pub enabled: bool,
    pub fetch_interval_seconds: i32,
    pub filter_condition: Option<String>,
    pub last_fetch_at: Option<DateTime<Utc>>,
    pub last_fetch_status: Option<i16>,
    pub fail_count: i32,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct DueFeedRow {
    pub id: i64,
    pub url: String,
    pub source_domain: String,
    pub last_etag: Option<String>,
    pub filter_condition: Option<String>,
}

pub struct FeedUpsertRecord {
    pub url: String,
    pub title: Option<String>,
    pub site_url: Option<String>,
    pub source_domain: String,
    pub enabled: Option<bool>,
    pub fetch_interval_seconds: Option<i32>,
    pub filter_condition: Option<String>,
}

pub async fn list_feeds(pool: &PgPool) -> Result<Vec<FeedRow>, sqlx::Error> {
    sqlx::query_as::<_, FeedRow>(
        r#"
        SELECT id::bigint AS id,
               url,
               title,
               site_url,
               source_domain,
               enabled,
               fetch_interval_seconds,
               filter_condition,
               last_fetch_at,
               last_fetch_status,
               fail_count
        FROM news.feeds
        ORDER BY id DESC
        "#,
    )
    .fetch_all(pool)
    .await
}

pub async fn list_due_feeds(pool: &PgPool, limit: i64) -> Result<Vec<DueFeedRow>, sqlx::Error> {
    sqlx::query_as::<_, DueFeedRow>(
        r#"
        SELECT id::bigint AS id,
               url,
               source_domain,
               last_etag,
               filter_condition
        FROM news.feeds
        WHERE enabled = TRUE
          AND (
              last_fetch_at IS NULL OR
              last_fetch_at <= NOW() - make_interval(secs => fetch_interval_seconds)
          )
        ORDER BY last_fetch_at NULLS FIRST
        LIMIT $1
        "#,
    )
    .bind(limit)
    .fetch_all(pool)
    .await
}

pub async fn find_by_url(pool: &PgPool, url: &str) -> Result<Option<FeedRow>, sqlx::Error> {
    sqlx::query_as::<_, FeedRow>(
        r#"
        SELECT id::bigint AS id,
               url,
               title,
               site_url,
               source_domain,
               enabled,
               fetch_interval_seconds,
               filter_condition,
               last_fetch_at,
               last_fetch_status,
               fail_count
        FROM news.feeds
        WHERE url = $1
        "#,
    )
    .bind(url)
    .fetch_optional(pool)
    .await
}

pub async fn upsert_feed(pool: &PgPool, record: FeedUpsertRecord) -> Result<FeedRow, sqlx::Error> {
    sqlx::query_as::<_, FeedRow>(
        r#"
        INSERT INTO news.feeds (
            url,
            title,
            site_url,
            source_domain,
            enabled,
            fetch_interval_seconds,
            filter_condition
        )
        VALUES (
            $1,
            $2,
            $3,
            $4,
            COALESCE($5, TRUE),
            COALESCE($6, 600),
            NULLIF(trim($7), '')
        )
        ON CONFLICT (url) DO UPDATE SET
            title = COALESCE(EXCLUDED.title, news.feeds.title),
            site_url = COALESCE(EXCLUDED.site_url, news.feeds.site_url),
            source_domain = EXCLUDED.source_domain,
            enabled = COALESCE(EXCLUDED.enabled, news.feeds.enabled),
            fetch_interval_seconds = COALESCE(EXCLUDED.fetch_interval_seconds, news.feeds.fetch_interval_seconds),
            filter_condition = EXCLUDED.filter_condition,
            updated_at = NOW()
        RETURNING id::bigint AS id,
                  url,
                  title,
                  site_url,
                  source_domain,
                  enabled,
                  fetch_interval_seconds,
                  filter_condition,
                  last_fetch_at,
                  last_fetch_status,
                  fail_count
        "#,
    )
    .bind(record.url)
    .bind(record.title)
    .bind(record.site_url)
    .bind(record.source_domain)
    .bind(record.enabled)
    .bind(record.fetch_interval_seconds)
    .bind(record.filter_condition)
    .fetch_one(pool)
    .await
}

pub async fn delete_feed(tx: &mut Transaction<'_, Postgres>, id: i64) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        r#"
        DELETE FROM news.feeds
        WHERE id = $1
        "#,
    )
    .bind(id)
    .execute(tx.as_mut())
    .await?;

    Ok(result.rows_affected())
}

pub async fn mark_not_modified(
    pool: &PgPool,
    feed_id: i64,
    status: i16,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        UPDATE news.feeds
        SET last_fetch_at = NOW(),
            last_fetch_status = $2,
            fail_count = 0,
            updated_at = NOW()
        WHERE id = $1
        "#,
    )
    .bind(feed_id)
    .bind(status)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn mark_failure(pool: &PgPool, feed_id: i64, status: i16) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        UPDATE news.feeds
        SET last_fetch_at = NOW(),
            last_fetch_status = $2,
            fail_count = fail_count + 1,
            updated_at = NOW()
        WHERE id = $1
        "#,
    )
    .bind(feed_id)
    .bind(status)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn mark_success(
    pool: &PgPool,
    feed_id: i64,
    status: i16,
    etag: Option<String>,
    title: Option<String>,
    site_url: Option<String>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        UPDATE news.feeds
        SET last_fetch_at = NOW(),
            last_fetch_status = $2,
            last_etag = $3,
            title = COALESCE($4, title),
            site_url = COALESCE($5, site_url),
            fail_count = 0,
            updated_at = NOW()
        WHERE id = $1
        "#,
    )
    .bind(feed_id)
    .bind(status)
    .bind(etag)
    .bind(title)
    .bind(site_url)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn disable_feed(
    tx: &mut Transaction<'_, Postgres>,
    feed_id: i64,
) -> Result<u64, sqlx::Error> {
    let result: PgQueryResult = sqlx::query(
        r#"
        UPDATE news.feeds
        SET enabled = FALSE,
            updated_at = NOW()
        WHERE id = $1
        "#,
    )
    .bind(feed_id)
    .execute(tx.as_mut())
    .await?;

    Ok(result.rows_affected())
}

pub async fn acquire_processing_lock(
    conn: &mut PgConnection,
    feed_id: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query("SELECT pg_advisory_lock($1)")
        .bind(feed_id)
        .execute(&mut *conn)
        .await?;
    Ok(())
}

pub async fn release_processing_lock(
    conn: &mut PgConnection,
    feed_id: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query("SELECT pg_advisory_unlock($1)")
        .bind(feed_id)
        .execute(&mut *conn)
        .await?;
    Ok(())
}
