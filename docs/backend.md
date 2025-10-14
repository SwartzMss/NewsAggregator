# 后端指南

本文介绍如何构建、配置与排查 News Aggregator 的 Rust 后端服务。

## 技术栈概览
- 语言：Rust（稳定版工具链）
- Web 框架：Axum（基于 Tokio）
- 数据访问：SQLx（PostgreSQL）
- 抓取器：Reqwest + rss/atom 解析在后台任务中运行
- 观测：tracing + 滚动日志文件

目录结构示例：
```
backend/
├── src/
│   ├── api/        # HTTP 路由处理
│   ├── fetcher/    # RSS 定时抓取
│   ├── repo/       # SQLx 数据访问
│   ├── service/    # 业务逻辑
│   └── config.rs   # AppConfig + 环境变量覆盖
└── config/config.yaml  # 默认配置
```

## 前置要求
- 安装 Rust 工具链（建议 `rustup default stable`）
- PostgreSQL，执行 `docs/database.md` 提供的 DDL
- `cargo sqlx prepare` 非必需，运行时直接使用 `DATABASE_URL`

## 配置方式
运行时配置来源有三层：

1. YAML 配置文件（默认读取 `config/config.yaml`）
2. 环境变量（覆盖关键字段）
3. 代码默认值（`AppConfig::default()`）

生产环境常用的环境变量：
```
CONFIG_FILE=<配置文件路径>          # 默认存在则读取 config/config.yaml
SERVER_BIND=127.0.0.1:8081          # 监听地址
DATABASE_URL=postgres://user:pass@host:port/db
LOG_FILE_PATH=/var/log/news-backend.log
FETCH_INTERVAL_SECS=300
FETCH_BATCH_SIZE=8
FETCH_CONCURRENCY=4
FETCH_TIMEOUT_SECS=15
LOG_LEVEL=info
```

缺少必需项（尤其是 `DATABASE_URL`）时服务会直接退出。

## 本地开发
```bash
# 在仓库根目录
cd backend
cp ../config/config.yaml config/local.yaml   # 如需自定义配置
export CONFIG_FILE=config/local.yaml
export DATABASE_URL=postgres://superset:superset@127.0.0.1:55432/superset
cargo run
```

常用命令：
- `cargo check`：快速语法/类型检查
- `cargo test`：运行测试
- `RUST_LOG=debug cargo run`：输出更详细的日志，同时保留文件日志

## Feed 删除策略
- 删除订阅源时后端会先获取数据库级锁，等待当前抓取任务完成后再继续。
- 删除流程会禁用该 Feed，并级联清理 `news.article_sources` 与 `news.articles` 中的相关记录。
- 若请求到达时抓取正在进行，API 会阻塞到锁释放，确保不会出现竞态或残留数据。
- 服务启动时会额外清理孤立内容（Feed 已删除但文章或来源残留），保证历史数据不会继续出现在列表中。

## Release 构建
```bash
cd backend
cargo build --release
# 可执行文件位于 backend/target/release/backend
```
部署脚本 `nginx/deploy.sh` 会自动执行该构建步骤。

## 抓取器说明
- 抓取周期、并发度、超时时间等可通过环境变量控制。
- 使用 `news.feeds` 中的 `last_etag`、`last_modified` 进行条件请求。
- 抓取失败会增加 `fail_count`，成功后重置，便于实现退避策略。

## systemd 集成
执行 `nginx/deploy.sh deploy` 会生成 `/etc/systemd/system/news-backend.service`。常用运维命令：
```bash
sudo systemctl status news-backend.service
sudo systemctl restart news-backend.service
sudo journalctl -u news-backend.service -n 200
```

## 日志
- 本地开发默认写入 `logs/backend.log`
- 生产环境可通过 `LOG_FILE_PATH` 指定日志文件位置
- stdout 仍会输出部分人类友好的 tracing 信息，方便实时查看

## 常见排错
- 服务无法启动：检查 `DATABASE_URL`、`CONFIG_FILE` 路径以及文件权限
- 抓取器不写数据：确认 `news.feeds.enabled`、`last_fetch_status`，以及系统时间是否正确
- 部署权限问题：确保 systemd 运行用户对仓库与日志目录有访问权限
