# RSS 新闻聚合（MVP）

极简新闻聚合服务：抓取公开 RSS/Atom，写入 PostgreSQL，通过 HTTP API 提供给前端展示与管理。

## 快速认识
- **核心能力**：RSS 订阅管理、定时抓取、文章列表分页、前端平铺展示
- **技术栈**：Rust (Axum + SQLx) + PostgreSQL / React + Vite + Tailwind
- **部署方式**：nginx 反向代理 + systemd 后端守护（见 `nginx/deploy.sh`）

## 快速开始
1. **数据库**：执行 `docs/database.md` 中的 DDL，准备 `DATABASE_URL`。
2. **后端**：参考 `docs/backend.md` 配置并运行 `cargo run` 或 `cargo build --release`。
3. **前端**：按照 `docs/frontend.md` 设置 `VITE_API_BASE_URL`，执行 `npm run dev` 或 `npm run build`。
4. **一键部署**：更新 `nginx/config.sh` 后运行 `sudo bash nginx/deploy.sh deploy`。

## 文档索引
- `docs/backend.md` 后端配置、运行与排错指南
- `docs/frontend.md` 前端开发与构建说明
- `docs/database.md` 数据库结构与常用 SQL
- `nginx/nginx_deploy.md` 生产部署步骤与维护命令

## API 速览
- `GET /healthz` – 存活检查
- `GET /articles` – 文章列表（分页 + 时间过滤）
- `GET /feeds` – Feed 列表
- `POST /feeds`, `PATCH /feeds/:id`, `DELETE /feeds/:id` – Feed 管理

## 设计取舍
- 不做去重与全文索引，优先保证抓取可靠性
- 文章仅保存摘要与链接，全文交由来源站点
- 抓取器已支持 `ETag` 与 `Last-Modified`，为后续优化留钩子

欢迎继续扩展：聚合去重、全文检索、推送通知、抓取监控面板等。
