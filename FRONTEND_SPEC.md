
# 前端设计（MVP｜无鉴权｜简化版：不做多语言与来源筛选）

> 目标：页面**尽量简单**，直接“平铺显示”聚合后的新闻列表；不提供 `lang` 和 `source_domain` 的筛选。仅保留“时间范围（可选）+ 分页”，以及 Feed 的新增/管理。

---

## 0. 技术栈（推荐不变）
- Vite + React + TypeScript
- React Router v6
- TanStack Query（请求缓存/重试/状态）
- Tailwind CSS
- 时间格式化：`Intl.DateTimeFormat`

环境变量：
```
VITE_API_BASE_URL=http://127.0.0.1:8080
```

---

## 1. 路由与信息架构

```
/                新闻列表（默认页）
// 平铺 + 时间范围（可选）+ 分页；无语言/来源筛选
/feeds           Feed 管理（列表 + 新增/编辑/启用）
```

全局布局：顶部导航（News｜Feeds），底部简单版权。

---

## 2. 页面设计

### 2.1 新闻列表页（/）
**目的**：按时间倒序“平铺显示”最近文章；支持分页；（可选）时间范围过滤。

#### 交互
- 工具栏（极简）：
  - 时间窗 `from` / `to`（可选，默认不填即“最近”）
  - 每页条数 `page_size`（10/20/50）
  - 刷新按钮
- 列表项：
  - 标题（外链新开）
  - 来源域名（仅展示，不可筛选）
  - 发布时间（本地时区，相对时间可选）
  - 摘要（两行省略）
- 分页：上一页/下一页（`page` 参数）
- 状态：Loading/Empty/Error

#### URL 参数（与后端对齐）
```
/?from=2025-10-10T00:00:00Z&to=2025-10-11T23:59:59Z&page=1&page_size=20
```
> 不包含 `lang` 与 `source_domain`。

#### 数据获取
- `GET /articles`（仅用 `from/to/page/page_size`）

#### 组件
- `<Toolbar />`：时间窗 + 每页条数 + 刷新
- `<ArticleList />`：渲染卡片
- `<Pagination />`：分页控件

---

### 2.2 Feed 管理（/feeds）
**目的**：新增/编辑/启用/禁用 RSS 订阅源，观察抓取状态。

#### 列表表头
- 标题 `title`（无则显示 `source_domain`）
- URL
- 来源域名 `source_domain`（仅展示）
- 语言 / 国家（仅展示，不影响筛选）
- 启用状态（开关）
- 抓取状态：`last_fetch_status`
- 上次抓取：`last_fetch_at`
- 失败次数：`fail_count`
- 操作：编辑（弹窗）/ 删除（可选，或仅禁用）

#### 交互
- **新增**：表单 `POST /feeds`
  - 字段：`url`（必填），`source_domain`（可自动从 `url` 推断，也可手动覆盖），`language?`，`country?`，`enabled`，`fetch_interval_seconds`
- **编辑**：`PATCH /feeds/:id`
- **启用/禁用**：切换 `enabled` 即发 `PATCH`
- **删除**（可选）：`DELETE /feeds/:id` 或仅禁用
- **刷新**：操作后自动刷新列表

---

## 3. TypeScript 模型（与后端一致）

```ts
export type ArticleOut = {
  id: number;
  title: string;
  url: string;
  description?: string | null;
  language?: string | null;
  source_domain: string;
  source_display_name?: string | null;
  published_at: string; // UTC ISO8601
};

export type FeedOut = {
  id: number;
  url: string;
  title?: string | null;
  site_url?: string | null;
  source_domain: string;
  source_display_name?: string | null;
  language?: string | null;
  country?: string | null;
  enabled: boolean;
  fetch_interval_seconds: number;
  last_fetch_at?: string | null;
  last_fetch_status?: number | null;
  fail_count: number;
};

export type PageResp<T> = {
  page: number;
  page_size: number;
  total_hint: number;
  items: T[];
};
```

---

## 4. API 封装（与简化筛选对齐）

