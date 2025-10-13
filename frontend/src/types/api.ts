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
};

export type FeedTestResult = {
  status: number;
  title?: string | null;
  site_url?: string | null;
  entry_count: number;
};
