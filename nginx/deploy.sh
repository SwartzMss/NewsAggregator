#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CONFIG_FILE_PATH="${SCRIPT_DIR}/config.sh"

if [[ ! -f "${CONFIG_FILE_PATH}" ]]; then
  echo "Configuration file ${CONFIG_FILE_PATH} not found." >&2
  exit 1
fi

# shellcheck source=./config.sh
source "${CONFIG_FILE_PATH}"

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "Command '$1' not found. Please install it before running deploy.sh." >&2
    exit 1
  fi
}

run_as_app() {
  local cmd="$1"
  runuser -u "${APP_USER}" -- bash -lc "${cmd}"
}

ensure_root() {
  if [[ "${EUID}" -ne 0 ]]; then
    echo "This script must be run as root (use sudo)." >&2
    exit 1
  fi
}

validate_config() {
  if ! id "${APP_USER}" >/dev/null 2>&1; then
    echo "User '${APP_USER}' does not exist on this system." >&2
    exit 1
  fi

  if [[ ! -d "${BACKEND_DIR}" ]]; then
    echo "Backend directory ${BACKEND_DIR} not found." >&2
    exit 1
  fi

  if [[ ! -d "${FRONTEND_DIR}" ]]; then
    echo "Frontend directory ${FRONTEND_DIR} not found." >&2
    exit 1
  fi

  if [[ ! -f "${CONFIG_FILE}" ]]; then
    echo "Config file ${CONFIG_FILE} not found." >&2
    exit 1
  fi

  if [[ ! -f "${SSL_CERT_PATH}" ]]; then
    echo "SSL certificate not found at ${SSL_CERT_PATH}." >&2
    exit 1
  fi

  if [[ ! -f "${SSL_KEY_PATH}" ]]; then
    echo "SSL key not found at ${SSL_KEY_PATH}." >&2
    exit 1
  fi
}

ensure_paths() {
  mkdir -p "${STATIC_ROOT}"
  mkdir -p "$(dirname "${LOG_FILE_PATH}")"
  touch "${LOG_FILE_PATH}"
  chown "${APP_USER}:${APP_USER}" "${LOG_FILE_PATH}"

  adjust_static_permissions
}

adjust_static_permissions() {
  if id "${STATIC_OWNER}" >/dev/null 2>&1 && getent group "${STATIC_GROUP}" >/dev/null 2>&1; then
    chown -R "${STATIC_OWNER}:${STATIC_GROUP}" "${STATIC_ROOT}"
  fi
}

build_backend() {
  echo "[1/6] Building backend (release)..."
  require_cmd cargo
  run_as_app "cd '${BACKEND_DIR}' && cargo build --release"
  if [[ ! -x "${BACKEND_BINARY}" ]]; then
    echo "Backend binary not found at ${BACKEND_BINARY} after build." >&2
    exit 1
  fi
}

build_frontend() {
  echo "[2/6] Building frontend..."
  require_cmd npm
  run_as_app "cd '${FRONTEND_DIR}' && npm install && npm run build"
}

sync_static_assets() {
  echo "[3/6] Syncing frontend assets to ${STATIC_ROOT}..."
  local dist_dir="${FRONTEND_DIR}/dist"
  if [[ ! -d "${dist_dir}" ]]; then
    echo "Frontend dist directory not found at ${dist_dir}. Build step may have failed." >&2
    exit 1
  fi

  if command -v rsync >/dev/null 2>&1; then
    rsync -a --delete "${dist_dir}/" "${STATIC_ROOT}/"
  else
    rm -rf "${STATIC_ROOT:?}/"*
    cp -a "${dist_dir}/." "${STATIC_ROOT}/"
  fi

  adjust_static_permissions
}

