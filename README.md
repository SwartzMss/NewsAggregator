# RSS 新闻聚合（MVP）— 简要说明

一个**极简**的新闻聚合服务：从公开 **RSS/Atom** 拉取新闻，存入 **PostgreSQL**，提供 **HTTP API** 给前端平铺展示与基础管理。

## 核心功能
- **RSS 订阅管理**：新增/编辑/启用/禁用 RSS 源（支持抓取状态查看）。
- **定时抓取**：周期拉取、解析 RSS，记录至数据库（允许重复，不做去重/全文）。
- **文章列表**：按时间倒序分页读取；（可选）按时间范围过滤。
- **前端展示**：简洁列表页 + Feed 管理页（新增、编辑、启用/禁用）。

## 技术栈
- 后端：Rust（Axum / Tokio / Reqwest / SQLx）+ PostgreSQL  
- 前端：React + Vite + TypeScript + Tailwind  
- 数据：两张表 `news.feeds`、`news.articles`（无 sources 表）

## API（简要）
- `GET /healthz`：存活检查  
- `GET /articles`：文章列表（参数：`from`、`to`、`page`、`page_size`）  
- `GET /feeds`：Feed 列表  
- `POST /feeds`：新增/更新 Feed（传 `url`、`source_domain` 等）  
- `PATCH /feeds/:id`：编辑/启用禁用  
- `DELETE /feeds/:id`：（可选）

## 快速开始
1) **准备数据库**  
   执行 `DB_SCHEMA_FRESH_MVP.md` 中的一次性 DDL（创建 `news` schema 与两张表）。
2) **启动后端**  
   配置环境：`DATABASE_URL`、定时抓取间隔等；运行：`cargo run`
3) **启动前端**  
   `.env`：`VITE_API_BASE_URL=http://127.0.0.1:8080`；运行：`npm i && npm run dev`
4) **使用**  
   打开前端：新增一个 RSS 源 → 等待抓取 → 在首页看到新闻列表。

## 取舍说明（MVP）
- 不做去重、不做模糊/全文检索（后续可加）。  
- 仅返回标题/摘要/原文链接，**不缓存全文**。  
- 条件请求（ETag/Last-Modified）已预留字段，节省带宽。

## 路线图（可选）
- 去重与相似合并  
- 关键词搜索 / 全文检索  
- 留存清理 / 分区  
- 推送（邮件/Telegram）与抓取日志面板

—— 就这么简单：能**订阅 RSS**、能**存到 PG**、能**查出来展示** ✅
