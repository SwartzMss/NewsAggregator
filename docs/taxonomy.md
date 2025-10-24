# 内容分类/频道/话题设计方案

目标：在当前仅按来源、语言浏览的基础上，引入可扩展的分类维度，支持今后新增“频道（Channel）”“话题（Topic）”“标签（Tag）”，并预留“市场/板块（Market）”的分区能力。

## 设计原则
- 最小侵入：不影响现有抓取与展示；无分类时行为不变。
- 可扩展：后续能方便新增新的分类类型或层级。
- 低耦合：文章与分类多对多，Feed 可选默认分类，用于新文章的自动归类。
- 易查询：常用过滤有索引；避免繁重 JOIN。

## 方案 A：通用“术语表 + 关系表”（推荐）
使用统一的 `taxonomy_terms` 存储不同类型的分类项，通过 `article_taxonomies`、`feed_taxonomies` 建立关联。

```sql
-- 分类项：频道/话题/标签/市场等
CREATE TABLE IF NOT EXISTS news.taxonomy_terms (
  id            BIGSERIAL PRIMARY KEY,
  kind          TEXT NOT NULL,      -- channel | topic | tag | market
  slug          TEXT NOT NULL,      -- 唯一标识（URL 友好）
  name          TEXT NOT NULL,      -- 展示名
  parent_id     BIGINT REFERENCES news.taxonomy_terms(id) ON DELETE SET NULL,
  sort_order    INTEGER NOT NULL DEFAULT 0,
  enabled       BOOLEAN NOT NULL DEFAULT TRUE,
  created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  UNIQUE(kind, slug)
);
CREATE INDEX IF NOT EXISTS idx_taxonomy_terms_kind ON news.taxonomy_terms(kind);
CREATE INDEX IF NOT EXISTS idx_taxonomy_terms_parent ON news.taxonomy_terms(parent_id);

-- 文章与分类的多对多
CREATE TABLE IF NOT EXISTS news.article_taxonomies (
  article_id    BIGINT NOT NULL REFERENCES news.articles(id) ON DELETE CASCADE,
  term_id       BIGINT NOT NULL REFERENCES news.taxonomy_terms(id) ON DELETE CASCADE,
  PRIMARY KEY(article_id, term_id)
);
CREATE INDEX IF NOT EXISTS idx_article_taxonomies_term ON news.article_taxonomies(term_id);

-- Feed 与分类（用于新文章默认归类）
CREATE TABLE IF NOT EXISTS news.feed_taxonomies (
  feed_id       BIGINT NOT NULL REFERENCES news.feeds(id) ON DELETE CASCADE,
  term_id       BIGINT NOT NULL REFERENCES news.taxonomy_terms(id) ON DELETE CASCADE,
  PRIMARY KEY(feed_id, term_id)
);
CREATE INDEX IF NOT EXISTS idx_feed_taxonomies_term ON news.feed_taxonomies(term_id);
```

要点：
- `kind` 支持四类：`market`、`channel`、`topic`、`tag`。可按需精简或扩展。
- `parent_id` 支持层级结构（如频道下挂子频道；话题归属于频道）。
- Feed 绑定的分类，在抓取入库时将对应 term 自动打到新文章上（可多选）。

## 方案 B：分表（更直观，但表较多）
为频道、话题、标签分别建表，并各自维护文章与 Feed 的关系表。结构更清晰，但迁移与查询需写多套 SQL。若你已确定只需要“频道”和“话题”，可选 B。

## API 变更建议
- 列表文章：`GET /api/articles?channel=slug&topic=slug&tag=slug&market=slug`
  - 后端将 slug 映射到 term_id 后与 `article_taxonomies` 关联过滤。
  - 建议一次仅按一种 `kind` 过滤，或提供并集/交集语义参数（`match=any|all`）。
- 管理端分类：
  - `GET /admin/api/taxonomies?kind=channel` 列出分类项
  - `POST /admin/api/taxonomies` 新增/编辑/禁用项
  - `POST /admin/api/feeds/:id/taxonomies` 维护 Feed 默认分类
- 入库流程（后端）：
  - 读取 `feed_taxonomies`，为新文章批量插入 `article_taxonomies`。

## 前端改动建议
- 顶部导航或侧边栏添加“频道”入口；频道页支持：
  - 频道列表（`/channel`）+ 频道详情（`/channel/:slug`）展示文章流。
  - 话题为次级过滤或在详情页展示可切换的 Tab。
- 管理端新增“分类管理”页面：增删改、排序、启用开关，支持拖拽排序（可后续迭代）。
- Feed 编辑弹窗中增加“默认频道/话题/标签”多选。

## 典型查询（方案 A）
按频道筛选文章：
```sql
SELECT a.id, a.title, a.url, a.description, a.language, a.source_domain,
       a.published_at, COALESCE(c.click_count, 0) AS click_count
FROM news.articles a
JOIN news.article_taxonomies at ON at.article_id = a.id
JOIN news.taxonomy_terms t ON t.id = at.term_id AND t.kind = 'channel' AND t.slug = $1
LEFT JOIN LATERAL (
  SELECT 0::bigint AS click_count -- 若有点击统计表，可在此 JOIN
) c ON TRUE
WHERE a.published_at BETWEEN $2 AND $3
ORDER BY a.published_at DESC
LIMIT $4 OFFSET $5;
```

## 迁移步骤
1) 执行上述建表 SQL（可追加到 `docs/database.md` 的初始化 SQL 或写独立迁移脚本）。
2) 管理端提供分类项的增删改接口与页面。
3) Feed 侧新增维护默认分类（保存到 `feed_taxonomies`）。
4) 抓取入库时，将 Feed 对应分类同步写入 `article_taxonomies`。
5) 列表接口支持按 slug 过滤。

## 命名与示例
- Market 示例：`cn`（中国市场）、`us-tech`（美国科技板块）。
- Channel 示例：`ai`、`frontend`、`macro`。
- Topic 示例：`openai-devday`、`swift-evolution`（短期热点或专题）。
- Tag 示例：更自由的用户标注，如 `deep-dive`、`beginner`。

## 取舍与后续
- 若短期仅需“频道”，可只用 `kind='channel'` 与两张关系表，后续再扩展其他 kind。
- 若需要权限隔离或多租户，“market”可用于路由前缀与数据隔离（在查询中加上 market term 过滤）。
- 如需统计热度或排序，可增表记录点击/收藏并与分类维度结合做排行榜。

如你确定先上“频道”而暂不开放“话题/标签/市场”，我可以直接按方案 A 只落地 `channel` 所需的最小表与 API 草稿代码。
