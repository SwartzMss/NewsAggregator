use sqlx::{PgPool, Postgres, Transaction};
use tracing::info;

pub async fn cleanup_orphan_content(pool: &PgPool) -> Result<(u64, u64), sqlx::Error> {
    let mut tx: Transaction<'_, Postgres> = pool.begin().await?;

    let deleted_article_sources = sqlx::query(
        r#"
        DELETE FROM news.article_sources
        WHERE feed_id IS NULL
        "#,
    )
    .execute(tx.as_mut())
    .await?
    .rows_affected();

    let deleted_articles = sqlx::query(
        r#"
        DELETE FROM news.articles
        WHERE feed_id IS NULL
        "#,
    )
    .execute(tx.as_mut())
    .await?
    .rows_affected();

    tx.commit().await?;

    if deleted_articles > 0 || deleted_article_sources > 0 {
        info!(
            deleted_articles,
            deleted_article_sources, "cleaned orphaned content left from removed feeds"
        );
    }

    Ok((deleted_articles, deleted_article_sources))
}
