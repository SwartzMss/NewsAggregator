
# 数据库设计（MVP 极简版，无模糊匹配、无去重）

> 目标：**先能写入，再能读取**。不做重复判定、不做模糊/全文检索。后续再演进。

---

## 0. 范围与约定
- 只使用 **PostgreSQL**，创建独立 schema `news`。
- 不安装任何扩展（不需要 `pg_trgm` / `unaccent` / `tsvector`）。
- 允许**重复文章**（不做唯一约束、不做去重字段）。
- 只提供**按时间倒序**、**按语言/来源筛选**的基础查询索引。

---

## 1. Schema 初始化

```sql
CREATE SCHEMA IF NOT EXISTS news;
```

---

## 2. 表结构（极简）

### 2.1 `news.sources` —— 媒体来源（域名/机构）
```sql
CREATE TABLE IF NOT EXISTS news.sources (
  id            BIGSERIAL PRIMARY KEY,
  domain        TEXT NOT NULL UNIQUE,          -- 如 reuters.com
  display_name  TEXT,                          -- 如 "Reuters"
  created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
```

### 2.2 `news.feeds` —— RSS/Atom 订阅源
> 含条件请求与抓取状态字段；便于省流/排错。
```sql
CREATE TABLE IF NOT EXISTS news.feeds (
  id              BIGSERIAL PRIMARY KEY,
  url             TEXT NOT NULL UNIQUE,        -- RSS 源地址
  title           TEXT,                        -- channel.title（抓到后回填）
  site_url        TEXT,                        -- 频道/站点主页
  source_id       BIGINT REFERENCES news.sources(id),
  language        TEXT,                        -- 频道默认语言（可空）
  country         TEXT,                        -- 频道默认国家（可空）

  enabled         BOOLEAN NOT NULL DEFAULT TRUE,
  fetch_interval_seconds INTEGER NOT NULL DEFAULT 600,

  -- 条件请求与抓取状态
  last_etag         TEXT,
  last_modified     TIMESTAMPTZ,
  last_fetch_at     TIMESTAMPTZ,
  last_fetch_status SMALLINT,                  -- 200/304/429/5xx...
  fail_count        INTEGER NOT NULL DEFAULT 0,

  created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_feeds_enabled ON news.feeds(enabled);
```

### 2.3 `news.articles` —— 文章（无去重约束）
> **不做去重**：允许同一链接/标题多次写入。  
> **最少字段**：标题、链接、来源、语言、时间、摘要、关联 feed/source。
```sql
CREATE TABLE IF NOT EXISTS news.articles (
  id             BIGSERIAL PRIMARY KEY,

  feed_id        BIGINT REFERENCES news.feeds(id)   ON DELETE SET NULL,
  source_id      BIGINT REFERENCES news.sources(id) ON DELETE SET NULL,

  title          TEXT NOT NULL,
  url            TEXT NOT NULL,
  source         TEXT NOT NULL,                     -- 展示用：域名或频道名
  description    TEXT,
  language       TEXT,

  published_at   TIMESTAMPTZ NOT NULL,             -- 统一 UTC
  fetched_at     TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- 基础查询索引（按时间倒序 + 语言/来源筛选）
CREATE INDEX IF NOT EXISTS idx_articles_published_at ON news.articles(published_at DESC);
CREATE INDEX IF NOT EXISTS idx_articles_language     ON news.articles(language);
CREATE INDEX IF NOT EXISTS idx_articles_source_id    ON news.articles(source_id);
```

---

## 3. 写入与读取（示例）

### 3.1 Upsert 来源与 Feed（可选）
```sql
-- sources：存在则更新展示名，不存在则创建
INSERT INTO news.sources (domain, display_name)
VALUES ($1, $2)
ON CONFLICT (domain) DO UPDATE
SET display_name = EXCLUDED.display_name
RETURNING id;

-- feeds：存在则回填标题/站点，新增则创建
INSERT INTO news.feeds (url, title, site_url, source_id, language, country)
VALUES ($1, $2, $3, $4, $5, $6)
ON CONFLICT (url) DO UPDATE
SET title      = COALESCE(EXCLUDED.title, news.feeds.title),
    site_url   = COALESCE(EXCLUDED.site_url, news.feeds.site_url),
    updated_at = NOW()
RETURNING id;
```

### 3.2 插入文章（允许重复）
```sql
INSERT INTO news.articles
(feed_id, source_id, title, url, source, description, language, published_at, fetched_at)
VALUES
($1,     $2,        $3,    $4,  $5,     $6,          $7,        $8,          NOW());
```

### 3.3 基础列表/检索（只做精确筛选）
```sql
-- 按时间倒序 + 可选语言/来源（source_id）过滤 + 分页
SELECT id, title, url, source, description, language, published_at
FROM news.articles
WHERE ($1::text IS NULL OR language = $1)
  AND ($2::bigint IS NULL OR source_id = $2)
  AND published_at BETWEEN $3 AND $4
ORDER BY published_at DESC
LIMIT $5 OFFSET $6;
```

> 说明：若不传过滤条件，对应参数传 `NULL` 即可。

---

## 4. 留存（可选，后续加）
- MVP 不强制清理。后续若要控制体量，可以按天清理：
```sql
DELETE FROM news.articles
WHERE published_at < (NOW() AT TIME ZONE 'UTC') - INTERVAL '30 days';
```

---

## 5. 迁移说明
- 将本文件保存为 `V1__init_mvp.sql`，用迁移工具（如 `sqlx migrate`）或直接 `psql` 执行。  
- 以后若要加 **去重/模糊/全文/分区**，另行新增迁移文件，不修改本版。

---

## 6. 后续演进点（非 MVP）
- 去重（`url_hash UNIQUE` + 近似去重表达式索引）。
- 检索增强（`pg_trgm` + ILIKE / `tsvector` + GIN）。
- 留存与分区（按月分区或 TimescaleDB）。
- 抓取调度策略（根据 `last_fetch_status/fail_count` 退避/熔断）。
