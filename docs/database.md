# 数据库指南

项目使用 PostgreSQL，主要存储在 `news` schema 下。当前表结构包括：

- `feeds`：订阅源配置；
- `articles`：入库文章；
- `article_sources`：文章与来源的关联记录（用于展示和去重追踪）；
- `settings`：系统级键值配置（例如翻译服务提供商、Deepseek/Baidu 的凭据）。

## 初始化建表 SQL
第一次部署时执行以下 SQL（`canonical_id` 自指向，记录主文章；`article_sources` 保存多来源信息）：

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
  published_at         TIMESTAMPTZ NOT NULL,
  fetched_at           TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  canonical_id         BIGINT
);

CREATE INDEX IF NOT EXISTS idx_articles_published_at  ON news.articles(published_at DESC);
CREATE INDEX IF NOT EXISTS idx_articles_language      ON news.articles(language);
CREATE INDEX IF NOT EXISTS idx_articles_source_domain ON news.articles(source_domain);

ALTER TABLE news.articles
  ADD CONSTRAINT IF NOT EXISTS articles_canonical_id_fkey
  FOREIGN KEY (canonical_id) REFERENCES news.articles(id) ON DELETE SET NULL;

UPDATE news.articles SET canonical_id = id WHERE canonical_id IS NULL;

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

CREATE UNIQUE INDEX IF NOT EXISTS idx_article_sources_article_url
  ON news.article_sources(article_id, source_url);

CREATE TABLE IF NOT EXISTS news.settings (
  key        TEXT PRIMARY KEY,
  value      TEXT NOT NULL,
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
```

## 字段说明
- `source_domain` 在 `feeds` 与 `articles` 中重复保存，方便筛选与展示，避免 JOIN。
- `last_etag`、`last_modified` 支持抓取时发送条件请求，节省带宽。
- `fail_count` 记录连续失败次数，可据此实现退避或熔断策略。
- `canonical_id` 标识主文章（默认指向自身），后续如需归并可指向原始文章。
- `news.article_sources` 记录每篇文章被哪些来源收录以及判定原因/置信度，可用于展示“多源引用”或调试去重逻辑。
  - `decision` 说明这条记录的判定来源：`primary` 表示这是文章首次入库的来源；`recent_jaccard` 表示最近文章的标题相似度超过严格阈值而被判定为重复；其他字符串通常来自 DeepSeek 的判定结果（例如 `deepseek_duplicate` 或模型返回的自定义理由）。
  - `confidence` 搭配 `decision` 使用，在 DeepSeek 判定时保存模型输出的置信度，便于后续追踪阈值与误判。
- `news.settings` 为简单的键值对表（`key` 唯一），目前用于存放翻译相关配置：
  - `translation.provider`：当前默认翻译服务（`deepseek` 或 `baidu`）。
  - `translation.deepseek_api_key`：Deepseek API Key。
  - `translation.baidu_app_id` / `translation.baidu_secret_key`：百度翻译凭据。
  这些值可在后台控制台实时更新，服务启动时会读取并注册到翻译引擎。

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

**写入文章（去重 + 返回 ID）**
```sql
WITH inserted AS (
  INSERT INTO news.articles (
      feed_id, title, url, description, language, source_domain, published_at
  )
  VALUES ($1, $2, $3, $4, $5, $6, $7)
  ON CONFLICT (feed_id, url) DO NOTHING
  RETURNING id
)
SELECT id FROM inserted;
```

**记录文章来源（可用于重复判定原因追踪）**
```sql
INSERT INTO news.article_sources (
    article_id, feed_id, source_name, source_url, published_at, decision, confidence
)
VALUES ($1, $2, $3, $4, $5, $6, $7)
ON CONFLICT (article_id, source_url) DO NOTHING;
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
