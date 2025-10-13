# 前端指南

前端工程基于 Vite + React，提供单页应用界面，通过 HTTP API 与后端交互。

## 技术栈概览
- 构建工具：Vite（ESBuild + Rollup）
- 语言：TypeScript（严格模式）
- 数据请求：TanStack Query
- 样式系统：Tailwind CSS

目录结构示例：
```
frontend/
├── src/
│   ├── api/        # 封装 API 请求
│   ├── components/ # 复用组件
│   ├── pages/      # 页面级组件（新闻列表、Feed 管理）
│   └── main.tsx
├── index.html
└── vite.config.ts
```

## 环境变量
通过 `.env` 或 shell 设置：
```
VITE_API_BASE_URL=http://127.0.0.1:8081/api
```
部署脚本会让 nginx 反向代理 `/api/` 到后端，并在 `/` 提供静态资源。

## 本地开发
```bash
cd frontend
npm install         # 或 npm ci
npm run dev         # 启动 Vite 开发服务，默认 http://127.0.0.1:5173
```

常用命令：
- `npm run build`：生成生产环境包，输出到 `dist/`
- `npm run lint`：TypeScript 类型检查
- `npm run preview`：在本地预览已构建产物

## API 约定
API 客户端定义在 `src/api`。当前主要接口：
- `GET /articles`：文章列表（分页 + 可选时间范围）
- `GET /feeds`：Feed 列表
- `POST /feeds`：新增或更新 Feed
- `PATCH /feeds/:id`：修改 Feed
- `DELETE /feeds/:id`：删除 Feed

如果后端接口前缀改变，请同步更新 `VITE_API_BASE_URL`。

## 生产构建与部署
部署脚本 `nginx/deploy.sh deploy` 会自动执行：
1. `npm install`
2. `npm run build`
3. 将 `dist/` 同步到 `/var/www/news-aggregator/dist`

手动上线时可参考：
```bash
cd frontend
npm install
npm run build
sudo rsync -a dist/ /var/www/news-aggregator/dist/
```

## 常见问题
- 生产环境页面空白：检查 nginx `root` 指向的目录是否存在 `dist/`。
- API 返回 404/500：确认 nginx 反向代理配置以及后端服务状态。
- 样式异常：重新构建或执行 `npm run build -- --mode development` 便于调试。
