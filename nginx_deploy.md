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

## 2. Nginx 配置
在 `/etc/nginx/sites-available/news-aggregator.conf` 写入：
```nginx
server {
    listen 80;
    listen [::]:80;
    server_name your-domain.com www.your-domain.com;
    return 301 https://$host$request_uri;
}

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
