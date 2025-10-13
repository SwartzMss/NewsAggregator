#!/usr/bin/env bash

# Deployment configuration for the News Aggregator stack.
# Adjust the values below to match your environment before running deploy.sh.

# Linux account that owns the project sources. All builds run as this user.
APP_USER="swartz"

# Domain and TLS assets for the public site.
DOMAIN="swartzlubel.online"
SSL_CERT_PATH="/home/swartz/.acme.sh/swartzlubel.online_ecc/fullchain.cer"
SSL_KEY_PATH="/home/swartz/.acme.sh/swartzlubel.online_ecc/swartzlubel.online.key"

# Project directories.
REPO_ROOT="/home/swartz/WorkSpace/NewsAggregator"
BACKEND_DIR="${REPO_ROOT}/backend"
FRONTEND_DIR="${REPO_ROOT}/frontend"
CONFIG_FILE="${REPO_ROOT}/config/config.yaml"

# Backend build/output settings.
BACKEND_BINARY="${BACKEND_DIR}/target/release/backend"
BACKEND_BIND_ADDR="127.0.0.1:8081"

# Frontend static asset destination served by nginx.
STATIC_ROOT="/var/www/news-aggregator/dist"
STATIC_OWNER="www-data"
STATIC_GROUP="www-data"

# nginx configuration.
NGINX_SITE_NAME="news-aggregator"
NGINX_CONF_PATH="/etc/nginx/sites-available/${NGINX_SITE_NAME}.conf"
NGINX_ENABLED_PATH="/etc/nginx/sites-enabled/${NGINX_SITE_NAME}.conf"

# systemd service definition for the Rust backend.
SYSTEMD_SERVICE_NAME="news-backend.service"
SYSTEMD_UNIT_PATH="/etc/systemd/system/${SYSTEMD_SERVICE_NAME}"

# Backend runtime environment variables.
DATABASE_URL="postgres://superset:superset@127.0.0.1:55432/superset"
LOG_FILE_PATH="/var/log/news-backend.log"
FETCH_INTERVAL_SECS="300"
FETCH_BATCH_SIZE="8"
FETCH_CONCURRENCY="4"
FETCH_TIMEOUT_SECS="15"
LOG_LEVEL="info"