write_nginx_config() {
  echo "[4/6] Writing nginx config to ${NGINX_CONF_PATH}..."
  require_cmd nginx

  cat > "${NGINX_CONF_PATH}" <<EOF
server {
    listen 80;
    listen [::]:80;
    server_name ${DOMAIN};
    return 301 https://\$host\$request_uri;
}

server {
    listen 443 ssl http2;
    listen [::]:443 ssl http2;
    server_name ${DOMAIN};

    ssl_certificate     ${SSL_CERT_PATH};
    ssl_certificate_key ${SSL_KEY_PATH};
    ssl_protocols       TLSv1.2 TLSv1.3;
    ssl_prefer_server_ciphers on;

    root ${STATIC_ROOT};
    index index.html;
    try_files \$uri \$uri/ /index.html;

    location /api/ {
        proxy_pass http://${BACKEND_BIND_ADDR}/;
        proxy_set_header Host \$host;
        proxy_set_header X-Real-IP \$remote_addr;
        proxy_set_header X-Forwarded-For \$proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto \$scheme;
        proxy_http_version 1.1;
    }

    location /healthz {
        proxy_pass http://${BACKEND_BIND_ADDR}/healthz;
        proxy_set_header Host \$host;
    }

    location ~* \.(css|js|jpg|jpeg|png|gif|ico|svg)$ {
        expires 7d;
        access_log off;
    }
}
EOF

  ln -sf "${NGINX_CONF_PATH}" "${NGINX_ENABLED_PATH}"
  nginx -t
}

write_systemd_unit() {
  echo "[5/6] Installing systemd unit at ${SYSTEMD_UNIT_PATH}..."
  require_cmd systemctl

  cat > "${SYSTEMD_UNIT_PATH}" <<EOF
[Unit]
Description=News Aggregator Backend
After=network.target

[Service]
User=${APP_USER}
Group=${APP_USER}
WorkingDirectory=${BACKEND_DIR}
ExecStart=${BACKEND_BINARY}
Environment=CONFIG_FILE=${CONFIG_FILE}
Environment=SERVER_BIND=${BACKEND_BIND_ADDR}
Environment=DATABASE_URL=${DATABASE_URL}
Environment=LOG_FILE_PATH=${LOG_FILE_PATH}
Environment=FETCH_INTERVAL_SECS=${FETCH_INTERVAL_SECS}
Environment=FETCH_BATCH_SIZE=${FETCH_BATCH_SIZE}
Environment=FETCH_CONCURRENCY=${FETCH_CONCURRENCY}
Environment=FETCH_TIMEOUT_SECS=${FETCH_TIMEOUT_SECS}
Environment=LOG_LEVEL=${LOG_LEVEL}
Restart=always
RestartSec=5
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=multi-user.target
EOF

  systemctl daemon-reload
  systemctl enable --now "${SYSTEMD_SERVICE_NAME}"
  systemctl restart "${SYSTEMD_SERVICE_NAME}"
}

reload_nginx() {
  echo "[6/6] Reloading nginx..."
  systemctl reload nginx
}

backend_service_start() {
  echo "Starting ${SYSTEMD_SERVICE_NAME}..."
  systemctl start "${SYSTEMD_SERVICE_NAME}"
}

backend_service_stop() {
  echo "Stopping ${SYSTEMD_SERVICE_NAME}..."
  systemctl stop "${SYSTEMD_SERVICE_NAME}"
}

backend_service_status() {
  systemctl status "${SYSTEMD_SERVICE_NAME}"
}

uninstall() {
  echo "Disabling and removing systemd unit..."
  systemctl stop "${SYSTEMD_SERVICE_NAME}" || true
  systemctl disable "${SYSTEMD_SERVICE_NAME}" || true
  rm -f "${SYSTEMD_UNIT_PATH}"
  systemctl daemon-reload

  echo "Removing nginx site..."
  rm -f "${NGINX_ENABLED_PATH}"
  rm -f "${NGINX_CONF_PATH}"
  systemctl reload nginx || true

  echo "Uninstall complete (static assets left in ${STATIC_ROOT})."
}

deploy() {
  require_cmd runuser
  validate_config
  ensure_paths
  build_backend
  build_frontend
  sync_static_assets
  write_nginx_config
  write_systemd_unit
  reload_nginx
  echo "Deployment complete."
}

usage() {
  cat <<EOF
Usage: sudo bash deploy.sh <command>

Commands:
  deploy     Build and deploy backend/frontend, update nginx and systemd (default)
  start      Start the backend systemd service
  stop       Stop the backend systemd service
  status     Show backend systemd service status
  uninstall  Remove nginx config and systemd unit (keeps build artifacts)
EOF
}

main() {
  ensure_root

  local cmd="${1:-deploy}"

  case "${cmd}" in
    deploy)
      deploy
      ;;
    start)
      backend_service_start
      ;;
    stop)
      backend_service_stop
      ;;
    status)
      backend_service_status
      ;;
    uninstall)
      uninstall
      ;;
    -h|--help|help)
      usage
      ;;
    *)
      echo "Unknown command: ${cmd}" >&2
      usage
      exit 1
      ;;
  esac
}

main "$@"
