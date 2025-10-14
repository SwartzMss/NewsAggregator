# Nginx 部署指南（HTTPS）

以下示例假设：
- 后端 Rust 服务监听 `127.0.0.1:8081`
- 前端使用 `npm run build` 产物，部署在 `/var/www/news-aggregator/dist`
- 域名为 `your-domain.com`
- 使用 Let’s Encrypt 申请免费证书（可替换成自有证书）

## 1. 申请 HTTPS 证书
```bash
sudo apt-get update
sudo apt-get install certbot python3-certbot-nginx
sudo certbot --nginx -d your-domain.com -d www.your-domain.com
```
> 如果已有证书，将证书与私钥复制至服务器即可；替换下方 Nginx 配置中的路径。

Certbot 会：
- 验证域名所有权
- 生成证书 (`/etc/letsencrypt/live/...`)
- 配置自动续期

## 一键部署脚本（推荐）
仓库新增了 `nginx/` 目录，提供一键部署脚本：

1. 按需调整 `config/config.yaml`（或 `config/config.example.yaml` 复制后）里的 `deployment` 部分，填好域名、证书路径、部署用户等参数。若需要使用其他配置文件，可设置环境变量 `DEPLOY_CONFIG_FILE=/path/to/config.yaml`。
   - 如需在本地通过 `http://localhost` 调试，可将 `deployment.domain_aliases` 中加入 `localhost`，脚本会自动把它写入 `server_name`。
2. 在项目目录中以部署用户手动构建产物（脚本不会再执行编译）。可以分别进入目录执行命令，或直接运行 `nginx/build.sh`：
  ```bash
  bash nginx/build.sh
  ```
  构建完成后，`backend/target/release/backend` 与 `frontend/dist` 应当存在。
3. 使用 root 权限执行脚本完成部署：
   ```bash
   sudo bash nginx/deploy.sh
   ```
   脚本会完成：
   - 校验已构建的后端二进制与前端产物
   - 同步前端 dist 到 `/var/www/news-aggregator/dist`
   - 生成仅监听 443 端口的 `/etc/nginx/sites-available/news-aggregator.conf` 并启用
   - 写入 `/etc/systemd/system/news-backend.service`
   - 重新加载 systemd 与 nginx
4. 根据输出确认各步骤成功，可以通过 `systemctl status news-backend.service`、`nginx -t` 等命令复查。

首次部署：需先按步骤 2 完成后端与前端的手动构建，再执行 `sudo bash nginx/deploy.sh deploy` 以同步资源、写入 nginx 配置与 systemd 单元。只有成功执行过 `deploy` 之后，下面的运维命令才能生效。

部署完成后，可使用脚本管理生命周期：

```bash
sudo bash nginx/deploy.sh status    # 查看 systemd 状态（不会重新部署）
sudo bash nginx/deploy.sh start     # 启动已安装的后端服务（仅执行 systemctl start）
sudo bash nginx/deploy.sh stop      # 停止后端服务
sudo bash nginx/deploy.sh uninstall # 移除 systemd 和 nginx 配置
```

## 2. Nginx 配置
在 `/etc/nginx/sites-available/news-aggregator.conf` 写入（脚本输出仅含 HTTPS，若需 HTTP→HTTPS 跳转可自行追加 80 端口 server 块）：
```nginx
server {
    listen 443 ssl http2;
    listen [::]:443 ssl http2;
    server_name your-domain.com www.your-domain.com;

    ssl_certificate     /etc/letsencrypt/live/your-domain.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/your-domain.com/privkey.pem;
    include             /etc/letsencrypt/options-ssl-nginx.conf;
    ssl_dhparam         /etc/letsencrypt/ssl-dhparams.pem;

    root /var/www/news-aggregator/dist;
    index index.html;
    try_files $uri $uri/ /index.html;

    location /api/ {
        proxy_pass http://127.0.0.1:8081/;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        proxy_http_version 1.1;
    }

    location /config/ {
        proxy_pass http://127.0.0.1:8081/config/;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        proxy_http_version 1.1;
    }

    location /healthz {
        proxy_pass http://127.0.0.1:8081/healthz;
    }

    location ~* \.(css|js|jpg|jpeg|png|gif|ico|svg)$ {
        expires 7d;
        access_log off;
    }
}
```
启用配置并重载：
```bash
sudo ln -s /etc/nginx/sites-available/news-aggregator.conf /etc/nginx/sites-enabled/
sudo nginx -t
sudo systemctl reload nginx
```

## 3. 部署前端静态资源
```bash
cd /path/to/NewsAggregator/frontend
npm install
npm run build
sudo mkdir -p /var/www/news-aggregator/dist
sudo rsync -av dist/ /var/www/news-aggregator/dist/
```
可在 CI 或手动更新时重复 `npm run build` + `rsync` 步骤。

## 4. 后端服务（Systemd）
示例 systemd 单元：`/etc/systemd/system/news-backend.service`
```ini
[Unit]
Description=News Aggregator Backend
After=network.target

[Service]
WorkingDirectory=/home/your-user/WorkSpace/NewsAggregator/backend
ExecStart=/usr/bin/env \
    DATABASE_URL=postgres://user:pass@127.0.0.1:55432/superset \
    SERVER_BIND=127.0.0.1:8081 \
    LOG_FILE_PATH=/var/log/news-backend.log \
    FETCH_INTERVAL_SECS=600 \
    RUST_LOG=info \
    /home/your-user/WorkSpace/NewsAggregator/backend/target/release/backend
Restart=always
User=your-user
Environment=FETCH_CONCURRENCY=4
Environment=FETCH_BATCH_SIZE=8

[Install]
WantedBy=multi-user.target
```
启用后端：
```bash
sudo systemctl daemon-reload
sudo systemctl enable --now news-backend.service
```

## 5. 验证
- 浏览器访问 `https://your-domain.com`
- `curl -I https://your-domain.com/api/articles`
- 检查证书续期：`sudo certbot renew --dry-run`

## 6. 注意事项
- `proxy_pass` 末尾必须带 `/`，否则路径拼接会出错。
- 若后端只有 HTTP，不要把它暴露在公网上，保持 `127.0.0.1` 绑定。
- 若需要 WebSocket/SSE，在 `location /api/` 补充：
  ```nginx
  proxy_set_header Upgrade $http_upgrade;
  proxy_set_header Connection "upgrade";
  ```
- 日志目录需确保 systemd 运行用户名可写。
- 若使用 Let’s Encrypt，检查 crontab/systemd timer 是否存在自动续期（Certbot 安装时会自动配置）。

这样配置后，用户只访问 HTTPS 域名，前端静态资源与 API 请求均由 Nginx 统一代理，后端仍保持本地端口运行，更安全也便于扩展。