```ts
// src/lib/api.ts
const BASE = import.meta.env.VITE_API_BASE_URL;

type Query = Record<string, string | number | boolean | undefined | null>;

export function qs(q: Query) {
  const p = new URLSearchParams();
  Object.entries(q).forEach(([k, v]) => {
    if (v !== undefined && v !== null && v !== "") p.set(k, String(v));
  });
  return p.toString();
}

export async function getJSON<T>(path: string, params?: Query, signal?: AbortSignal): Promise<T> {
  const url = params ? `${BASE}${path}?${qs(params)}` : `${BASE}${path}`;
  const r = await fetch(url, { signal, headers: { "Accept": "application/json" } });
  if (!r.ok) throw new Error(`GET ${path} ${r.status}`);
  return r.json();
}

export async function postJSON<T>(path: string, body: any, signal?: AbortSignal): Promise<T> {
  const r = await fetch(`${BASE}${path}`, {
    method: "POST",
    headers: { "Content-Type": "application/json", "Accept": "application/json" },
    body: JSON.stringify(body),
    signal,
  });
  if (!r.ok) throw new Error(`POST ${path} ${r.status}`);
  return r.json();
}

export async function patchJSON<T>(path: string, body: any, signal?: AbortSignal): Promise<T> {
  const r = await fetch(`${BASE}${path}`, {
    method: "PATCH",
    headers: { "Content-Type": "application/json", "Accept": "application/json" },
    body: JSON.stringify(body),
    signal,
  });
  if (!r.ok) throw new Error(`PATCH ${path} ${r.status}`);
  return r.json();
}

export async function del(path: string, signal?: AbortSignal) {
  const r = await fetch(`${BASE}${path}`, { method: "DELETE", signal });
  if (!r.ok) throw new Error(`DELETE ${path} ${r.status}`);
}
```

常用：
```ts
// 仅时间+分页
getJSON<PageResp<ArticleOut>>("/articles", { from, to, page, page_size });

// Feeds
getJSON<FeedOut[]>("/feeds");
postJSON<FeedOut>("/feeds", payload);
patchJSON<FeedOut>(`/feeds/${id}`, patch);
del(`/feeds/${id}`);
```

---

## 5. 组件结构

```
src/
├─ app/
│  ├─ App.tsx
│  └─ routes.tsx
├─ pages/
│  ├─ NewsList/
│  │  ├─ index.tsx            # 工具栏 + 列表 + 分页
│  │  ├─ Toolbar.tsx          # 时间窗/每页/刷新
│  │  └─ ArticleCard.tsx
│  └─ Feeds/
│     ├─ index.tsx
│     ├─ FeedTable.tsx
│     └─ FeedFormModal.tsx
├─ lib/
│  ├─ api.ts
│  ├─ time.ts
│  └─ domain.ts
├─ types/
│  └─ api.ts
├─ styles/
│  └─ globals.css
└─ main.tsx
```

---

## 6. 伪线框（ASCII）

### 新闻列表（平铺，无语言/来源筛选）
```
+--------------------------------------------------------------+
| News | Feeds                                                 |
+--------------------------------------------------------------+
| From [..]  To [..]   Page size [20 v]         [Refresh]      |
+--------------------------------------------------------------+
| ▸ Title...............................................[↗]    |
|   reuters.com · 2 hours ago                                   |
|   short summary...                                            |
+--------------------------------------------------------------+
| ▸ Title...............................................[↗]    |
|   bbc.co.uk · 3 hours ago                                     |
|   short summary...                                            |
+--------------------------------------------------------------+
|  « Prev  1  2  3  Next »                                      |
+--------------------------------------------------------------+
```

### Feed 管理
```
+--------------------------------------------------------------+
| News | Feeds                                   [+ New Feed]  |
+--------------------------------------------------------------+
| Title     | URL             | Source     | Lang | Last | Fail |
|-----------+-----------------+------------+------+------|------|
| Reuters.. | https://...rss  | reuters... | en   | 200  | 0    |
| [Edit] [Toggle Enabled] [Delete]                              
+--------------------------------------------------------------+
```

---

## 7. 开发清单（简化）
1. 项目骨架与路由。
2. `lib/api.ts` 封装。
3. 新闻列表页（仅时间+分页）。
4. Feeds 管理页（增/改/启用禁用）。
5. Loading/Empty/Error 统一处理。

---

## 8. 未来增强（非 MVP）
- 关键词搜索（待后端 ILIKE/全文支持）。
- 来源/语言筛选（如有需求再加）。
- 导入 OPML/文本；抓取日志看板；深色模式。
