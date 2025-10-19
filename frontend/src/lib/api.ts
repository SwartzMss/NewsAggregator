import {
  FeedUpsertPayload,
  PageResp,
  ArticleOut,
  FeedOut,
  FeedTestResult,
  AdminLoginResponse,
  TranslationSettings,
  TranslationSettingsUpdate,
  AiDedupSettings,
  AiDedupSettingsUpdate,
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

  // Vite env var if provided overrides config.
  const fromEnv = (import.meta as any).env?.VITE_API_BASE_URL;
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
    let resolved = message;
    if (message) {
      try {
        const parsed = JSON.parse(message) as {
          error?: { message?: string };
          message?: string;
        };
        resolved =
          parsed.error?.message ?? parsed.message ??
          (typeof parsed === "string" ? parsed : message);
      } catch {
        resolved = message;
      }
    }
    throw new Error(resolved || `Request failed with status ${response.status}`);
  }
  return (await response.json()) as T;
};

export class UnauthorizedError extends Error {
  constructor(message = "Unauthorized") {
    super(message);
    this.name = "UnauthorizedError";
  }
}

const adminRequest = async (path: string, token: string, init?: RequestInit) => {
  const headers = new Headers(init?.headers ?? {});
  if (!token) {
    throw new UnauthorizedError();
  }
  headers.set("Authorization", `Bearer ${token}`);

  const res = await request(path, { ...init, headers });
  if (res.status === 401) {
    throw new UnauthorizedError();
  }
  return res;
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

export async function adminLogin(
  username: string,
  password: string
): Promise<AdminLoginResponse> {
  const res = await request("/admin/login", {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      Accept: "application/json",
    },
    body: JSON.stringify({ username, password }),
  });
  if (res.status === 401) {
    const message = await res.text();
    let resolved = "用户名或密码错误";
    if (message) {
      try {
        const parsed = JSON.parse(message) as {
          error?: { message?: string };
          message?: string;
        };
        resolved = parsed.error?.message ?? parsed.message ?? resolved;
      } catch {
        resolved = message;
      }
    }
    throw new Error(resolved);
  }
  return parseJSON<AdminLoginResponse>(res);
}

export async function adminLogout(token: string): Promise<void> {
  const res = await adminRequest("/admin/logout", token, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      Accept: "application/json",
    },
    body: JSON.stringify({ token }),
  });
  if (!res.ok) {
    const message = await res.text();
    throw new Error(message || "Failed to logout");
  }
}

export async function listFeeds(token: string): Promise<FeedOut[]> {
  const res = await adminRequest("/admin/api/feeds", token, {
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

export async function upsertFeed(
  token: string,
  payload: FeedUpsertPayload
): Promise<FeedOut> {
  const res = await adminRequest("/admin/api/feeds", token, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      Accept: "application/json",
    },
    body: JSON.stringify(payload),
  });
  return parseJSON<FeedOut>(res);
}

export async function deleteFeed(token: string, id: number): Promise<void> {
  const res = await adminRequest(`/admin/api/feeds/${id}`, token, {
    method: "DELETE",
  });
  if (!res.ok) {
    const message = await res.text();
    throw new Error(message || `Failed to delete feed ${id}`);
  }
}

export async function getTranslationSettings(
  token: string
): Promise<TranslationSettings> {
  const res = await adminRequest(
    "/admin/api/settings/translation",
    token,
    {
      headers: { Accept: "application/json" },
    }
  );
  return parseJSON<TranslationSettings>(res);
}

export async function updateTranslationSettings(
  token: string,
  payload: TranslationSettingsUpdate
): Promise<TranslationSettings> {
  const res = await adminRequest(
    "/admin/api/settings/translation",
    token,
    {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        Accept: "application/json",
      },
      body: JSON.stringify(payload),
    }
  );
  return parseJSON<TranslationSettings>(res);
}

// Model settings (Deepseek/Ollama)
export type ModelSettings = {
  deepseek_api_key_masked?: string | null;
  ollama_base_url?: string | null;
  ollama_model?: string | null;
};

export type ModelSettingsUpdate = {
  deepseek_api_key?: string;
  ollama_base_url?: string;
  ollama_model?: string;
};

export async function getModelSettings(token: string): Promise<ModelSettings> {
  const res = await adminRequest("/admin/api/settings/models", token, { method: "GET" });
  return parseJSON<ModelSettings>(res);
}

export async function updateModelSettings(token: string, payload: ModelSettingsUpdate): Promise<ModelSettings> {
  const res = await adminRequest("/admin/api/settings/models", token, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(payload),
  });
  return parseJSON<ModelSettings>(res);
}

export async function testModelConnectivity(token: string, provider: "deepseek" | "ollama"): Promise<boolean> {
  const res = await adminRequest("/admin/api/settings/models/test", token, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ provider }),
  });
  if (!res.ok) {
    const msg = await res.text();
    throw new Error(msg || "测试失败");
  }
  return true;
}

export async function testFeed(
  token: string,
  url: string
): Promise<FeedTestResult> {
  const res = await adminRequest("/admin/api/feeds/test", token, {
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

// AI Dedup settings
export async function getAiDedupSettings(
  token: string
): Promise<AiDedupSettings> {
  const res = await adminRequest(
    "/admin/api/settings/ai_dedup",
    token,
    { headers: { Accept: "application/json" } }
  );
  return parseJSON<AiDedupSettings>(res);
}

export async function updateAiDedupSettings(
  token: string,
  payload: AiDedupSettingsUpdate
): Promise<AiDedupSettings> {
  const res = await adminRequest(
    "/admin/api/settings/ai_dedup",
    token,
    {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        Accept: "application/json",
      },
      body: JSON.stringify(payload),
    }
  );
  return parseJSON<AiDedupSettings>(res);
}
