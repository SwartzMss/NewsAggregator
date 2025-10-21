export type ArticleOut = {
  id: number;
  title: string;
  url: string;
  description?: string | null;
  language?: string | null;
  source_domain: string;
  published_at: string; // ISO8601 UTC
  click_count: number;
};

export type FeedOut = {
  id: number;
  url: string;
  title?: string | null;
  site_url?: string | null;
  source_domain: string;
  language?: string | null;
  enabled: boolean;
  fetch_interval_seconds: number;
  filter_condition?: string | null;
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

export type FeedUpsertPayload = {
  id?: number;
  url: string;
  source_domain: string;
  language?: string | null;
  enabled?: boolean;
  fetch_interval_seconds?: number;
  title?: string | null;
  site_url?: string | null;
  filter_condition?: string | null;
};

export type FeedTestResult = {
  status: number;
  title?: string | null;
  site_url?: string | null;
  entry_count: number;
};

export type TranslationSettings = {
  provider: string;
  translation_enabled: boolean;
  deepseek_configured: boolean;
  ollama_configured: boolean;
  deepseek_api_key_masked?: string | null;
  deepseek_error?: string | null;
  ollama_error?: string | null;
  ollama_base_url?: string | null;
  ollama_model?: string | null;
};

export type TranslationSettingsUpdate = {
  provider?: string;
  translation_enabled?: boolean;
  deepseek_api_key?: string;
  ollama_base_url?: string;
  ollama_model?: string;
};

export type AiDedupSettings = {
  enabled: boolean;
  provider?: string | null;
  deepseek_configured: boolean;
  ollama_configured: boolean;
  threshold: number;
  max_checks: number;
};

export type AiDedupSettingsUpdate = {
  enabled?: boolean;
  provider?: string;
};

export type AdminLoginResponse = {
  token: string;
  expires_in: number;
};

// Alerts / Notification Center
export type AlertRecord = {
  id: number;
  ts: string; // ISO8601
  level: "info" | "warn" | "error" | string;
  code: string;
  title: string;
  message: string;
  attrs: Record<string, any>;
  source: string;
  dedupe_key?: string | null;
  count: number;
};
