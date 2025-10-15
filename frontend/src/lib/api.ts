import {
  FeedUpsertPayload,
  PageResp,
  ArticleOut,
  FeedOut,
  FeedTestResult,
} from "../types/api";

type QueryParams = Record<string, string | number | undefined | null>;

const defaultApiBase =
  typeof window !== "undefined" ? `${window.location.origin}/api` : "/api";

let cachedApiBase: string | null = null;
let apiBasePromise: Promise<string> | null = null;

const normalizeBase = (value: string) => value.replace(/\/+$/, "");

const resolveDefaultBase = () => normalizeBase(defaultApiBase);

const loadBaseFromConfig = async (): Promise<string | null> => {
  try {
    const res = await fetch("/config/frontend", {
      headers: { Accept: "application/json" },
    });
    if (!res.ok) return null;
    const data = (await res.json()) as { api_base_url?: string | null };
    if (data.api_base_url) {
      return normalizeBase(data.api_base_url);
    }
    return null;
  } catch {
    return null;
  }
};

const getApiBase = async (): Promise<string> => {
  if (cachedApiBase) return cachedApiBase;

  const fromEnv = import.meta.env.VITE_API_BASE_URL;
  if (fromEnv && typeof fromEnv === "string" && fromEnv.trim()) {
    cachedApiBase = normalizeBase(fromEnv);
    return cachedApiBase;
  }

  if (!apiBasePromise) {
    apiBasePromise = loadBaseFromConfig().then(
      (base) => base ?? resolveDefaultBase()
    );
  }

  cachedApiBase = await apiBasePromise;
  return cachedApiBase;
};

const request = async (path: string, init?: RequestInit) => {
  const base = await getApiBase();
  return fetch(`${base}${path}`, init);
};

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
  keyword?: string;
}): Promise<PageResp<ArticleOut>> {
  const res = await request(`/articles${toQueryString(params)}`, {
    headers: { Accept: "application/json" },
  });
  return parseJSON<PageResp<ArticleOut>>(res);
}

export async function listFeeds(): Promise<FeedOut[]> {
  const res = await request("/feeds", {
    headers: { Accept: "application/json" },
  });
  return parseJSON<FeedOut[]>(res);
}

export async function getFeaturedArticles(limit = 6): Promise<ArticleOut[]> {
  const res = await request(
    `/articles/featured${toQueryString({ limit })}`,
    {
      headers: { Accept: "application/json" },
    }
  );
  return parseJSON<ArticleOut[]>(res);
}

export async function recordArticleClick(id: number): Promise<void> {
  await request(`/articles/${id}/click`, {
    method: "POST",
  });
}

export async function upsertFeed(payload: FeedUpsertPayload): Promise<FeedOut> {
  const res = await request("/feeds", {
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
  const res = await request(`/feeds/${id}`, {
    method: "DELETE",
  });
  if (!res.ok) {
    const message = await res.text();
    throw new Error(message || `Failed to delete feed ${id}`);
  }
}

export async function testFeed(url: string): Promise<FeedTestResult> {
  const res = await request("/feeds/test", {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      Accept: "application/json",
    },
    body: JSON.stringify({ url }),
  });

  if (!res.ok) {
    const raw = await res.text();
    let message = `Feed test failed with status ${res.status}`;
    if (raw) {
      try {
        const body = JSON.parse(raw) as { error?: { message?: string } };
        message = body.error?.message ?? message;
      } catch {
        message = raw;
      }
    }
    throw new Error(message);
  }

  return (await res.json()) as FeedTestResult;
}
