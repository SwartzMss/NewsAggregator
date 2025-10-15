# 新闻搜索功能设计方案

## 背景与目标
- 让用户可以快速定位历史文章，而不是依赖手动翻页。
- 支持按关键字模糊匹配标题，保持现有分页体验。
- 本设计聚焦 MVP，暂不涵盖高级筛选、排序等能力。

## 功能范围
- 搜索输入为自由文本关键字；暂不区分多词分词或高级语法。
- 搜索结果沿用现有 `/articles` 接口分页格式。
- 其他筛选条件暂不开放，默认仅按标题搜索。

## 后端设计
### REST API
- 在现有 `GET /articles` 接口上新增 `keyword` 查询参数。
- 典型请求：`GET /articles?page=1&page_size=20&keyword=OpenAI`.
- 响应仍为 `PageResp<ArticleOut>`，便于前端复用。

### 服务层与仓储层
- `ArticleListQuery` 模型新增 `keyword: Option<String>`.
- Service 层负责去除首尾空白、过滤空字符串。
- Repository 层扩展 SQL 查询：
  - `WHERE` 子句追加 `keyword IS NULL OR title ILIKE $keyword`，只针对标题模糊匹配，聚焦于查找文章而非来源过滤。
  - `keyword` 采用 `%keyword%` 模式的 `ILIKE`，兼容大小写。
- 统计总数的 `COUNT(*)` 查询同步增加筛选条件，确保分页正确。

### 数据库考虑
- 依赖 PostgreSQL，`ILIKE` 提供模糊匹配但在大表会慢。
- 初始阶段可接受；数据增长后可考虑：
  1. 为 `title` 建立 trigram (`pg_trgm`) GIN 索引以提速模糊匹配。
  2. 或引入 `tsvector` 与全文搜索。
- 若未来需要多语言搜索，可扩展 `Language` 字段与 `to_tsvector`。

## 前端设计
### UI/UX
- 顶部导航新增“搜索”入口，点击后进入专属搜索页面。
- 搜索页提供单一输入框，支持 Enter 触发，仅展示标题匹配结果列表。

### 状态与数据流
- React Query 查询键扩展为 `["articles", { keyword }]` 保证缓存隔离。
- 搜索提交后重置页码为 1，调用 `query.refetch()`。
- 保留无限滚动逻辑：`getNextPageParam` 基于返回的 `page` + `page_size` 判定是否有下一页。
- 在 URL query string 中同步 keyword（可选），支持刷新/分享。

### 错误与空态
- 搜索无结果：展示 “未找到与关键字匹配的文章”。
- 保留现有加载、错误处理逻辑。

## 兼容性与迁移
- 后端新增字段默认为 `None`，兼容旧客户端。
- 前端在后端上线后再发布，避免 `keyword` 参数未被识别。
- 文档更新：README 新增搜索使用说明，API 文档列出参数。
