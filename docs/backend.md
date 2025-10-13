# Backend Guide

This document explains how to build, configure, and troubleshoot the Rust backend that powers the News Aggregator service.

## Stack Overview
- Language: Rust (stable toolchain)
- Framework: Axum on Tokio
- Data access: SQLx (PostgreSQL)
- Fetcher: Reqwest + rss/atom parsers running inside the backend process
- Observability: tracing + rolling file appender

Source layout highlights:
```
backend/
├── src/
│   ├── api/        # HTTP handlers
│   ├── fetcher/    # Background RSS polling
│   ├── repo/       # SQLx repositories
│   ├── service/    # Business logic
│   └── config.rs   # AppConfig + env overrides
└── config/config.yaml  # Default configuration
```

## Prerequisites
- Rust toolchain (`rustup default stable`)
- PostgreSQL with the schema from `docs/database.md`
- `sqlx` offline data (`cargo sqlx prepare`) is optional; runtime uses `DATABASE_URL`

## Configuration
Runtime configuration comes from three layers:

1. YAML file (default `config/config.yaml`)
2. Environment variables (override selected values)
3. Compile-time defaults in `AppConfig::default()`

Key environment variables used in production:
```
CONFIG_FILE=<path-to-config.yaml>   # Default: config/config.yaml if present
SERVER_BIND=127.0.0.1:8081          # Listen address
DATABASE_URL=postgres://user:pass@host:port/db
LOG_FILE_PATH=/var/log/news-backend.log
FETCH_INTERVAL_SECS=300
FETCH_BATCH_SIZE=8
FETCH_CONCURRENCY=4
FETCH_TIMEOUT_SECS=15
LOG_LEVEL=info
```

If any required value is missing (notably `DATABASE_URL`), the process aborts on startup.

## Local Development
```bash
# from repo root
cd backend
cp ../config/config.yaml config/local.yaml   # optional custom config
export CONFIG_FILE=config/local.yaml
export DATABASE_URL=postgres://superset:superset@127.0.0.1:55432/superset
cargo run
```

Useful commands:
- `cargo check` – fast validation
- `cargo test` – run unit/integration tests
- `RUST_LOG=debug cargo run` – verbose logging in stdout while keeping file logs

## Building Release Artifacts
```bash
cd backend
cargo build --release
# Binary: backend/target/release/backend
```
The deployment script (`nginx/deploy.sh`) compiles the release target automatically.

## Fetcher Notes
- Polling interval, concurrency, and timeouts are configurable via env vars.
- Conditional requests use the `last_etag` and `last_modified` columns in the `news.feeds` table.
- Failures increment `fail_count`; recovery resets it.

## Systemd Integration
`nginx/deploy.sh deploy` generates `/etc/systemd/system/news-backend.service` with the proper environment. Manual management:
```bash
sudo systemctl status news-backend.service
sudo systemctl restart news-backend.service
sudo journalctl -u news-backend.service -n 200
```

## Logging
- Default log file: `logs/backend.log` in the repo when running locally.
- Production log file path configurable via `LOG_FILE_PATH`.
- stdout keeps human-friendly tracing for entries with `target` starting with `backend`.

## Troubleshooting
- Service fails to start: verify `DATABASE_URL`, `CONFIG_FILE`, and file permissions.
- Fetcher not writing data: check `news.feeds.enabled`, `last_fetch_status`, and system time.
- Permission errors in deployment: ensure the systemd user has access to the repo and log directory.
