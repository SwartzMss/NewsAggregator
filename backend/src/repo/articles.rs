use chrono::{DateTime, Utc};
use sqlx::PgPool;

#[derive(Debug, sqlx::FromRow)]
pub struct ArticleRow {
    pub id: i64,
    pub title: String,
    pub url: String,
    pub description: Option<String>,
    pub language: Option<String>,
    pub source_domain: String,
    pub published_at: DateTime<Utc>,
}

pub struct ArticleListArgs {
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Debug, Clone)]
pub struct NewArticle {
    pub feed_id: Option<i64>,
    pub title: String,
    pub url: String,
    pub description: Option<String>,
    pub language: Option<String>,
    pub source_domain: String,
    pub published_at: DateTime<Utc>,
}

pub async fn list_articles(
    pool: &PgPool,
    args: ArticleListArgs,
) -> Result<(Vec<ArticleRow>, i64), sqlx::Error> {
    let rows = sqlx::query_as::<_, ArticleRow>(
        r#"
        SELECT id,
               title,
               url,
               description,
               language,
               source_domain,
               published_at
        FROM news.articles
        WHERE ($1::timestamptz IS NULL OR published_at >= $1)
          AND ($2::timestamptz IS NULL OR published_at <= $2)
        ORDER BY published_at DESC
        LIMIT $3
        OFFSET $4
        "#,
    )
    .bind(args.from)
    .bind(args.to)
    .bind(args.limit)
    .bind(args.offset)
    .fetch_all(pool)
    .await?;

    let total = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COUNT(*)
        FROM news.articles
        WHERE ($1::timestamptz IS NULL OR published_at >= $1)
          AND ($2::timestamptz IS NULL OR published_at <= $2)
        "#,
    )
    .bind(args.from)
    .bind(args.to)
    .fetch_one(pool)
    .await?;

    Ok((rows, total))
}

pub async fn insert_articles(pool: &PgPool, articles: Vec<NewArticle>) -> Result<(), sqlx::Error> {
    if articles.is_empty() {
        return Ok(());
    }

    let mut tx = pool.begin().await?;
    for article in articles {
        sqlx::query(
            r#"
            INSERT INTO news.articles (
                feed_id,
                title,
                url,
                description,
                language,
                source_domain,
                published_at,
                fetched_at
            )
            VALUES (
                $1, $2, $3, $4, $5, $6, $7, NOW()
            )
            "#,
        )
        .bind(article.feed_id)
        .bind(article.title)
        .bind(article.url)
        .bind(article.description)
        .bind(article.language)
        .bind(article.source_domain)
        .bind(article.published_at)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(())
}
