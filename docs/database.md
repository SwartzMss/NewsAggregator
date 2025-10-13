# 数据库指南

项目使用 PostgreSQL，结构极简：`news` schema 下只有 `feeds` 与 `articles` 两张表。

## 初始化建表 SQL
第一次部署时执行以下 SQL：

```sql
CREATE SCHEMA IF NOT EXISTS news;

CREATE TABLE IF NOT EXISTS news.feeds (
  id                         BIGSERIAL PRIMARY KEY,
  url                        TEXT NOT NULL UNIQUE,
  title                      TEXT,
  site_url                   TEXT,
  source_domain              TEXT NOT NULL,
  source_display_name        TEXT,
  language                   TEXT,
  country                    TEXT,
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

CREATE INDEX IF NOT EXISTS idx_feeds_enabled ON news.feeds(enabled);

CREATE TABLE IF NOT EXISTS news.articles (
  id                   BIGSERIAL PRIMARY KEY,
  feed_id              BIGINT REFERENCES news.feeds(id) ON DELETE SET NULL,
  title                TEXT NOT NULL,
  url                  TEXT NOT NULL,
  description          TEXT,
  language             TEXT,
  source_domain        TEXT NOT NULL,
  source_display_name  TEXT,
  published_at         TIMESTAMPTZ NOT NULL,
  fetched_at           TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_articles_published_at  ON news.articles(published_at DESC);
CREATE INDEX IF NOT EXISTS idx_articles_language      ON news.articles(language);
CREATE INDEX IF NOT EXISTS idx_articles_source_domain ON news.articles(source_domain);
```

## 字段说明
- `source_domain` / `source_display_name` 在 `feeds` 与 `articles` 中重复保存，方便筛选与展示，避免 JOIN。
- `last_etag`、`last_modified` 支持抓取时发送条件请求，节省带宽。
- `fail_count` 记录连续失败次数，可据此实现退避或熔断策略。

## 常用 SQL 示例
**插入或更新 Feed（按 URL upsert）**
```sql
INSERT INTO news.feeds
  (url, title, site_url, source_domain, source_display_name, language, country,
   enabled, fetch_interval_seconds)
VALUES
  ($1, $2, $3, $4, $5, $6, $7, $8, $9)
ON CONFLICT (url) DO UPDATE SET
  title = COALESCE(EXCLUDED.title, news.feeds.title),
  site_url = COALESCE(EXCLUDED.site_url, news.feeds.site_url),
  source_domain = EXCLUDED.source_domain,
  source_display_name = COALESCE(EXCLUDED.source_display_name, news.feeds.source_display_name),
  language = COALESCE(EXCLUDED.language, news.feeds.language),
  country = COALESCE(EXCLUDED.country, news.feeds.country),
  enabled = EXCLUDED.enabled,
  fetch_interval_seconds = EXCLUDED.fetch_interval_seconds,
  updated_at = NOW()
RETURNING id;
```

**写入文章（允许重复）**
```sql
INSERT INTO news.articles
  (feed_id, title, url, description, language, source_domain, source_display_name, published_at)
VALUES
  ($1, $2, $3, $4, $5, $6, $7, $8);
```

**按时间倒序查询文章（可选语言/来源过滤）**
```sql
SELECT id, title, url, description, language,
       source_domain, source_display_name, published_at
FROM news.articles
WHERE ($1::text IS NULL OR language = $1)
  AND ($2::text IS NULL OR source_domain = $2)
  AND published_at BETWEEN $3 AND $4
ORDER BY published_at DESC
LIMIT $5 OFFSET $6;
```

## 运维建议
- 所有时间列建议保持 UTC，前端负责本地化展示。
- 抓取量大时定期 `VACUUM ANALYZE news.*`，保持统计信息新鲜。
- 根据业务需要设定留存策略，例如定期清理过旧文章或归档到冷库。
