use chrono::{DateTime, Utc};
use sqlx::{postgres::PgQueryResult, PgPool, Postgres, Row, Transaction};

#[derive(Debug, sqlx::FromRow)]
pub struct ArticleRow {
    pub id: i64,
    pub title: String,
    pub url: String,
    pub description: Option<String>,
    pub language: Option<String>,
    pub source_domain: String,
    pub published_at: DateTime<Utc>,
    pub click_count: i64,
}

pub struct ArticleListArgs {
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
    pub keyword: Option<String>,
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
    let keyword = args.keyword.as_ref().map(|value| format!("%{}%", value));

    let rows = sqlx::query_as::<_, ArticleRow>(
        r#"
        SELECT id::bigint AS id,
               title,
               url,
               description,
               language,
               source_domain,
               published_at,
               click_count::bigint AS click_count
        FROM news.articles
        WHERE ($1::timestamptz IS NULL OR published_at >= $1)
          AND ($2::timestamptz IS NULL OR published_at <= $2)
          AND ($3::text IS NULL OR title ILIKE $3)
        ORDER BY published_at DESC
        LIMIT $4
        OFFSET $5
        "#,
    )
    .bind(args.from)
    .bind(args.to)
    .bind(keyword.as_deref())
    .bind(args.limit)
    .bind(args.offset)
    .fetch_all(pool)
    .await?;

    let total = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COUNT(*)::bigint
        FROM news.articles
        WHERE ($1::timestamptz IS NULL OR published_at >= $1)
          AND ($2::timestamptz IS NULL OR published_at <= $2)
          AND ($3::text IS NULL OR title ILIKE $3)
        "#,
    )
    .bind(args.from)
    .bind(args.to)
    .bind(keyword.as_deref())
    .fetch_one(pool)
    .await?;

    Ok((rows, total))
}

pub async fn insert_articles(
    pool: &PgPool,
    articles: Vec<NewArticle>,
) -> Result<Vec<(i64, NewArticle)>, sqlx::Error> {
    if articles.is_empty() {
        return Ok(Vec::new());
    }

    let mut inserted = Vec::new();

    let mut tx = pool.begin().await?;
    for article in articles {
        let row = sqlx::query(
            r#"
            INSERT INTO news.articles (
                feed_id,
                title,
                url,
                description,
                language,
                source_domain,
                published_at,
                fetched_at,
                click_count
            )
            VALUES (
                $1, $2, $3, $4, $5, $6, $7, NOW(), 0
            )
            ON CONFLICT (feed_id, url) DO NOTHING
            RETURNING id::bigint AS id
            "#,
        )
        .bind(article.feed_id)
        .bind(&article.title)
        .bind(&article.url)
        .bind(&article.description)
        .bind(&article.language)
        .bind(&article.source_domain)
        .bind(article.published_at)
        .fetch_optional(&mut *tx)
        .await?;

        if let Some(row) = row {
            let article_id: i64 = row.get("id");
            sqlx::query(
                r#"
                UPDATE news.articles
                SET canonical_id = COALESCE(canonical_id, id)
                WHERE id = $1
                "#,
            )
            .bind(article_id)
            .execute(&mut *tx)
            .await?;

            inserted.push((article_id, article.clone()));
        }
    }

    tx.commit().await?;
    Ok(inserted)
}

pub async fn delete_by_feed(
    tx: &mut Transaction<'_, Postgres>,
    feed_id: i64,
) -> Result<u64, sqlx::Error> {
    let result: PgQueryResult = sqlx::query(
        r#"
        DELETE FROM news.articles
        WHERE feed_id = $1
        "#,
    )
    .bind(feed_id)
    .execute(tx.as_mut())
    .await?;

    Ok(result.rows_affected())
}

pub async fn increment_click(pool: &PgPool, id: i64) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        UPDATE news.articles
        SET click_count = click_count + 1
        WHERE id = $1
        "#,
    )
    .bind(id)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn list_top_articles(pool: &PgPool, limit: i64) -> Result<Vec<ArticleRow>, sqlx::Error> {
    sqlx::query_as::<_, ArticleRow>(
        r#"
        SELECT id::bigint AS id,
               title,
               url,
               description,
               language,
               source_domain,
               published_at,
               click_count::bigint AS click_count
        FROM news.articles
        WHERE published_at >= NOW() - INTERVAL '24 HOURS'
        ORDER BY click_count DESC, published_at DESC
        LIMIT $1
        "#,
    )
    .bind(limit)
    .fetch_all(pool)
    .await
}

pub async fn list_recent_articles(
    pool: &PgPool,
    limit: i64,
) -> Result<Vec<ArticleRow>, sqlx::Error> {
    sqlx::query_as::<_, ArticleRow>(
        r#"
        SELECT id::bigint AS id,
               title,
               url,
               description,
               language,
               source_domain,
               published_at,
               click_count::bigint AS click_count
        FROM news.articles
        ORDER BY published_at DESC
        LIMIT $1
        "#,
    )
    .bind(limit)
    .fetch_all(pool)
    .await
}

pub async fn apply_filter_condition(
    pool: &PgPool,
    feed_id: i64,
    condition: &str,
) -> Result<u64, sqlx::Error> {
    let sql = format!(
        "DELETE FROM news.articles WHERE feed_id = $1 AND NOT ({})",
        condition
    );
    let result = sqlx::query(&sql).bind(feed_id).execute(pool).await?;
    Ok(result.rows_affected())
}
