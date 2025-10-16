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
  available_providers: string[];
  baidu_configured: boolean;
  deepseek_configured: boolean;
  ollama_configured: boolean;
  baidu_app_id_masked?: string | null;
  baidu_secret_key_masked?: string | null;
  deepseek_api_key_masked?: string | null;
  baidu_error?: string | null;
  deepseek_error?: string | null;
  ollama_error?: string | null;
  ollama_base_url?: string | null;
  ollama_model?: string | null;
  translate_descriptions: boolean;
};

export type TranslationSettingsUpdate = {
  provider?: string;
  baidu_app_id?: string;
  baidu_secret_key?: string;
  deepseek_api_key?: string;
  ollama_base_url?: string;
  ollama_model?: string;
  translate_descriptions?: boolean;
};

export type AdminLoginResponse = {
  token: string;
  expires_in: number;
};
