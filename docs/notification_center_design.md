# 通知中心设计（Draft）

本设计旨在提供“重要运行事件”的可视化与实时通知，不作为全量日志查看器。事件由后端在关键路径显式写入，日志作为详情参考，不依赖解析日志生成通知。

## 目标
- 准实时呈现关键事件，避免被高频日志淹没。
- 可筛选、可标记已读/已解决、可静音与聚合计数。
- 与日志系统协同：事件含结构化字段，便于跳转或检索对应日志。

## 非目标
- 不记录“手动检测”类事件（如模型连通性测试）。
- 不表示“实时可用性/健康状态”，除非由事件显式表达。

---

## 事件模型（Phase 1 精简版）
- 字段（建议表 `ops.events`）：
  - `id bigserial`
  - `ts timestamptz` 事件时间
  - `level text`：`info` | `warn` | `error`
  - `code text`：稳定的机器可读事件码（如 `FETCHER_STOPPED`）
  - `title text`：用户可读标题
  - `message text`：简要描述（不放大文本堆栈）
  - `attrs jsonb`：结构化属性（如 `{feed_id, url, provider, http_status, trace_id}`）
  - `source text`：组件/模块名（如 `fetcher`, `api`, `models`）
  - `dedupe_key text`：去重键（如 `feed:{id}`、`provider:{name}`）
  - `count int`：聚合计数（窗口内递增）
  - `ttl_at timestamptz`：到期自动清理（可选）

- 去重策略：同 `(code, dedupe_key)` 在 N 分钟（默认 5 分钟）窗口内聚合，`count++`；超窗新建事件。

- 本阶段不包含：已读/已解决/静音 状态与相关接口（Phase 2 再评估）。

## 事件分类与样例
- System
  - `FETCHER_STOPPED` (error)：抓取循环退出。attrs=`{error}`；dedupe_key=`system:fetcher`
  - `STARTUP_CONFIG_NORMALIZE_FAILED` (warn)：启动时配置归一化失败。attrs=`{error}`；dedupe_key=`system:startup`
- Feeds
  - `FEED_PROCESS_FAILED` (warn)：单 feed 处理失败。attrs=`{feed_id, url, error}`；dedupe_key=`feed:{id}`
  - `FEED_MARKED_FAILURE` (warn)：拉取失败标记（含状态码）。attrs=`{feed_id, status}`；dedupe_key=`feed:{id}`
  - `FEED_IMMEDIATE_FETCH_FAILED` (warn)：新建后立即拉取失败。attrs=`{feed_id, error}`；dedupe_key=`feed:{id}`
  - `FEED_LOCK_RELEASE_FAILED` (error)：异常后锁释放失败。attrs=`{feed_id, error}`；dedupe_key=`feed:{id}`
- Models
  - `TRANSLATOR_PROVIDER_UNAVAILABLE` (warn)：设置为未配置齐全的 provider。attrs=`{provider}`；dedupe_key=`provider:{name}`
  - `TRANSLATION_FAILED` (warn)：翻译重试后仍失败。attrs=`{feed_id, url, provider, error}`；dedupe_key=`provider:{name}:feed:{id}`
- Content
  - `URL_NORMALIZE_FAILED` (warn)：URL 归一化失败。attrs=`{raw_url, error}`；dedupe_key=`domain:{host}`
  - `ARTICLE_INSERT_SKIPPED` (warn)：入库失败被跳过。attrs=`{url, error}`；dedupe_key=`domain:{host}`
  - `ARTICLE_SOURCE_RECORD_FAILED` (warn)：来源追踪写入失败。attrs=`{feed_id, article_id, error}`；dedupe_key=`feed:{id}`
- API
  - `INTERNAL_SERVER_ERROR` (error)：统一 500。attrs=`{route?, trace_id, error}`；dedupe_key=`route:{path}`

> 备注：不记录 `TRANSLATOR_VERIFY_FAILED`（手动测试）。

## 事件写入封装
- 提供异步、尽力而为（不阻塞主路径）的接口：

```rust
// 伪代码
pub async fn emit(
  level: Level, code: &str, title: &str, message: &str,
  source: &str, dedupe_key: &str, attrs: serde_json::Value,
) -> anyhow::Result<()>
```

- 实现要点：
  - 通过 `tokio::mpsc` 将事件送入后台批处理写入；失败时降级为一条 `tracing` 日志。
  - 聚合：先查窗口内同键记录，命中则 `UPDATE count+=1, ts=now()`，否则 `INSERT`。

