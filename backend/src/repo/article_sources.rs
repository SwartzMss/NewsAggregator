use chrono::{DateTime, Utc};
use sqlx::{postgres::PgQueryResult, PgPool, Postgres, Transaction};

#[derive(Debug, Clone)]
pub struct ArticleSourceRecord {
    pub article_id: i64,
    pub feed_id: Option<i64>,
    pub source_name: Option<String>,
    pub source_url: String,
    pub published_at: DateTime<Utc>,
    pub decision: Option<String>,
    pub confidence: Option<f32>,
}

pub async fn insert_source(pool: &PgPool, record: ArticleSourceRecord) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO news.article_sources (
            article_id,
            feed_id,
            source_name,
            source_url,
            published_at,
            inserted_at,
            decision,
            confidence
        )
        VALUES (
            $1, $2, $3, $4, $5, NOW(), $6, $7
        )
        ON CONFLICT (article_id, source_url) DO NOTHING
        "#,
    )
    .bind(record.article_id)
    .bind(record.feed_id)
    .bind(record.source_name)
    .bind(record.source_url)
    .bind(record.published_at)
    .bind(record.decision)
    .bind(record.confidence)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn delete_by_feed(
    tx: &mut Transaction<'_, Postgres>,
    feed_id: i64,
) -> Result<u64, sqlx::Error> {
    let result: PgQueryResult = sqlx::query(
        r#"
        DELETE FROM news.article_sources
        WHERE feed_id = $1
        "#,
    )
    .bind(feed_id)
    .execute(tx.as_mut())
    .await?;

    Ok(result.rows_affected())
}
