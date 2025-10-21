use sqlx::{Executor, PgPool};
use tracing::info;

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
          enabled                    BOOLEAN NOT NULL DEFAULT TRUE,
          fetch_interval_seconds     INTEGER NOT NULL DEFAULT 600,
          filter_condition           TEXT,
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
          DROP COLUMN IF EXISTS country,
          DROP COLUMN IF EXISTS language,
          DROP COLUMN IF EXISTS last_modified;
        "#,
    )
    .await?;

    tx.execute(
        r#"
        ALTER TABLE news.feeds
          ADD COLUMN IF NOT EXISTS filter_condition TEXT;
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
          fetched_at           TIMESTAMPTZ NOT NULL DEFAULT NOW(),
          canonical_id         BIGINT
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
        ALTER TABLE news.articles
          ADD COLUMN IF NOT EXISTS click_count BIGINT NOT NULL DEFAULT 0;
        "#,
    )
    .await?;

    tx.execute(
        r#"
        ALTER TABLE news.articles
          ADD COLUMN IF NOT EXISTS canonical_id BIGINT;
        "#,
    )
    .await?;

    tx.execute(
        r#"
        UPDATE news.articles
        SET canonical_id = id
        WHERE canonical_id IS NULL;
        "#,
    )
    .await?;

    tx.execute(
        r#"
        DO $$
        BEGIN
            IF NOT EXISTS (
                SELECT 1 FROM information_schema.table_constraints
                WHERE table_schema = 'news'
                  AND table_name = 'articles'
                  AND constraint_name = 'articles_canonical_id_fkey'
            ) THEN
                ALTER TABLE news.articles
                    ADD CONSTRAINT articles_canonical_id_fkey
                    FOREIGN KEY (canonical_id)
                    REFERENCES news.articles(id)
                    ON DELETE SET NULL;
            END IF;
        END
        $$;
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

    tx.execute(
        r#"
        CREATE TABLE IF NOT EXISTS news.article_sources (
          id            BIGSERIAL PRIMARY KEY,
          article_id    BIGINT NOT NULL REFERENCES news.articles(id) ON DELETE CASCADE,
          feed_id       BIGINT REFERENCES news.feeds(id) ON DELETE SET NULL,
          source_name   TEXT,
          source_url    TEXT NOT NULL,
          published_at  TIMESTAMPTZ,
          inserted_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
          decision      TEXT,
          confidence    REAL
        );
        "#,
    )
    .await?;

    tx.execute(
        r#"
        CREATE UNIQUE INDEX IF NOT EXISTS idx_article_sources_article_url
          ON news.article_sources(article_id, source_url);
        "#,
    )
    .await?;

    tx.execute(
        r#"
        CREATE TABLE IF NOT EXISTS news.settings (
          key        TEXT PRIMARY KEY,
          value      TEXT NOT NULL,
          updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        );
        "#,
    )
    .await?;

    let deleted = sqlx::query_scalar::<_, i64>(
        r#"
        WITH duplicates AS (
            SELECT a.id
            FROM news.articles a
            JOIN news.articles b
              ON a.feed_id IS NOT DISTINCT FROM b.feed_id
             AND a.url = b.url
             AND a.id > b.id
        )
        DELETE FROM news.articles
        WHERE id IN (SELECT id FROM duplicates)
        RETURNING 1::bigint;
        "#,
    )
    .fetch_all(&mut *tx)
    .await?
    .len();

    if deleted > 0 {
        info!(
            count = deleted,
            "removed duplicate articles before creating unique index"
        );
    }

    tx.execute(
        r#"
        CREATE UNIQUE INDEX IF NOT EXISTS idx_articles_feed_id_url ON news.articles(feed_id, url);
        "#,
    )
    .await?;

    // ops schema and events table for notification center (Phase 1)
    tx.execute(
        r#"
        CREATE SCHEMA IF NOT EXISTS ops;
        "#,
    )
    .await?;

    tx.execute(
        r#"
        CREATE TABLE IF NOT EXISTS ops.events (
          id          BIGSERIAL PRIMARY KEY,
          ts          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
          level       TEXT NOT NULL,
          code        TEXT NOT NULL,
          title       TEXT NOT NULL,
          message     TEXT NOT NULL,
          attrs       JSONB NOT NULL DEFAULT '{}'::jsonb,
          source      TEXT NOT NULL,
          dedupe_key  TEXT,
          count       INTEGER NOT NULL DEFAULT 1
        );
        "#,
    )
    .await?;

    tx.execute(
        r#"
        CREATE INDEX IF NOT EXISTS idx_ops_events_ts ON ops.events(ts DESC);
        "#,
    )
    .await?;

    tx.commit().await?;
    Ok(())
}
