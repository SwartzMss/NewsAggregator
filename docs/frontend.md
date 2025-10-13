# Frontend Guide

The frontend is a Vite + React single-page app that consumes the backend HTTP API.

## Stack Overview
- Build tool: Vite (ESBuild + Rollup)
- Language: TypeScript (strict mode)
- State/query: TanStack Query
- Styling: Tailwind CSS

Source layout highlights:
```
frontend/
├── src/
│   ├── api/        # Fetch client and API adapters
│   ├── components/ # Shared UI components
│   ├── pages/      # Route-level views (News list, Feeds)
│   └── main.tsx
├── index.html
└── vite.config.ts
```

## Environment Variables
Set through `.env` files or the shell:
```
VITE_API_BASE_URL=http://127.0.0.1:8081/api
```
The deploy script serves the frontend behind nginx at `/`, and proxies `/api/` to the backend.

## Local Development
```bash
cd frontend
npm install         # or npm ci
npm run dev         # starts Vite dev server on http://127.0.0.1:5173
```

Common tasks:
- `npm run build` – production bundle in `dist/`
- `npm run lint` – TypeScript type check
- `npm run preview` – serve built assets locally

## API Conventions
Requests are defined in `src/api`. The app currently relies on:
- `GET /articles` with pagination and optional time window
- `GET /feeds`
- `POST /feeds`
- `PATCH /feeds/:id`
- `DELETE /feeds/:id`

Adjust `VITE_API_BASE_URL` if the backend path changes.

## Production Build & Deployment
The deployment script (`nginx/deploy.sh deploy`) performs:
1. `npm install`
2. `npm run build`
3. Syncs `dist/` to `/var/www/news-aggregator/dist`

Manual steps (if needed):
```bash
cd frontend
npm install
npm run build
sudo rsync -a dist/ /var/www/news-aggregator/dist/
```

## Troubleshooting
- Blank page in production: check nginx root path and ensure `dist/` exists.
- API 404/500: verify nginx proxy target or backend service status.
- Style issues: run `npm run build -- --mode development` to include source maps for debugging.
