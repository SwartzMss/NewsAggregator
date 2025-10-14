# News Aggregator

一体化的 RSS 新闻聚合服务，自动抓取多源内容、去重入库，并通过 Web 界面 + HTTP API 对外提供阅读与管理能力。

## 当前能力
- **Feed 管理**：后端提供增删改查接口，维护订阅源状态、抓取频率与启用开关，前端支持可视化管理。
- **定时抓取**：后台定时任务使用 Reqwest 抓取 RSS/Atom，结合 ETag 与 Last-Modified 控制带宽，并记录抓取日志。
- **智能去重**：从 URL/发布时间规则到标题相似度，再到 DeepSeek 语义判定的多层策略，阻止重复新闻进入主表，同时保留来源追溯。
- **数据存储**：PostgreSQL `news` schema 维护 feeds、articles、article_sources 三张核心表，支持分页与多条件查询。
- **前端展示**：React + Vite 构建的 SPA，通过 TanStack Query 调用 API，提供文章列表与订阅源面板。
- **运维部署**：nginx 反向代理静态资源与 `/api`，systemd 接管 Rust 服务，部署脚本 `nginx/deploy.sh` 支持一键上线。

## 代码结构
- `backend/`：Axum 服务、抓取器、SQLx 数据访问层与配置管理。
- `frontend/`：Vite + React 前端源码，包含组件、页面与 API 封装。
- `docs/`：后端、前端、数据库与去重方案的详细说明。
- `nginx/`：部署脚本与示例配置。

## 更多信息
- 后端运行方式、配置项与排错技巧：`docs/backend.md`
- 前端开发与构建说明：`docs/frontend.md`
- 数据库 schema 与常用 SQL：`docs/database.md`
- 新闻去重策略：`docs/news-dedup-plan.md`
- WSL 端口转发提示：Windows 的 `netsh interface portproxy` 在默认 WSL 配置下需要使用 `v4tov6`，例如 `netsh interface portproxy add v4tov6 listenaddress=0.0.0.0 listenport=443 connectaddress=::1 connectport=443`，否则 127.0.0.1 的访问不会映射到 WSL 内的服务。

如需本地开发或生产部署，请参照对应文档逐步执行。
