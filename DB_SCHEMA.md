
# 数据库设计（MVP 极简）

> 适用于 **全新数据库** 安装：仅创建 `news.feeds` 与 `news.articles` 两张表；允许重复文章；不做模糊/全文检索。

---

## 1) 一次性 DDL（直接执行）

```sql
-- Schema
CREATE SCHEMA IF NOT EXISTS news;

-- =========================
-- 订阅源：每条就是一个 RSS/Atom 频道
-- =========================
CREATE TABLE IF NOT EXISTS news.feeds (
  id                         BIGSERIAL PRIMARY KEY,
  url                        TEXT NOT NULL UNIQUE,     -- RSS 源地址
  title                      TEXT,                     -- channel.title（抓到后回填）
  site_url                   TEXT,                     -- 频道/站点主页

  -- 直接保存来源信息在 feed 上
  source_domain              TEXT NOT NULL,            -- 例如 reuters.com（去掉 www.）
  source_display_name        TEXT,                     -- 例如 Reuters（可空）

  language                   TEXT,                     -- 频道默认语言（可空）
  country                    TEXT,                     -- 频道默认国家（可空）

  enabled                    BOOLEAN NOT NULL DEFAULT TRUE,
  fetch_interval_seconds     INTEGER NOT NULL DEFAULT 600,

  -- 条件请求与抓取状态（省流/排错）
  last_etag                  TEXT,
  last_modified              TIMESTAMPTZ,
  last_fetch_at              TIMESTAMPTZ,
  last_fetch_status          SMALLINT,                 -- 200/304/429/5xx...
  fail_count                 INTEGER NOT NULL DEFAULT 0,

  created_at                 TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at                 TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_feeds_enabled ON news.feeds(enabled);

-- =========================
-- 文章：允许重复（本期不做去重/唯一约束）
-- =========================
CREATE TABLE IF NOT EXISTS news.articles (
  id                   BIGSERIAL PRIMARY KEY,

  feed_id              BIGINT REFERENCES news.feeds(id) ON DELETE SET NULL,

  title                TEXT NOT NULL,
  url                  TEXT NOT NULL,
  description          TEXT,
  language             TEXT,

  -- 直接记录来源字段，便于筛选与展示
  source_domain        TEXT NOT NULL,                  -- 例如 reuters.com
  source_display_name  TEXT,                           -- 例如 Reuters（可空）

  published_at         TIMESTAMPTZ NOT NULL,           -- 统一 UTC
  fetched_at           TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- 基础查询索引：按时间倒序 + 语言/来源筛选
CREATE INDEX IF NOT EXISTS idx_articles_published_at    ON news.articles(published_at DESC);
CREATE INDEX IF NOT EXISTS idx_articles_language        ON news.articles(language);
CREATE INDEX IF NOT EXISTS idx_articles_source_domain   ON news.articles(source_domain);
```

---

## 2) 字段说明

### 2.1 `news.feeds`（RSS 频道）

| 列名 | 类型 | 约束 | 说明 |
|---|---|---|---|
| id | BIGSERIAL | PK | 自增主键 |
| url | TEXT | UNIQUE, NOT NULL | RSS/Atom 源地址 |
| title | TEXT |  | RSS `channel.title`，抓到后回填 |
| site_url | TEXT |  | 频道/站点主页 |
| source_domain | TEXT | NOT NULL | 来源域名（如 `reuters.com`，从 `url/site_url` 解析或前端提交） |
| source_display_name | TEXT |  | 来源展示名（如 `Reuters`），可为空 |
| language | TEXT |  | 频道默认语言（可空） |
| country | TEXT |  | 频道默认国家/地区（可空） |
| enabled | BOOLEAN | DEFAULT true | 是否启用抓取 |
| fetch_interval_seconds | INTEGER | DEFAULT 600 | 抓取间隔（秒） |
| last_etag | TEXT |  | 上次响应的 ETag（条件请求） |
| last_modified | TIMESTAMPTZ |  | 上次响应的 Last-Modified（条件请求） |
| last_fetch_at | TIMESTAMPTZ |  | 上次抓取时间（UTC） |
| last_fetch_status | SMALLINT |  | 上次抓取 HTTP 状态码（200/304/429/5xx） |
| fail_count | INTEGER | DEFAULT 0 | 连续失败次数（用于退避/熔断） |
| created_at | TIMESTAMPTZ | DEFAULT now() | 创建时间 |
| updated_at | TIMESTAMPTZ | DEFAULT now() | 最近更新时间 |

> 索引：`idx_feeds_enabled(enabled)`。

### 2.2 `news.articles`（文章）

| 列名 | 类型 | 约束 | 说明 |
|---|---|---|---|
| id | BIGSERIAL | PK | 自增主键 |
| feed_id | BIGINT | FK → news.feeds(id) ON DELETE SET NULL | 来源 feed（频道）ID |
| title | TEXT | NOT NULL | 标题 |
| url | TEXT | NOT NULL | 原文链接 |
| description | TEXT |  | 摘要（可空） |
| language | TEXT |  | 语言代码（可空） |
| source_domain | TEXT | NOT NULL | 来源域名（与 `feeds.source_domain` 对齐） |
| source_display_name | TEXT |  | 来源展示名（可空） |
| published_at | TIMESTAMPTZ | NOT NULL | 发布时间（UTC；解析失败可回退为抓取时刻） |
| fetched_at | TIMESTAMPTZ | DEFAULT now() | 抓取入库时间 |

> 索引：  
> `idx_articles_published_at(published_at DESC)`  
> `idx_articles_language(language)`  
> `idx_articles_source_domain(source_domain)`

---

## 3) 最小 SQL 示例

**写入 feed（新增或忽略重复 URL）**
```sql
INSERT INTO news.feeds
(url, title, site_url, source_domain, source_display_name, language, country, enabled, fetch_interval_seconds)
VALUES
($1,  $2,    $3,      $4,             $5,                  $6,       $7,       $8,      $9)
ON CONFLICT (url) DO UPDATE
SET title = COALESCE(EXCLUDED.title, news.feeds.title),
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
(feed_id, title, url, description, language, source_domain, source_display_name, published_at, fetched_at)
VALUES
($1,      $2,    $3,  $4,          $5,       $6,            $7,                  $8,          NOW());
```

**文章列表（按时间倒序 + 精确筛选）**
```sql
SELECT id, title, url, description, language, source_domain, source_display_name, published_at
FROM news.articles
WHERE ($1::text IS NULL OR language = $1)
  AND ($2::text IS NULL OR source_domain = $2)
  AND published_at BETWEEN $3 AND $4
ORDER BY published_at DESC
LIMIT $5 OFFSET $6;
```

---

## 4) 约定与建议
- 全部时间字段统一为 **UTC**；前端自行本地化显示。
- 解析 `source_domain` 时去除 `www.` 等前缀。
- 先不做去重/模糊检索；后续需要时再新增字段与索引（不破坏现有表）。
