# Google News 集成设计草案

## 目标
- 以「订阅刷新」方式接入 Google News RSS 源，聚合指定站点（首阶段为 `site:reuters.com`）的内容。
- 为后续的「手动搜索触发」模式预留扩展点，保证服务层逻辑可复用。
- 将英文资讯统一翻译成中文，提供中/英双语内容，并保留原始链接。

## 范围
- Server 侧抓取、解析、翻译、入库和去重流程。
- 配置和安全注意事项（例如 UA、自适应参数、限频）。
- 前端暂时只消费订阅式输出，不包含 UI 改动细节。

## 搜索入口设计
- 「站内搜索」独立提供本地数据库内容查询，聚焦已有文章，支持来源过滤（如 `source=reuters`）和多语字段匹配。
- 「Google 搜索」作为另一入口，用户主动触发时复用下方的 Feed Fetcher + Parser 流程即时拉取 RSS；结果可落地至临时表或缓存，供用户浏览，也可选同步到主库。
- 前端搜索页展示为并列的两个分组（或 Tabs），用户在站内和 Google 之间手动切换，无优先级或回退逻辑。
- 搜索 API 层新增 `mode=local|google` 参数，两种模式分别调用本地检索或远程抓取流程；返回结构保持一致，便于前端共用渲染。

## 关键组件
1. **Feed Fetcher**
   - 负责构造搜索 URL，例如：`https://news.google.com/rss/search?q=site:reuters.com&hl=zh-CN&gl=US&ceid=US:zh-Hans`。
   - 支持动态参数（站点、关键词、地区、语言），以便切换不同订阅源或未来用户搜索。
   - 通过 HTTP 客户端发送请求，附带自定义 `User-Agent`（伪装为常见浏览器 UA，降低被阻挡概率）。
   - 需要处理网络错误、超时、403 等异常，并做指数退避重试。

2. **Feed Parser**
   - 使用现有库（Node: `rss-parser` / Python: `feedparser`）解析 RSS XML。
   - 将条目标准化为内部结构：`id`、`title_en`、`summary_en`、`link`、`published_at`、`source` 等。
   - 校验 `pubDate` 是否新鲜；针对 Google 可能返回旧数据的情况，额外记录抓取时间。

3. **Translation Service**
   - 提供统一接口 `translate(text, from_lang='en', to_lang='zh-CN')`。
   - 首阶段可集成云服务（如 Google Cloud Translation 或 Azure Translator）；后期可替换为自部署模型。
   - 支持批量翻译，减少并发请求次数；翻译失败时回退保留英文原文。
   - 输出字段：`title_zh`、`summary_zh`（后续若抓正文，可扩展 `content_en` → `content_zh`）。
   - 字段映射校验示例（基于 RSS `<item>`）：
     - `<title>` → `title_en`
     - `<description>` 多数场景只含超链接/HTML，可尝试提取锚文本；若为空则保留 `summary_en = null`
     - `<link>` → `link`
     - `<guid>` → `guid`（优先用于去重）
     - `<pubDate>` → `published_at`
     - `<source>` → `source`
     - 翻译结果写入 `title_zh`、`summary_zh`

4. **Persistence & Deduplication**
   - 将条目写入数据库（参考 `database.md`，选择现有 `articles` 或新建表）。
   - 去重策略：优先使用 RSS `guid`；若缺失则根据 `link`、`title_en`+`published_at` 组合判断。
   - 保存抓取时间、翻译状态、失败原因，便于监控。

5. **Scheduler / Workflow**
   - 订阅模式：使用定时任务（cron/job runner）每 15~30 分钟拉取一次；根据实际新鲜度调整。
   - 后期扩展用户搜索时，复用同一套 `Feed Fetcher + Parser + Translation` 流程，只是触发方式改为 API 请求。

6. **API Layer**
   - 提供给前端的查询接口，例如 `GET /api/articles?source=reuters&limit=20`。
   - 按照发布时间倒序返回，支持分页。
   - 返回中英字段，并可附带原文链接和 `source_logo` 等展示信息。

## 配置与运维
- **User-Agent**：配置在服务器端（可放 `config/google_news.yml`），默认使用现代浏览器 UA，例如 `Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/... Chrome/... Safari/...`，并支持快速切换。
- **代理/网络**：Google News 在部分地区不可直连，需要通过代理节点。配置项中记录代理地址，代码读取后走代理客户端。
- **限频**：默认抓取频率限制在每路订阅每 15 分钟一次；用户触发时采用节流/缓存（例如 60 秒内重复关键词返回缓存）。
- **日志与监控**：记录每次抓取状态、翻译耗时、错误码，便于追踪。

## 安全与合规
- 遵守 Reuters 与 Google News 的使用条款：仅做内部聚合，不对外大规模再分发；访问频率要控制。
- 存储原文链接，尊重版权；若要展示正文，需评估是否允许全文抓取。

## 后续扩展
- **手动搜索模式**：引入 `POST /api/search`，接受关键词并动态构造 RSS URL；结果写入同一数据结构并做临时缓存。
- **多源支持**：增加配置文件，允许按语言/地区定义多个订阅任务。
- **自然语言摘要**：在翻译后增加摘要或标签生成，提升内容可读性。
- **质量评估**：记录翻译质量或人工反馈，决定是否切换其他翻译服务。
