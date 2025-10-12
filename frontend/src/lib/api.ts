import { FeedUpsertPayload, PageResp, ArticleOut, FeedOut } from "../types/api";

type QueryParams = Record<string, string | number | undefined | null>;

const API_BASE = import.meta.env.VITE_API_BASE_URL ?? "http://127.0.0.1:8081";

const toQueryString = (params?: QueryParams) => {
  if (!params) return "";
  const search = new URLSearchParams();
  Object.entries(params).forEach(([key, value]) => {
    if (value === undefined || value === null || value === "") return;
    search.append(key, String(value));
  });
  const query = search.toString();
  return query ? `?${query}` : "";
};

const parseJSON = async <T>(response: Response): Promise<T> => {
  if (!response.ok) {
    const message = await response.text();
    throw new Error(message || `Request failed with status ${response.status}`);
  }
  return (await response.json()) as T;
};

export async function getArticles(params: {
  from?: string;
  to?: string;
  page?: number;
  page_size?: number;
}): Promise<PageResp<ArticleOut>> {
  const res = await fetch(`${API_BASE}/articles${toQueryString(params)}`, {
    headers: { Accept: "application/json" },
  });
  return parseJSON<PageResp<ArticleOut>>(res);
}

export async function listFeeds(): Promise<FeedOut[]> {
  const res = await fetch(`${API_BASE}/feeds`, {
    headers: { Accept: "application/json" },
  });
  return parseJSON<FeedOut[]>(res);
}

export async function getFeaturedArticles(limit = 6): Promise<ArticleOut[]> {
  const res = await fetch(
    `${API_BASE}/articles/featured${toQueryString({ limit })}`,
    {
      headers: { Accept: "application/json" },
    }
  );
  return parseJSON<ArticleOut[]>(res);
}

export async function recordArticleClick(id: number): Promise<void> {
  await fetch(`${API_BASE}/articles/${id}/click`, {
    method: "POST",
  });
}

export async function upsertFeed(payload: FeedUpsertPayload): Promise<FeedOut> {
  const res = await fetch(`${API_BASE}/feeds`, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      Accept: "application/json",
    },
    body: JSON.stringify(payload),
  });
  return parseJSON<FeedOut>(res);
}

export async function deleteFeed(id: number): Promise<void> {
  const res = await fetch(`${API_BASE}/feeds/${id}`, {
    method: "DELETE",
  });
  if (!res.ok) {
    const message = await res.text();
    throw new Error(message || `Failed to delete feed ${id}`);
  }
}