## 后端接口（Phase 1 精简版）
- 拉取列表
  - `GET /admin/api/alerts?level=&code=&source=&from=&to=&since_id=&limit=`
- SSE 推送
  - `GET /admin/api/alerts/stream`
  - 事件名：`alert`；数据：事件 JSON
  - 心跳：`event: ping` 每 15–30s
  - 权限：复用 Admin 鉴权中间件

### SSE 交互与鉴权细节
- 握手与流格式：标准 `text/event-stream`；每条事件：`event: alert` + `data: <json>\n\n`
- 鉴权：
  - 默认使用 `Authorization: Bearer <token>`（浏览器 EventSource 不支持自定义 Header）。
  - 兼容方式：支持查询参数 `?token=<admin_token>`，仅用于受信的管理端界面。
- 保活：服务端写入 keep-alive（20–30s），避免中间层断开空闲连接。
- 重连与兜底：
  - EventSource 默认断线自动重连；如需要“无遗漏”，前端可在重连后调用 `GET /admin/api/alerts?since_id=<last_id>` 补齐增量。
  - 若网络或代理阻断 SSE，可回退为 10–30s 轮询（带 since_id）。
- 事件 JSON 结构：
  - `{ id, ts, level, code, title, message, attrs, source, dedupe_key, count }`
  - `attrs` 根据事件不同包含：`feed_id`、`url`、`provider`、`trace_id` 等；`count` 为 5 分钟窗口聚合计数。

### 前端消费约定
- 首屏：调用 `GET /admin/api/alerts?limit=50` 获取最近事件。
- 实时：创建 `EventSource('/admin/api/alerts/stream?token=...')`，监听 `alert` 事件，将 JSON 解析后插入列表顶部；列表建议保留 100–200 条。
- 过滤：本地对 `level/code/source` 过滤；服务端查询接口支持同名参数用于分页或初始筛选。
- 断线：关闭 EventSource 并提示，但保留已加载数据；可选实现自动重连与 `since_id` 补齐。

### SSE 与轮询
- 首次进入页面：`GET /alerts?limit=50` 载入最近列表
- 同时创建 `EventSource('/admin/api/alerts/stream')`，实时插入新事件
- 断线回退：以 `since_id` 每 15s 轮询增量

## 前端（管理端）交互（Phase 1 精简版）
- 列表：时间倒序，按 `level` 上色，展示 `title`、关键信息（`feed_id/url/provider`）、`count`。
- 过滤：`level/code/source/feed_id/provider/时间窗`。
- 详情抽屉：显示 `message` 与 `attrs`。
- 暂不提供：已读/已解决/静音；后续 Phase 2 再加。

## 字段与日志协同
- 统一结构化字段：`feed_id`、`url`、`provider`、`http_status`、`trace_id`、`error_chain`。
- 500 错误建议注入 `trace_id`，便于事件→日志快速定位。

## 数据保留与清理
- 低价值事件设置 `ttl_at`，后台定期清理。
- 历史归档：可按月将老事件转存对象存储（可选）。

## 最小落地范围（Phase 1）
- 建表 `ops.events` 与写入封装（含聚合）。
- 在 6–8 个关键位点埋事件：
  - `FETCHER_STOPPED`、`FEED_PROCESS_FAILED`、`FEED_MARKED_FAILURE`、
    `FEED_IMMEDIATE_FETCH_FAILED`、`FEED_LOCK_RELEASE_FAILED`、
    `TRANSLATOR_PROVIDER_UNAVAILABLE`、`URL_NORMALIZE_FAILED`、`INTERNAL_SERVER_ERROR`。
- 提供列表 `GET` 与 `SSE` 接口；前端只读列表页 + 基础筛选。

## 后续增强（Phase 2+）
- 静音与 TTL 管理界面；批量操作。
- 阈值升级（如单 feed 连续失败转 `FEED_HEALTH_DEGRADED`）。
- Dashboard：异常订阅源 TopN、翻译失败率趋势。

---

## 与现有代码的映射
- 事件不替代日志：保留 `tracing` 的 INFO/WARN/ERROR；通知中心仅消费显式事件。
- 手动测试（模型连通性）不落事件，仅返回接口结果。
- “可用/不可用”在设置阶段仅代表“是否已配置完整”，非在线状态。
 - API 中间件：
   - `assign_trace_id` 为每个请求注入 `X-Trace-Id`，用于 500 事件的日志关联。
   - `report_internal_errors` 拦截状态码 >= 500 的响应并发出 `INTERNAL_SERVER_ERROR` 事件（attrs 含 `method/path/trace_id`）。
