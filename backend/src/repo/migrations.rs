use sqlx::{Executor, PgPool};

pub async fn ensure_schema(pool: &PgPool) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;

    tx.execute(
        r#"
        CREATE SCHEMA IF NOT EXISTS news;
        "#,
    )
    .await?;

    tx.execute(
        r#"
        CREATE TABLE IF NOT EXISTS news.feeds (
          id                         BIGSERIAL PRIMARY KEY,
          url                        TEXT NOT NULL UNIQUE,
          title                      TEXT,
          site_url                   TEXT,
          source_domain              TEXT NOT NULL,
          language                   TEXT,
          enabled                    BOOLEAN NOT NULL DEFAULT TRUE,
          fetch_interval_seconds     INTEGER NOT NULL DEFAULT 600,
          last_etag                  TEXT,
          last_modified              TIMESTAMPTZ,
          last_fetch_at              TIMESTAMPTZ,
          last_fetch_status          SMALLINT,
          fail_count                 INTEGER NOT NULL DEFAULT 0,
          created_at                 TIMESTAMPTZ NOT NULL DEFAULT NOW(),
          updated_at                 TIMESTAMPTZ NOT NULL DEFAULT NOW()
        );
        "#,
    )
    .await?;

    tx.execute(
        r#"
        ALTER TABLE news.feeds
          DROP COLUMN IF EXISTS source_display_name,
          DROP COLUMN IF EXISTS country;
        "#,
    )
    .await?;

    tx.execute(
        r#"
        CREATE INDEX IF NOT EXISTS idx_feeds_enabled ON news.feeds(enabled);
        "#,
    )
    .await?;

    tx.execute(
        r#"
        CREATE TABLE IF NOT EXISTS news.articles (
          id                   BIGSERIAL PRIMARY KEY,
          feed_id              BIGINT REFERENCES news.feeds(id) ON DELETE SET NULL,
          title                TEXT NOT NULL,
          url                  TEXT NOT NULL,
          description          TEXT,
          language             TEXT,
          source_domain        TEXT NOT NULL,
          published_at         TIMESTAMPTZ NOT NULL,
          fetched_at           TIMESTAMPTZ NOT NULL DEFAULT NOW()
        );
        "#,
    )
    .await?;

    tx.execute(
        r#"
        ALTER TABLE news.articles
          DROP COLUMN IF EXISTS source_display_name;
        "#,
    )
    .await?;

    tx.execute(
        r#"
        CREATE INDEX IF NOT EXISTS idx_articles_published_at    ON news.articles(published_at DESC);
        "#,
    )
    .await?;

    tx.execute(
        r#"
        CREATE INDEX IF NOT EXISTS idx_articles_language        ON news.articles(language);
        "#,
    )
    .await?;

    tx.execute(
        r#"
        CREATE INDEX IF NOT EXISTS idx_articles_source_domain   ON news.articles(source_domain);
        "#,
    )
    .await?;

    tx.commit().await?;
    Ok(())
}
